//! WASM Extension System
//! 
//! This module implements the WebAssembly-based extension system for the GraphQL API.
//! Extensions are WASM modules that can provide GraphQL schema fragments and handle
//! field resolution in a secure, isolated environment.

pub mod loader;
pub mod interface;
pub mod schema;

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;

/// Represents a loaded extension with its metadata and runtime state
#[derive(Debug)]
pub struct Extension {
    pub name: String,
    pub db_path: PathBuf,
    pub schema_sdl: String,
}

/// Extension manager coordinates loading and lifecycle management of extensions
pub struct ExtensionManager {
    extensions: HashMap<String, Extension>,
    extensions_dir: PathBuf,
    db_path: PathBuf,
}

impl ExtensionManager {
    /// Create a new extension manager
    pub fn new(extensions_dir: PathBuf, db_path: PathBuf) -> Self {
        Self {
            extensions: HashMap::new(),
            extensions_dir,
            db_path,
        }
    }

    /// Load all WASM extensions from the extensions directory
    pub async fn load_extensions(&mut self) -> Result<()> {
        // Ensure extensions directory exists
        if !self.extensions_dir.exists() {
            std::fs::create_dir_all(&self.extensions_dir)
                .with_context(|| format!("Failed to create extensions directory: {:?}", self.extensions_dir))?;
        }

        // Scan for .wasm files
        let entries = std::fs::read_dir(&self.extensions_dir)
            .with_context(|| format!("Failed to read extensions directory: {:?}", self.extensions_dir))?;

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

    /// Load a single extension from a WASM file
    async fn load_extension(&self, name: &str, wasm_path: &PathBuf) -> Result<Extension> {
        // Create extension-specific database path
        let db_path = self.db_path.join(format!("{}.extension.db", name));
        
        // Load and initialize the WASM module
        let mut extension_instance = loader::load_wasm_module(wasm_path).await?;
        
        // Initialize the extension
        let config = interface::ExtensionConfig {
            name: name.to_string(),
            db_path: db_path.to_string_lossy().to_string(),
            config: None,
        };

        extension_instance.init(&config)
            .await
            .map_err(|e| anyhow::anyhow!("Extension init failed: {}", e))?;

        // Run migrations
        extension_instance.migrate(&db_path.to_string_lossy())
            .await
            .map_err(|e| anyhow::anyhow!("Extension migration failed: {}", e))?;

        // Get schema
        let schema_sdl = extension_instance.get_schema().await?;

        Ok(Extension {
            name: name.to_string(),
            db_path,
            schema_sdl,
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
            if !extension.schema_sdl.is_empty() {
                merged.push_str(&extension.schema_sdl);
                merged.push('\n');
            }
        }
        
        merged
    }
}