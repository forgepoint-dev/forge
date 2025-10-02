//! Session management for single-tenant forge
//!
//! This module provides in-memory session storage for authenticated users.
//! Multiple users can be logged in simultaneously (single-tenant, multi-user).

use super::User;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

/// Represents an active session
#[derive(Debug, Clone)]
pub struct Session {
    /// Unique session identifier
    pub id: String,
    /// Authenticated user
    pub user: User,
    /// Access token for ATProto API calls
    pub access_token: String,
    /// Refresh token (if available)
    pub refresh_token: Option<String>,
    /// DPoP private key (PKCS#8) bound to the access token (optional)
    pub dpop_pkcs8: Option<Vec<u8>>,
    /// DPoP public JWK (optional)
    pub dpop_jwk: Option<String>,
}

/// Session manager for single-tenant forge
///
/// This is a simple in-memory store that holds multiple active sessions.
/// For a single-tenant forge, multiple users can be logged in simultaneously.
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new session for an authenticated user
    pub fn create_session(
        &self,
        user: User,
        access_token: String,
        refresh_token: Option<String>,
        dpop_pkcs8: Option<Vec<u8>>,
        dpop_jwk: Option<String>,
    ) -> Result<String> {
        let session_id = Uuid::new_v4().to_string();
        let session = Session {
            id: session_id.clone(),
            user,
            access_token,
            refresh_token,
            dpop_pkcs8,
            dpop_jwk,
        };

        let mut sessions = self.sessions.write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire session lock: {}", e))?;
        sessions.insert(session_id.clone(), session);

        Ok(session_id)
    }

    /// Get a session by its ID
    pub fn get_session(&self, session_id: &str) -> Result<Option<Session>> {
        let sessions = self.sessions.read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire session lock: {}", e))?;
        
        Ok(sessions.get(session_id).cloned())
    }

    /// Get user from a session
    pub fn get_user(&self, session_id: &str) -> Result<Option<User>> {
        let sessions = self.sessions.read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire session lock: {}", e))?;
        
        Ok(sessions.get(session_id).map(|s| s.user.clone()))
    }

    /// Delete a specific session (logout)
    pub fn delete_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire session lock: {}", e))?;
        sessions.remove(session_id);
        Ok(())
    }

    /// Get all active sessions (for admin purposes)
    pub fn get_all_sessions(&self) -> Result<Vec<Session>> {
        let sessions = self.sessions.read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire session lock: {}", e))?;
        
        Ok(sessions.values().cloned().collect())
    }

    /// Get count of active sessions
    pub fn session_count(&self) -> Result<usize> {
        let sessions = self.sessions.read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire session lock: {}", e))?;
        
        Ok(sessions.len())
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get_session() {
        let manager = SessionManager::new();
        let user = User::new("did:plc:test123".to_string(), "testuser.bsky.social".to_string());
        
        let session_id = manager.create_session(user.clone(), "token123".to_string(), None, None, None).unwrap();
        
        let session = manager.get_session(&session_id).unwrap();
        assert!(session.is_some());
        
        let session = session.unwrap();
        assert_eq!(session.user.did, "did:plc:test123");
        assert_eq!(session.access_token, "token123");
    }

    #[test]
    fn test_multiple_concurrent_sessions() {
        let manager = SessionManager::new();
        let user1 = User::new("did:plc:user1".to_string(), "user1.bsky.social".to_string());
        let user2 = User::new("did:plc:user2".to_string(), "user2.bsky.social".to_string());
        
        let session_id1 = manager.create_session(user1.clone(), "token1".to_string(), None, None, None).unwrap();
        let session_id2 = manager.create_session(user2.clone(), "token2".to_string(), None, None, None).unwrap();
        
        // Both sessions should exist
        let session1 = manager.get_session(&session_id1).unwrap();
        let session2 = manager.get_session(&session_id2).unwrap();
        
        assert!(session1.is_some());
        assert!(session2.is_some());
        assert_eq!(session1.unwrap().user.did, "did:plc:user1");
        assert_eq!(session2.unwrap().user.did, "did:plc:user2");
        
        // Session count should be 2
        assert_eq!(manager.session_count().unwrap(), 2);
    }

    #[test]
    fn test_get_user() {
        let manager = SessionManager::new();
        let user = User::new("did:plc:test123".to_string(), "testuser.bsky.social".to_string());
        
        let session_id = manager.create_session(user.clone(), "token123".to_string(), None, None, None).unwrap();
        
        let retrieved_user = manager.get_user(&session_id).unwrap();
        assert!(retrieved_user.is_some());
        assert_eq!(retrieved_user.unwrap().did, "did:plc:test123");
    }

    #[test]
    fn test_delete_session() {
        let manager = SessionManager::new();
        let user1 = User::new("did:plc:user1".to_string(), "user1.bsky.social".to_string());
        let user2 = User::new("did:plc:user2".to_string(), "user2.bsky.social".to_string());
        
        let session_id1 = manager.create_session(user1.clone(), "token1".to_string(), None, None, None).unwrap();
        let session_id2 = manager.create_session(user2.clone(), "token2".to_string(), None, None, None).unwrap();
        
        // Delete first session
        manager.delete_session(&session_id1).unwrap();
        
        // First session should be gone, second should remain
        assert!(manager.get_session(&session_id1).unwrap().is_none());
        assert!(manager.get_session(&session_id2).unwrap().is_some());
        assert_eq!(manager.session_count().unwrap(), 1);
    }

    #[test]
    fn test_invalid_session_id() {
        let manager = SessionManager::new();
        let user = User::new("did:plc:test123".to_string(), "testuser.bsky.social".to_string());
        
        let session_id = manager.create_session(user.clone(), "token123".to_string(), None).unwrap();
        
        let session = manager.get_session("invalid-id").unwrap();
        assert!(session.is_none());
        
        let session = manager.get_session(&session_id).unwrap();
        assert!(session.is_some());
    }

    #[test]
    fn test_get_all_sessions() {
        let manager = SessionManager::new();
        let user1 = User::new("did:plc:user1".to_string(), "user1.bsky.social".to_string());
        let user2 = User::new("did:plc:user2".to_string(), "user2.bsky.social".to_string());
        
        manager.create_session(user1.clone(), "token1".to_string(), None).unwrap();
        manager.create_session(user2.clone(), "token2".to_string(), None).unwrap();
        
        let all_sessions = manager.get_all_sessions().unwrap();
        assert_eq!(all_sessions.len(), 2);
    }
}
