//! Runtime implementation that bridges WIT bindings with the extension system

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::loader::ExtensionLimits;
use super::schema::SchemaFragment;
use super::wit_bindings::{ComponentExtension, ExtensionConfig, ExtensionInfo, ResolveInfo, ResolveResult};

/// Runtime extension that wraps the component-based extension
pub struct RuntimeExtension {
    name: String,
    component: Arc<RwLock<ComponentExtension>>,
    schema: String,
    info: ExtensionInfo,
    limits: ExtensionLimits,
}

impl RuntimeExtension {
    /// Load and initialize an extension
    pub async fn load(
        wasm_path: &Path,
        extension_dir: &Path,
        name: String,
        db_path: PathBuf,
        limits: ExtensionLimits,
    ) -> Result<Self> {
        // Load the component
        let mut component = ComponentExtension::load(
            &wasm_path.to_path_buf(),
            &extension_dir.to_path_buf(),
            name.clone(),
        ).await?;

        // Initialize with config
        let config = ExtensionConfig {
            name: name.clone(),
            version: "0.1.0".to_string(),
            database_path: db_path.to_string_lossy().to_string(),
            custom_config: None,
        };

        component.init(config).await
            .context("Failed to initialize extension")?;

        // Get info and schema
        let info = component.get_info().await
            .context("Failed to get extension info")?;

        let schema = component.get_schema().await
            .context("Failed to get extension schema")?;

        Ok(Self {
            name,
            component: Arc::new(RwLock::new(component)),
            schema,
            info,
            limits,
        })
    }

    /// Get the extension's GraphQL schema
    pub fn get_schema(&self) -> &str {
        &self.schema
    }

    /// Get the extension's info
    pub fn get_info(&self) -> &ExtensionInfo {
        &self.info
    }

    /// Resolve a GraphQL field
    pub async fn resolve_field(
        &self,
        field_name: String,
        parent_type: String,
        arguments: serde_json::Value,
        context: serde_json::Value,
        parent: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let info = ResolveInfo {
            field_name,
            parent_type,
            arguments,
            context,
            parent,
        };

        // Apply timeout
        let result = tokio::time::timeout(
            self.limits.operation_timeout,
            self.resolve_field_inner(info),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Extension {} timed out", self.name))??;

        match result {
            ResolveResult::Success(value) => Ok(value),
            ResolveResult::Error(err) => Err(anyhow::anyhow!("Extension error: {}", err)),
        }
    }

    async fn resolve_field_inner(&self, info: ResolveInfo) -> Result<ResolveResult> {
        let mut component = self.component.write().await;
        component.resolve_field(info).await
    }

    /// Execute a GraphQL query (if supported)
    pub async fn execute_graphql(
        &self,
        query: String,
        operation_name: Option<String>,
        variables: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        use super::wasm_runtime::{GraphQLRequest, GraphQLResponse};

        let request = GraphQLRequest {
            query,
            operation_name,
            variables,
        };

        let mut component = self.component.write().await;

        // Try to execute GraphQL directly if supported
        match component.execute_graphql(request).await {
            Ok(response) => {
                // Convert GraphQLResponse to a standard GraphQL JSON response
                Ok(serde_json::json!({
                    "data": response.data,
                    "errors": response.errors
                }))
            }
            Err(e) if e.to_string().contains("does not support GraphQL execution") => {
                // Fallback: Extension doesn't support direct GraphQL execution
                // Return an error indicating this
                Ok(serde_json::json!({
                    "data": null,
                    "errors": [{
                        "message": "Extension does not support direct GraphQL execution",
                        "extensions": {
                            "code": "UNSUPPORTED_OPERATION"
                        }
                    }]
                }))
            }
            Err(e) => Err(e),
        }
    }

    /// Shutdown the extension
    pub async fn shutdown(&self) -> Result<()> {
        let mut component = self.component.write().await;
        component.shutdown().await
    }
}

/// Parse a schema fragment from SDL
pub fn parse_schema_sdl(sdl: &str) -> Result<SchemaFragment> {
    // For now, we'll create an empty fragment
    // In a real implementation, we'd parse the SDL
    Ok(SchemaFragment::default())
}