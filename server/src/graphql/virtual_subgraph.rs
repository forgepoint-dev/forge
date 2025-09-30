use anyhow::Result;
use async_graphql::{Context, EmptySubscription, Object, Schema, Value, Json};
use std::sync::Arc;
use sqlx::SqlitePool;

use crate::extensions::ExtensionManager;
use crate::repository::RepositoryStorage;
use super::extension_resolver::ExtensionFieldRegistry;

/// Virtual subgraph approach - simpler than full federation
/// This creates a single schema that routes to extensions without dynamic schema manipulation

#[derive(Default)]
pub struct VirtualQuery;

#[Object]
impl VirtualQuery {
    // Core queries remain static
    async fn get_all_groups(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<crate::group::GroupNode>> {
        crate::group::queries::get_all_groups(ctx).await
    }

    async fn get_all_repositories(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<crate::repository::RepositoryNode>> {
        crate::repository::queries::get_all_repositories(ctx).await
    }

    async fn get_group(&self, ctx: &Context<'_>, path: String) -> async_graphql::Result<Option<crate::group::GroupNode>> {
        crate::group::queries::get_group(ctx, path).await
    }

    async fn get_repository(&self, ctx: &Context<'_>, path: String) -> async_graphql::Result<Option<crate::repository::RepositoryNode>> {
        crate::repository::queries::get_repository(ctx, path).await
    }

    async fn browse_repository(
        &self,
        ctx: &Context<'_>,
        path: String,
        #[graphql(name = "treePath")] tree_path: Option<String>,
    ) -> async_graphql::Result<Option<crate::repository::RepositoryEntriesPayload>> {
        crate::repository::queries::browse_repository(ctx, path, tree_path).await
    }

    // Extension query handler - generic JSON in/out
    #[graphql(name = "extension")]
    async fn extension(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Extension name")] name: String,
        #[graphql(desc = "Operation name")] operation: String,
        #[graphql(desc = "Arguments as JSON")] args: Option<Json<serde_json::Value>>,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        let registry = ctx.data::<Arc<ExtensionFieldRegistry>>()?;

        // Convert arguments to async_graphql Value
        let args_value = if let Some(Json(args)) = args {
            // Convert serde_json::Value to async_graphql::Value
            let json_str = serde_json::to_string(&args)
                .map_err(|e| async_graphql::Error::new(e.to_string()))?;
            serde_json::from_str::<Value>(&json_str)
                .map_err(|e| async_graphql::Error::new(e.to_string()))?
        } else {
            Value::Null
        };

        // Resolve through the extension
        let result = registry
            .resolve_field("Query", &operation, args_value, Value::Null)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        // Convert back to serde_json::Value
        let json_str = serde_json::to_string(&result)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        let json_value = serde_json::from_str(&json_str)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(Json(json_value))
    }
}

#[derive(Default)]
pub struct VirtualMutation;

#[Object]
impl VirtualMutation {
    async fn create_group(
        &self,
        ctx: &Context<'_>,
        input: crate::group::CreateGroupInput,
    ) -> async_graphql::Result<crate::group::GroupNode> {
        crate::group::mutations::create_group(ctx, input).await
    }

    async fn create_repository(
        &self,
        ctx: &Context<'_>,
        input: crate::repository::CreateRepositoryInput,
    ) -> async_graphql::Result<crate::repository::RepositoryNode> {
        crate::repository::mutations::create_repository(ctx, input).await
    }

    async fn link_remote_repository(
        &self,
        ctx: &Context<'_>,
        url: String,
    ) -> async_graphql::Result<crate::repository::RepositoryNode> {
        crate::repository::mutations::link_remote_repository(ctx, url).await
    }

    // Extension mutation handler
    #[graphql(name = "extension")]
    async fn extension(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Extension name")] name: String,
        #[graphql(desc = "Operation name")] operation: String,
        #[graphql(desc = "Arguments as JSON")] args: Option<Json<serde_json::Value>>,
    ) -> async_graphql::Result<Json<serde_json::Value>> {
        let registry = ctx.data::<Arc<ExtensionFieldRegistry>>()?;

        let args_value = if let Some(Json(args)) = args {
            let json_str = serde_json::to_string(&args)
                .map_err(|e| async_graphql::Error::new(e.to_string()))?;
            serde_json::from_str::<Value>(&json_str)
                .map_err(|e| async_graphql::Error::new(e.to_string()))?
        } else {
            Value::Null
        };

        let result = registry
            .resolve_field("Mutation", &operation, args_value, Value::Null)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        let json_str = serde_json::to_string(&result)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        let json_value = serde_json::from_str(&json_str)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(Json(json_value))
    }
}

pub type VirtualSchema = Schema<VirtualQuery, VirtualMutation, EmptySubscription>;

/// Build the virtual subgraph schema
pub fn build_virtual_schema(
    pool: SqlitePool,
    storage: RepositoryStorage,
    extension_manager: ExtensionManager,
) -> VirtualSchema {
    let ext_manager_arc = Arc::new(extension_manager);
    let mut registry = ExtensionFieldRegistry::new(ext_manager_arc.clone());

    // Register extension fields
    if let Err(e) = registry.register_extensions() {
        tracing::warn!("Failed to register extension fields: {}", e);
    }

    Schema::build(
        VirtualQuery::default(),
        VirtualMutation::default(),
        EmptySubscription,
    )
    .data(pool)
    .data(storage)
    .data(ext_manager_arc)
    .data(Arc::new(registry))
    .finish()
}