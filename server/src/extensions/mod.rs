//! WASM Extension System
//!
//! This module implements the WebAssembly-based extension system for the GraphQL API.
//! Extensions are WASM modules that can provide GraphQL schema fragments and handle
//! field resolution in a secure, isolated environment.

pub mod cache;
pub mod interface;
pub mod loader;
pub mod oci_fetcher;
pub mod schema;
pub mod wasm_runtime;
pub mod wit_bindings;

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Represents a loaded extension with its metadata and runtime state
#[allow(dead_code)] // Will be used when extension system is fully integrated
pub struct Extension {
    pub name: String,
    pub schema: schema::SchemaFragment,
    pub runtime: Arc<wasm_runtime::Extension>,
}

/// Extension manager coordinates loading and lifecycle management of extensions
pub struct ExtensionManager {
    extensions: HashMap<String, Extension>,
    extensions_dir: PathBuf,
    #[allow(dead_code)]
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
        let schema_manager = schema::builder::SchemaManager::new();
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

    /// Load extensions from configuration (OCI + local)
    pub async fn load_extensions_from_config(
        &mut self,
        config: &crate::config::Extensions,
    ) -> Result<()> {
        use oci_distribution::secrets::RegistryAuth;

        let mut extension_paths: Vec<(String, PathBuf)> = Vec::new();

        // 1. Fetch OCI extensions if configured
        if !config.oci.is_empty() {
            let cache_dir = config
                .settings
                .cache_dir
                .clone()
                .unwrap_or_else(|| PathBuf::from(".forge/extensions/cache"));

            let fetcher = oci_fetcher::OciExtensionFetcher::new(
                cache_dir,
                config.settings.offline_mode,
                config.settings.verify_checksums,
            )?;

            for oci_ext in &config.oci {
                // Validate extension name
                if let Err(e) = oci_ext.validate() {
                    tracing::error!("Invalid OCI extension name '{}': {}", oci_ext.name, e);
                    return Err(anyhow::anyhow!(
                        "Invalid OCI extension name '{}': {}",
                        oci_ext.name,
                        e
                    ));
                }

                // Resolve authentication
                let auth = config
                    .auth
                    .get(&oci_ext.registry)
                    .and_then(|registry_auth| registry_auth.resolve_credentials())
                    .map(|(username, password)| RegistryAuth::Basic(username, password));

                match fetcher
                    .fetch_extension(
                        &oci_ext.registry,
                        &oci_ext.image,
                        oci_ext.reference.as_str(),
                        auth.as_ref(),
                    )
                    .await
                {
                    Ok(path) => {
                        tracing::info!(
                            "Fetched OCI extension: {} from {}/{}:{}",
                            oci_ext.name,
                            oci_ext.registry,
                            oci_ext.image,
                            oci_ext.reference.as_str()
                        );
                        extension_paths.push((oci_ext.name.clone(), path));
                    }
                    Err(e) if config.settings.offline_mode => {
                        tracing::warn!(
                            "Skipping {} (offline mode, not cached): {}",
                            oci_ext.name,
                            e
                        );
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch OCI extension {}: {}", oci_ext.name, e);
                        return Err(e)
                            .context(format!("Failed to fetch OCI extension: {}", oci_ext.name));
                    }
                }
            }
        }

        // 2. Add local extensions
        for local_ext in &config.local {
            // Validate extension name
            if let Err(e) = local_ext.validate() {
                tracing::error!("Invalid local extension name '{}': {}", local_ext.name, e);
                return Err(anyhow::anyhow!(
                    "Invalid local extension name '{}': {}",
                    local_ext.name,
                    e
                ));
            }

            let path = if local_ext.path.is_absolute() {
                local_ext.path.clone()
            } else {
                // Resolve relative paths from current directory
                std::env::current_dir()?.join(&local_ext.path)
            };

            // Canonicalize and validate the path to prevent traversal attacks
            let canonical_path = match path.canonicalize() {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(
                        "Cannot canonicalize path for extension '{}' ({}): {}",
                        local_ext.name,
                        path.display(),
                        e
                    );
                    continue;
                }
            };

            // Verify it's a file (not a directory or symlink to something weird)
            if !canonical_path.is_file() {
                tracing::warn!(
                    "Extension path '{}' is not a regular file: {}",
                    local_ext.name,
                    canonical_path.display()
                );
                continue;
            }

            // Verify it has .wasm extension
            if canonical_path.extension().and_then(|s| s.to_str()) != Some("wasm") {
                tracing::warn!(
                    "Extension path '{}' does not have .wasm extension: {}",
                    local_ext.name,
                    canonical_path.display()
                );
                continue;
            }

            extension_paths.push((local_ext.name.clone(), canonical_path));
        }

        // 3. Load all extensions
        for (name, path) in extension_paths {
            match self.load_extension(&name, &path).await {
                Ok(ext) => {
                    self.extensions.insert(name.clone(), ext);
                    tracing::info!("Loaded extension: {}", name);
                }
                Err(e) => {
                    tracing::error!("Failed to load extension {}: {}", name, e);
                }
            }
        }

        if self.extensions.is_empty() {
            tracing::info!("No extensions loaded");
        } else {
            tracing::info!("Loaded {} extension(s)", self.extensions.len());
        }

        Ok(())
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

        let mut wasm_extensions_found = false;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                wasm_extensions_found = true;
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    match self.load_extension(name, &path).await {
                        Ok(extension) => {
                            tracing::info!("Loaded extension: {}", name);
                            self.extensions.insert(name.to_string(), extension);
                        }
                        Err(e) => {
                            tracing::error!("Failed to load extension {}: {:#}", name, e);
                        }
                    }
                }
            }
        }

        // Log if no extensions were found
        if !wasm_extensions_found {
            tracing::warn!("No WASM extensions found in {:?}", self.extensions_dir);
        }

        Ok(())
    }

    /// Load a single extension from a WASM file with enhanced safety
    async fn load_extension(&self, name: &str, wasm_path: &Path) -> Result<Extension> {
        tracing::info!("Loading extension: {}", name);

        // Create extension-specific directory for isolation
        let extension_dir = self.extensions_dir.join(name);
        std::fs::create_dir_all(&extension_dir).with_context(|| {
            format!("Failed to create extension directory: {:?}", extension_dir)
        })?;

        // Load the WASM extension using the new runtime
        let limits = loader::ExtensionLimits::default();
        let extension =
            wasm_runtime::Extension::load(wasm_path, &extension_dir, name.to_string(), &limits)
                .await
                .with_context(|| format!("Failed to load WASM extension: {}", name))?;

        // Get the schema from the extension
        let schema_sdl = extension.schema().to_string();

        // Parse the schema SDL into a SchemaFragment
        let schema = schema::SchemaFragment {
            federation_sdl: Some(schema_sdl.clone()),
            types: vec![],
        };

        tracing::info!(
            "Successfully loaded extension: {} with schema ({} bytes)",
            name,
            schema_sdl.len()
        );

        Ok(Extension {
            name: name.to_string(),
            schema,
            runtime: Arc::new(extension),
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
