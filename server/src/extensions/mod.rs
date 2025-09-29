//! WASM Extension System
//!
//! This module implements the WebAssembly-based extension system for the GraphQL API.
//! Extensions are WASM modules that can provide GraphQL schema fragments and handle
//! field resolution in a secure, isolated environment.

pub mod interface;
pub mod loader;
pub mod schema;

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;

/// Represents a loaded extension with its metadata and runtime state
#[derive(Debug, Clone)]
#[allow(dead_code)] // Will be used when extension system is fully integrated
pub struct Extension {
    pub name: String,
    pub db_path: PathBuf,
    pub schema: schema::SchemaFragment,
}

/// Extension manager coordinates loading and lifecycle management of extensions
pub struct ExtensionManager {
    extensions: HashMap<String, Extension>,
    extensions_dir: PathBuf,
    db_path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_extension_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let extensions_dir = temp_dir.path().join("extensions");
        let db_path = temp_dir.path().join("db");

        fs::create_dir_all(&extensions_dir).unwrap();
        fs::create_dir_all(&db_path).unwrap();

        let manager = ExtensionManager::new(extensions_dir.clone(), db_path.clone());
        assert_eq!(manager.extensions_dir, extensions_dir);
        assert_eq!(manager.db_path, db_path);
        assert!(manager.extensions.is_empty());
    }

    #[tokio::test]
    async fn test_load_extensions_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let extensions_dir = temp_dir.path().join("extensions");
        let db_path = temp_dir.path().join("db");

        fs::create_dir_all(&extensions_dir).unwrap();
        fs::create_dir_all(&db_path).unwrap();

        let mut manager = ExtensionManager::new(extensions_dir, db_path);
        let result = manager.load_extensions().await;

        assert!(result.is_ok());
        assert!(manager.extensions.is_empty());
    }

    #[test]
    fn test_extract_extension_name() {
        assert_eq!(
            ExtensionManager::extract_extension_name(&PathBuf::from("test.wasm")).unwrap(),
            "test"
        );
        assert_eq!(
            ExtensionManager::extract_extension_name(&PathBuf::from("complex-name.wasm")).unwrap(),
            "complex-name"
        );

        // Should fail for non-wasm files
        assert!(ExtensionManager::extract_extension_name(&PathBuf::from("test.txt")).is_err());
    }

    #[test]
    fn test_schema_manager_basic_operations() {
        let mut schema_manager = schema::builder::SchemaManager::new();
        assert!(!schema_manager.has_extensions());
        assert!(schema_manager.get_extension_schemas().is_empty());

        let merged = schema_manager.create_merged_schema_sdl();
        assert!(merged.contains("Merged GraphQL schema"));
    }
}

#[allow(dead_code)] // Extension system methods will be used when fully integrated
impl ExtensionManager {
    /// Create a new extension manager
    pub fn new(extensions_dir: PathBuf, db_path: PathBuf) -> Self {
        Self {
            extensions: HashMap::new(),
            extensions_dir,
            db_path,
        }
    }

    /// Extract extension name from WASM file path
    fn extract_extension_name(wasm_path: &PathBuf) -> Result<String> {
        // Check if file has .wasm extension
        if wasm_path.extension().and_then(|s| s.to_str()) != Some("wasm") {
            return Err(anyhow::anyhow!(
                "File is not a WASM module: {:?}",
                wasm_path
            ));
        }

        // Extract the filename without extension
        wasm_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Invalid filename: {:?}", wasm_path))
    }

    /// Load all WASM extensions from the extensions directory
    pub async fn load_extensions(&mut self) -> Result<()> {
        // Ensure extensions directory exists
        if !self.extensions_dir.exists() {
            std::fs::create_dir_all(&self.extensions_dir).with_context(|| {
                format!(
                    "Failed to create extensions directory: {:?}",
                    self.extensions_dir
                )
            })?;
        }

        // Scan for .wasm files
        let entries = std::fs::read_dir(&self.extensions_dir).with_context(|| {
            format!(
                "Failed to read extensions directory: {:?}",
                self.extensions_dir
            )
        })?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    match self.load_extension(name, &path).await {
                        Ok(extension) => {
                            tracing::info!("Loaded extension: {}", name);
                            self.extensions.insert(name.to_string(), extension);
                        }
                        Err(e) => {
                            tracing::error!("Failed to load extension {}: {}", name, e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Load a single extension from a WASM file with enhanced safety
    async fn load_extension(&self, name: &str, wasm_path: &PathBuf) -> Result<Extension> {
        tracing::info!("Loading extension: {}", name);

        // Create extension-specific database path
        let db_path = self.db_path.join(format!("{}.extension.db", name));

        // Create extension-specific directory for isolation
        let extension_dir = self.extensions_dir.join(name);
        std::fs::create_dir_all(&extension_dir).with_context(|| {
            format!("Failed to create extension directory: {:?}", extension_dir)
        })?;

        // Load and initialize the WASM module with security constraints
        let mut extension_instance = loader::load_wasm_module(wasm_path, &extension_dir)
            .await
            .with_context(|| format!("Failed to load WASM module for extension: {}", name))?;

        // Set extension name for metrics and logging
        extension_instance.set_name(name.to_string());

        // Initialize the extension with API version and capabilities
        let config = interface::ExtensionConfig {
            name: name.to_string(),
            db_path: db_path.to_string_lossy().to_string(),
            config: None,
            api_version: "0.1.0".to_string(),
            capabilities: vec!["basic".to_string(), "database".to_string()],
        };

        extension_instance
            .init(&config)
            .await
            .with_context(|| format!("Failed to initialize extension: {}", name))?;

        // Run database migrations
        extension_instance
            .migrate(&db_path.to_string_lossy())
            .await
            .with_context(|| format!("Failed to run migrations for extension: {}", name))?;

        // Get and validate the GraphQL schema
        let schema = extension_instance
            .get_schema()
            .await
            .with_context(|| format!("Failed to get schema from extension: {}", name))?;

        tracing::info!("Successfully loaded extension: {}", name);

        Ok(Extension {
            name: name.to_string(),
            db_path,
            schema,
        })
    }

    /// Get all loaded extensions
    pub fn get_extensions(&self) -> &HashMap<String, Extension> {
        &self.extensions
    }

    /// Get merged GraphQL schema from all extensions
    pub fn get_merged_schema(&self) -> String {
        let mut merged = String::new();

        for extension in self.extensions.values() {
            if !extension.schema.is_empty() {
                merged.push_str(&extension.schema.to_sdl());
                merged.push('\n');
            }
        }

        merged
    }
}
