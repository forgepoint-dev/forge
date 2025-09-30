//! Configuration file loading and parsing
//!
//! This module handles loading Forge configuration from RON files with
//! fallback strategies for finding config files in standard locations.

use super::Config;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Standard config file names to search for
const CONFIG_FILENAMES: &[&str] = &["forge.ron", ".forge/config.ron"];

/// Load configuration from a specific file path
pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Config> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;

    parse_ron(&content).with_context(|| format!("Failed to parse config file: {}", path.display()))
}

/// Load configuration with automatic file discovery
///
/// Searches for config files in the following locations (in order):
/// 1. Path specified in FORGE_CONFIG_PATH environment variable
/// 2. forge.ron in current directory
/// 3. .forge/config.ron relative to current directory
///
/// If no config file is found, returns a default configuration.
pub fn load_with_discovery() -> Result<Config> {
    // Check environment variable first
    if let Ok(env_path) = std::env::var("FORGE_CONFIG_PATH") {
        let path = PathBuf::from(env_path);
        if path.exists() {
            tracing::info!("Loading config from FORGE_CONFIG_PATH: {}", path.display());
            return load_from_file(&path);
        } else {
            tracing::warn!(
                "FORGE_CONFIG_PATH specified but file not found: {}",
                path.display()
            );
        }
    }

    // Search standard locations
    for filename in CONFIG_FILENAMES {
        let path = PathBuf::from(filename);
        if path.exists() {
            tracing::info!("Loading config from: {}", path.display());
            return load_from_file(&path);
        }
    }

    // No config file found, use defaults
    tracing::info!("No config file found, using defaults");
    Ok(Config::default())
}

/// Parse RON configuration string
fn parse_ron(content: &str) -> Result<Config> {
    ron::from_str(content).context("Failed to parse RON configuration")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Reference;
    use tempfile::TempDir;

    #[test]
    fn test_parse_minimal_config() {
        let ron = r#"
Config(
    extensions: Extensions(
        oci: [],
        local: [],
        auth: {},
        settings: Settings(
            cache_dir: None,
            offline_mode: false,
            verify_checksums: true,
        ),
    ),
)
        "#;

        let config = parse_ron(ron).unwrap();
        assert!(config.extensions.oci.is_empty());
        assert!(config.extensions.local.is_empty());
        assert!(!config.extensions.settings.offline_mode);
    }

    #[test]
    fn test_parse_full_config() {
        let ron = r#"
Config(
    extensions: Extensions(
        oci: [
            OciExtension(
                name: "github-integration",
                registry: "ghcr.io",
                image: "forgepoint/extensions/github",
                reference: Tag("v1.0.0"),
            ),
            OciExtension(
                name: "gitlab-integration",
                registry: "ghcr.io",
                image: "forgepoint/extensions/gitlab",
                reference: Digest("sha256:abc123"),
            ),
        ],
        local: [
            LocalExtension(
                name: "custom-extension",
                path: "./extensions/custom.wasm",
            ),
        ],
        auth: {
            "ghcr.io": RegistryAuth(
                username_env: Some("GHCR_USERNAME"),
                token_env: Some("GHCR_TOKEN"),
            ),
        },
        settings: Settings(
            cache_dir: Some(".forge/extensions/cache"),
            offline_mode: true,
            verify_checksums: true,
        ),
    ),
)
        "#;

        let config = parse_ron(ron).unwrap();

        // Check OCI extensions
        assert_eq!(config.extensions.oci.len(), 2);
        assert_eq!(config.extensions.oci[0].name, "github-integration");
        assert_eq!(config.extensions.oci[0].registry, "ghcr.io");
        assert_eq!(
            config.extensions.oci[0].image,
            "forgepoint/extensions/github"
        );
        assert_eq!(
            config.extensions.oci[0].reference,
            Reference::Tag("v1.0.0".to_string())
        );

        assert_eq!(config.extensions.oci[1].name, "gitlab-integration");
        assert_eq!(
            config.extensions.oci[1].reference,
            Reference::Digest("sha256:abc123".to_string())
        );

        // Check local extensions
        assert_eq!(config.extensions.local.len(), 1);
        assert_eq!(config.extensions.local[0].name, "custom-extension");

        // Check auth
        assert!(config.extensions.auth.contains_key("ghcr.io"));

        // Check settings
        assert!(config.extensions.settings.offline_mode);
        assert!(config.extensions.settings.verify_checksums);
    }

    #[test]
    fn test_load_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.ron");

        let ron_content = r#"
Config(
    extensions: Extensions(
        oci: [
            OciExtension(
                name: "test-extension",
                registry: "localhost:5000",
                image: "test/extension",
                reference: Tag("latest"),
            ),
        ],
        local: [],
        auth: {},
        settings: Settings(
            cache_dir: Some("/tmp/cache"),
            offline_mode: false,
            verify_checksums: true,
        ),
    ),
)
        "#;

        std::fs::write(&config_path, ron_content).unwrap();

        let config = load_from_file(&config_path).unwrap();
        assert_eq!(config.extensions.oci.len(), 1);
        assert_eq!(config.extensions.oci[0].name, "test-extension");
    }

    #[test]
    fn test_load_from_nonexistent_file() {
        let result = load_from_file("/nonexistent/path/config.ron");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_with_discovery_defaults() {
        // Make sure no env var is set
        unsafe {
            std::env::remove_var("FORGE_CONFIG_PATH");
        }

        // This should return default config since no file exists in temp location
        let config = load_with_discovery().unwrap();
        assert!(config.extensions.oci.is_empty());
        assert!(config.extensions.local.is_empty());
    }

    #[test]
    fn test_parse_invalid_ron() {
        let invalid_ron = "This is not valid RON";
        let result = parse_ron(invalid_ron);
        assert!(result.is_err());
    }
}