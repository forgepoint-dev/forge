use async_graphql::{Context, EmptySubscription, Object, Schema};
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::extensions::ExtensionManager;
use super::extension_resolver::ExtensionFieldRegistry;
use crate::group::{CreateGroupInput, GroupNode};
use crate::repository::{
    CreateRepositoryInput, RepositoryEntriesPayload, RepositoryNode, RepositoryStorage,
};

#[derive(Default)]
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn get_all_groups(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<GroupNode>> {
        crate::group::queries::get_all_groups(ctx).await
    }


    async fn get_all_repositories(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<RepositoryNode>> {
        crate::repository::queries::get_all_repositories(ctx).await
    }

    async fn get_group(
        &self,
        ctx: &Context<'_>,
        path: String,
    ) -> async_graphql::Result<Option<GroupNode>> {
        crate::group::queries::get_group(ctx, path).await
    }

    async fn get_repository(
        &self,
        ctx: &Context<'_>,
        path: String,
    ) -> async_graphql::Result<Option<RepositoryNode>> {
        crate::repository::queries::get_repository(ctx, path).await
    }

    async fn browse_repository(
        &self,
        ctx: &Context<'_>,
        path: String,
        #[graphql(name = "treePath")] tree_path: Option<String>,
    ) -> async_graphql::Result<Option<RepositoryEntriesPayload>> {
        crate::repository::queries::browse_repository(ctx, path, tree_path).await
    }
}

#[derive(Default)]
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn create_group(
        &self,
        ctx: &Context<'_>,
        input: CreateGroupInput,
    ) -> async_graphql::Result<GroupNode> {
        crate::group::mutations::create_group(ctx, input).await
    }

    async fn create_repository(
        &self,
        ctx: &Context<'_>,
        input: CreateRepositoryInput,
    ) -> async_graphql::Result<RepositoryNode> {
        crate::repository::mutations::create_repository(ctx, input).await
    }

    async fn link_remote_repository(
        &self,
        ctx: &Context<'_>,
        url: String,
    ) -> async_graphql::Result<RepositoryNode> {
        crate::repository::mutations::link_remote_repository(ctx, url).await
    }
}

pub type AppSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub fn build_schema(
    pool: SqlitePool,
    storage: RepositoryStorage,
    extension_manager: ExtensionManager,
) -> AppSchema {
    // Initialize extension field registry
    let ext_manager_arc = Arc::new(extension_manager);
    let mut registry = ExtensionFieldRegistry::new(ext_manager_arc.clone());

    // Register extension fields
    if let Err(e) = registry.register_extensions() {
        tracing::warn!("Failed to register extension fields: {}", e);
    }

    Schema::build(
        QueryRoot::default(),
        MutationRoot::default(),
        EmptySubscription,
    )
    .data(pool)
    .data(storage)
    .data(ext_manager_arc)
    .data(Arc::new(registry))
    .finish()
}

/// Create the main GraphQL schema using federation
///
/// NOTE: Experimental - Federation support is incomplete. Entity resolution, type merging,
/// and distributed query planning are not fully implemented. Use `build_schema()` instead
/// for production workloads.
#[allow(dead_code)]
pub async fn create_federated_schema(
    pool: SqlitePool,
    storage: RepositoryStorage,
    extension_manager: ExtensionManager,
) -> anyhow::Result<AppSchema> {
    use super::federation_coordinator::FederationCoordinator;
    use anyhow::Context;

    // Create federation coordinator
    let coordinator = Arc::new(
        FederationCoordinator::new(pool.clone(), storage.clone(), extension_manager)
            .context("Failed to create federation coordinator")?
    );

    // Build the federated schema with coordinator as data
    let schema = Schema::build(
        QueryRoot::default(),
        MutationRoot::default(),
        EmptySubscription,
    )
    .data(pool)
    .data(storage)
    .data(coordinator)
    .finish();

    Ok(schema)
}
