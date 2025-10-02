//! ATProto authentication module
//!
//! This module implements OAuth authentication with ATProto/Bluesky accounts.
//! For single-user forge, this provides a way to authenticate the forge owner.

pub mod session;
pub mod atproto;

pub use session::{Session, SessionManager};
pub use atproto::{AtProtoAuthClient, AuthConfig};

use serde::{Deserialize, Serialize};

/// Represents an authenticated user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Decentralized Identifier (DID) - unique identifier in ATProto
    pub did: String,
    /// Handle (e.g., username.bsky.social)
    pub handle: String,
    /// Display name
    pub display_name: Option<String>,
    /// Avatar URL
    pub avatar: Option<String>,
}

impl User {
    pub fn new(did: String, handle: String) -> Self {
        Self {
            did,
            handle,
            display_name: None,
            avatar: None,
        }
    }
}
