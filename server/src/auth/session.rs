//! Session management for single-user forge
//!
//! This module provides in-memory session storage for the authenticated user.
//! Since this is a single-user forge, we only need to track one active session.

use super::User;
use anyhow::Result;
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
}

/// Session manager for single-user forge
///
/// This is a simple in-memory store that holds at most one active session.
/// For a single-user forge, this is sufficient.
pub struct SessionManager {
    current_session: Arc<RwLock<Option<Session>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            current_session: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a new session for the authenticated user
    pub fn create_session(
        &self,
        user: User,
        access_token: String,
        refresh_token: Option<String>,
    ) -> Result<String> {
        let session_id = Uuid::new_v4().to_string();
        let session = Session {
            id: session_id.clone(),
            user,
            access_token,
            refresh_token,
        };

        let mut current = self.current_session.write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire session lock: {}", e))?;
        *current = Some(session);

        Ok(session_id)
    }

    /// Get the current active session
    pub fn get_session(&self, session_id: &str) -> Result<Option<Session>> {
        let current = self.current_session.read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire session lock: {}", e))?;
        
        Ok(current.as_ref().and_then(|s| {
            if s.id == session_id {
                Some(s.clone())
            } else {
                None
            }
        }))
    }

    /// Get the current user if authenticated
    pub fn get_current_user(&self) -> Result<Option<User>> {
        let current = self.current_session.read()
            .map_err(|e| anyhow::anyhow!("Failed to acquire session lock: {}", e))?;
        
        Ok(current.as_ref().map(|s| s.user.clone()))
    }

    /// Delete the current session (logout)
    pub fn delete_session(&self) -> Result<()> {
        let mut current = self.current_session.write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire session lock: {}", e))?;
        *current = None;
        Ok(())
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
        
        let session_id = manager.create_session(user.clone(), "token123".to_string(), None).unwrap();
        
        let session = manager.get_session(&session_id).unwrap();
        assert!(session.is_some());
        
        let session = session.unwrap();
        assert_eq!(session.user.did, "did:plc:test123");
        assert_eq!(session.access_token, "token123");
    }

    #[test]
    fn test_get_current_user() {
        let manager = SessionManager::new();
        let user = User::new("did:plc:test123".to_string(), "testuser.bsky.social".to_string());
        
        manager.create_session(user.clone(), "token123".to_string(), None).unwrap();
        
        let current_user = manager.get_current_user().unwrap();
        assert!(current_user.is_some());
        assert_eq!(current_user.unwrap().did, "did:plc:test123");
    }

    #[test]
    fn test_delete_session() {
        let manager = SessionManager::new();
        let user = User::new("did:plc:test123".to_string(), "testuser.bsky.social".to_string());
        
        manager.create_session(user.clone(), "token123".to_string(), None).unwrap();
        manager.delete_session().unwrap();
        
        let current_user = manager.get_current_user().unwrap();
        assert!(current_user.is_none());
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
}
