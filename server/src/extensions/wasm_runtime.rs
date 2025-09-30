//! WASM runtime wrapper for extensions
//!
//! This module provides a high-level wrapper around the component model
//! extension runtime, making it easier to use from the rest of the codebase.

use anyhow::{Context, Result};
use std::path::Path;
use std::sync::{Arc, Mutex};

use super::loader::ExtensionLimits;
use super::wit_bindings::{ComponentExtension, ExtensionConfig, ExtensionInfo, ResolveInfo, ResolveResult};

/// High-level extension wrapper with runtime management
/// Uses Mutex to ensure Store<ExtensionState> is Send+Sync safe
#[derive(Clone)]
pub struct Extension {
    #[allow(dead_code)]
    component: Arc<Mutex<ComponentExtension>>,
    schema: String,
    #[allow(dead_code)]
    info: ExtensionInfo,
}

impl Extension {
    /// Load an extension from a WASM file
    pub async fn load(
        wasm_path: &Path,
        extension_dir: &Path,
        name: String,
        _limits: &ExtensionLimits,
    ) -> Result<Self> {
        // Ensure extension directory exists
        std::fs::create_dir_all(extension_dir)
            .context("Failed to create extension directory")?;

        // Canonicalize to get absolute path
        let extension_dir_abs = extension_dir.canonicalize()
            .context("Failed to canonicalize extension directory")?;

        // Create database path (absolute)
        let db_path = extension_dir_abs.join(format!("{}.db", name));
        tracing::debug!("Extension database path: {}", db_path.display());

        // Initialize database connection BEFORE loading WASM component
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::str::FromStr;

        let connect_options = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))?
            .create_if_missing(true)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(connect_options)
            .await
            .context("Failed to connect to extension database")?;

        // Load component in a blocking task to avoid runtime conflicts
        let wasm_path_buf = wasm_path.to_path_buf();
        let extension_dir_buf = extension_dir_abs.to_path_buf();
        let name_clone = name.clone();
        let db_path_str = db_path.to_string_lossy().to_string();

        let (component, info, schema) = tokio::task::spawn_blocking(move || {
            // Load the component extension and pass the pre-initialized pool
            let mut component = ComponentExtension::load(
                &wasm_path_buf,
                &extension_dir_buf,
                name_clone.clone(),
                pool,
            )
            .context("Failed to load WASM component")?;

            // Initialize the extension
            let config = ExtensionConfig {
                name: name_clone,
                version: "0.1.0".to_string(),
                database_path: db_path_str,
                custom_config: None,
            };

            component
                .init(config)
                .context("Failed to initialize extension")?;

            // Get extension info and schema
            let info = component
                .get_info()
                .context("Failed to get extension info")?;

            let schema = component
                .get_schema()
                .context("Failed to get extension schema")?;

            Ok::<_, anyhow::Error>((component, info, schema))
        })
        .await
        .context("Blocking task panicked")??;

        tracing::info!(
            "Loaded extension '{}' v{} with schema ({} bytes)",
            info.name,
            info.version,
            schema.len()
        );

        Ok(Self {
            component: Arc::new(Mutex::new(component)),
            schema,
            info,
        })
    }

    /// Load an extension with a pre-configured database pool (for testing)
    pub async fn load_with_pool(
        wasm_path: &Path,
        extension_dir: &Path,
        name: String,
        _pool: sqlx::SqlitePool,
        limits: &ExtensionLimits,
    ) -> Result<Self> {
        // For now, just use the regular load method
        // The component will create its own database connection
        Self::load(wasm_path, extension_dir, name, limits).await
    }

    /// Get the extension name
    #[allow(dead_code)]
    pub fn name(&self) -> &str {
        &self.info.name
    }

    /// Get the extension version
    #[allow(dead_code)]
    pub fn version(&self) -> &str {
        &self.info.version
    }

    /// Get the extension capabilities
    #[allow(dead_code)]
    pub fn capabilities(&self) -> &[String] {
        &self.info.capabilities
    }

    /// Get the extension's GraphQL schema
    pub fn schema(&self) -> &str {
        &self.schema
    }

    /// Resolve a GraphQL field
    #[allow(dead_code)]
    pub async fn resolve_field(
        &self,
        field_name: String,
        parent_type: String,
        arguments: serde_json::Value,
        context: serde_json::Value,
        parent: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        // Create resolve info
        let resolve_info = ResolveInfo {
            field_name,
            parent_type,
            arguments,
            context,
            parent,
        };

        // Call the component in a blocking task (Mutex ensures thread safety)
        let component = self.component.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut comp = component.lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock component: {}", e))?;
            comp.resolve_field(resolve_info)
                .context("Failed to resolve field in extension")
        })
        .await
        .context("Blocking task panicked")??;

        match result {
            ResolveResult::Success(value) => Ok(value),
            ResolveResult::Error(err) => Err(anyhow::anyhow!("Extension error: {}", err)),
        }
    }

    /// Shutdown the extension
    #[allow(dead_code)]
    pub fn shutdown(&self) -> Result<()> {
        let mut component = self.component.lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock component: {}", e))?;
        component.shutdown()
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_extension_lifecycle() {
        // This test requires a real WASM file, which we'll need to build
        // For now, we'll skip it
    }
}