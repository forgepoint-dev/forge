//! Configuration management for Forge
//!
//! This module handles loading and parsing of configuration files, primarily
//! for OCI-based extension distribution. Configuration is stored in RON format
//! for better Rust type expressiveness.

pub mod loader;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Top-level configuration for Forge
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct Config {
    #[serde(default)]
    pub extensions: Extensions,
}

/// Extension configuration section
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct Extensions {
    /// OCI-distributed extensions
    #[serde(default)]
    pub oci: Vec<OciExtension>,

    /// Local filesystem extensions
    #[serde(default)]
    pub local: Vec<LocalExtension>,

    /// Authentication configuration per registry
    #[serde(default)]
    pub auth: HashMap<String, RegistryAuth>,

    /// Extension system settings
    #[serde(default)]
    pub settings: Settings,
}

/// OCI-distributed extension configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct OciExtension {
    /// Extension name (used for logging and database paths)
    /// Must be a valid slug: lowercase, alphanumeric, hyphens only
    pub name: String,

    /// OCI registry hostname (e.g., "ghcr.io", "docker.io")
    pub registry: String,

    /// Image path within the registry (e.g., "forgepoint/extensions/github")
    pub image: String,

    /// Version reference (tag or digest)
    pub reference: Reference,
}

impl OciExtension {
    /// Validate the extension configuration
    pub fn validate(&self) -> Result<(), String> {
        validate_extension_name(&self.name)
    }
}

/// OCI image reference - either a tag or a content digest
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum Reference {
    /// Mutable tag reference (e.g., "v1.0.0", "latest")
    Tag(String),

    /// Immutable digest reference (e.g., "sha256:abc123...")
    Digest(String),
}

impl Reference {
    /// Get the reference string (tag or digest value)
    pub fn as_str(&self) -> &str {
        match self {
            Reference::Tag(tag) => tag,
            Reference::Digest(digest) => digest,
        }
    }

    /// Check if this is a digest reference (immutable)
    #[allow(dead_code)]
    pub fn is_digest(&self) -> bool {
        matches!(self, Reference::Digest(_))
    }
}

/// Local filesystem extension configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct LocalExtension {
    /// Extension name (used for logging and database paths)
    /// Must be a valid slug: lowercase, alphanumeric, hyphens only
    pub name: String,

    /// Path to the WASM file (absolute or relative to config file)
    pub path: PathBuf,
}

impl LocalExtension {
    /// Validate the extension configuration
    pub fn validate(&self) -> Result<(), String> {
        validate_extension_name(&self.name)
    }
}

/// Registry authentication configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct RegistryAuth {
    /// Environment variable name containing username
    pub username_env: Option<String>,

    /// Environment variable name containing access token/password
    pub token_env: Option<String>,
}

impl RegistryAuth {
    /// Resolve authentication credentials from environment variables
    pub fn resolve_credentials(&self) -> Option<(String, String)> {
        match (&self.username_env, &self.token_env) {
            (Some(user_env), Some(token_env)) => {
                let username = std::env::var(user_env).ok()?;
                let token = std::env::var(token_env).ok()?;
                Some((username, token))
            }
            _ => None,
        }
    }
}

/// Extension system settings
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Settings {
    /// Directory for caching OCI-fetched extensions
    pub cache_dir: Option<PathBuf>,

    /// If true, use cached extensions and don't fail on fetch errors
    #[serde(default)]
    pub offline_mode: bool,

    /// If true, verify checksums of cached extensions
    #[serde(default = "default_verify_checksums")]
    pub verify_checksums: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            cache_dir: Some(PathBuf::from(".forge/extensions/cache")),
            offline_mode: false,
            verify_checksums: true,
        }
    }
}

fn default_verify_checksums() -> bool {
    true
}

/// Validate extension name - must be a valid slug
fn validate_extension_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Extension name cannot be empty".to_string());
    }

    // Check for valid characters (lowercase alphanumeric and hyphens)
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(format!(
            "Extension name '{}' must contain only lowercase letters, numbers, and hyphens",
            name
        ));
    }

    // Cannot start or end with hyphen
    if name.starts_with('-') || name.ends_with('-') {
        return Err(format!(
            "Extension name '{}' cannot start or end with a hyphen",
            name
        ));
    }

    // Cannot contain consecutive hyphens
    if name.contains("--") {
        return Err(format!(
            "Extension name '{}' cannot contain consecutive hyphens",
            name
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.extensions.oci.is_empty());
        assert!(config.extensions.local.is_empty());
        assert!(config.extensions.auth.is_empty());
        assert!(!config.extensions.settings.offline_mode);
        assert!(config.extensions.settings.verify_checksums);
    }

    #[test]
    fn test_reference_is_digest() {
        let tag = Reference::Tag("v1.0.0".to_string());
        let digest = Reference::Digest("sha256:abc123".to_string());

        assert!(!tag.is_digest());
        assert!(digest.is_digest());
    }

    #[test]
    fn test_reference_as_str() {
        let tag = Reference::Tag("latest".to_string());
        assert_eq!(tag.as_str(), "latest");

        let digest = Reference::Digest("sha256:abc123".to_string());
        assert_eq!(digest.as_str(), "sha256:abc123");
    }

    #[test]
    fn test_registry_auth_resolve_credentials() {
        // Set test environment variables
        unsafe {
            std::env::set_var("TEST_USERNAME", "testuser");
            std::env::set_var("TEST_TOKEN", "testtoken");
        }

        let auth = RegistryAuth {
            username_env: Some("TEST_USERNAME".to_string()),
            token_env: Some("TEST_TOKEN".to_string()),
        };

        let creds = auth.resolve_credentials();
        assert_eq!(creds, Some(("testuser".to_string(), "testtoken".to_string())));

        // Clean up
        unsafe {
            std::env::remove_var("TEST_USERNAME");
            std::env::remove_var("TEST_TOKEN");
        }
    }

    #[test]
    fn test_registry_auth_missing_env_vars() {
        let auth = RegistryAuth {
            username_env: Some("NONEXISTENT_USER".to_string()),
            token_env: Some("NONEXISTENT_TOKEN".to_string()),
        };

        let creds = auth.resolve_credentials();
        assert_eq!(creds, None);
    }

    #[test]
    fn test_validate_extension_name_valid() {
        assert!(validate_extension_name("my-extension").is_ok());
        assert!(validate_extension_name("ext123").is_ok());
        assert!(validate_extension_name("my-ext-123").is_ok());
        assert!(validate_extension_name("a").is_ok());
    }

    #[test]
    fn test_validate_extension_name_invalid() {
        // Empty
        assert!(validate_extension_name("").is_err());

        // Uppercase
        assert!(validate_extension_name("MyExtension").is_err());

        // Special characters
        assert!(validate_extension_name("my_extension").is_err());
        assert!(validate_extension_name("my.extension").is_err());
        assert!(validate_extension_name("my/extension").is_err());

        // Leading/trailing hyphens
        assert!(validate_extension_name("-myext").is_err());
        assert!(validate_extension_name("myext-").is_err());

        // Consecutive hyphens
        assert!(validate_extension_name("my--ext").is_err());
    }

    #[test]
    fn test_oci_extension_validate() {
        let valid = OciExtension {
            name: "github-integration".to_string(),
            registry: "ghcr.io".to_string(),
            image: "forgepoint/extensions/github".to_string(),
            reference: Reference::Tag("v1.0.0".to_string()),
        };
        assert!(valid.validate().is_ok());

        let invalid = OciExtension {
            name: "Invalid_Name".to_string(),
            registry: "ghcr.io".to_string(),
            image: "test/ext".to_string(),
            reference: Reference::Tag("v1.0.0".to_string()),
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_local_extension_validate() {
        let valid = LocalExtension {
            name: "custom-extension".to_string(),
            path: PathBuf::from("./extensions/custom.wasm"),
        };
        assert!(valid.validate().is_ok());

        let invalid = LocalExtension {
            name: "invalid name".to_string(),
            path: PathBuf::from("./extensions/invalid.wasm"),
        };
        assert!(invalid.validate().is_err());
    }
}