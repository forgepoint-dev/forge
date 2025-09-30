use anyhow::Result;
use async_graphql::{
    Context, EmptySubscription, Object, Request, Response, Schema, Value,
};
use std::sync::Arc;
use sqlx::SqlitePool;

use crate::extensions::ExtensionManager;
use crate::repository::RepositoryStorage;

/// Federation gateway that combines core schema with extension subgraphs
pub struct FederationGateway {
    core_schema: Schema<CoreQuery, CoreMutation, EmptySubscription>,
    extensions: Vec<ExtensionSubgraph>,
}

/// Core query root (without extensions)
#[derive(Default)]
pub struct CoreQuery;

#[Object]
impl CoreQuery {
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

    /// Federation service field
    #[graphql(entity)]
    async fn find_entity(&self, _ctx: &Context<'_>, _representations: Vec<Value>) -> Vec<Value> {
        // Entity resolution for federation
        vec![]
    }
}

/// Core mutation root (without extensions)
#[derive(Default)]
pub struct CoreMutation;

#[Object]
impl CoreMutation {
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
}

/// Extension subgraph that wraps a WASM extension
pub struct ExtensionSubgraph {
    name: String,
    runtime: Arc<crate::extensions::wasm_runtime::Extension>,
}

impl ExtensionSubgraph {
    pub fn new(name: String, runtime: Arc<crate::extensions::wasm_runtime::Extension>) -> Self {
        Self { name, runtime }
    }

    /// Execute a GraphQL request against this subgraph
    pub async fn execute(&self, request: Request) -> Result<Response> {
        // Extract query and variables from the request
        let query = request.query.clone();
        let operation_name = request.operation_name.clone();
        let variables = if request.variables.is_empty() {
            None
        } else {
            Some(serde_json::to_value(&request.variables)?)
        };

        // Execute through the WASM runtime
        let result = self.runtime
            .execute_graphql(query, operation_name, variables)
            .await?;

        // Convert the result to an async_graphql Response
        let response = if let Some(errors) = result.get("errors") {
            let mut resp = Response::new(Value::Null);
            if let Some(errors_array) = errors.as_array() {
                for error in errors_array {
                    if let Some(message) = error.get("message").and_then(|m| m.as_str()) {
                        resp.errors.push(async_graphql::ServerError::new(message, None));
                    }
                }
            }
            if let Some(data) = result.get("data") {
                // Convert serde_json::Value to async_graphql::Value
                let json_str = serde_json::to_string(data)?;
                let value: Value = serde_json::from_str(&json_str)?;
                resp.data = value;
            }
            resp
        } else if let Some(data) = result.get("data") {
            // Convert serde_json::Value to async_graphql::Value
            let json_str = serde_json::to_string(data)?;
            let value: Value = serde_json::from_str(&json_str)?;
            Response::new(value)
        } else {
            Response::new(Value::Null)
        };

        Ok(response)
    }

    /// Get the SDL schema for this subgraph
    pub fn sdl(&self) -> String {
        self.runtime.schema().to_string()
    }
}

impl FederationGateway {
    pub fn new(
        pool: SqlitePool,
        storage: RepositoryStorage,
        extension_manager: ExtensionManager,
    ) -> Result<Self> {
        // Build core schema with federation support
        let core_schema = Schema::build(
            CoreQuery::default(),
            CoreMutation::default(),
            EmptySubscription,
        )
        .enable_federation()
        .data(pool)
        .data(storage)
        .finish();

        // Create subgraphs for each extension
        let mut extensions = Vec::new();
        for (name, extension) in extension_manager.get_extensions() {
            let subgraph = ExtensionSubgraph::new(
                name.clone(),
                extension.runtime.clone(),
            );
            extensions.push(subgraph);
        }

        Ok(Self {
            core_schema,
            extensions,
        })
    }

    /// Execute a request, routing to appropriate subgraph if needed
    pub async fn execute(&self, request: Request) -> Response {
        // For now, execute against the core schema
        // In a full federation setup, we'd use a query planner to route to subgraphs

        // Check if the query is targeting an extension
        if let Ok(doc) = async_graphql_parser::parse_query(&request.query) {
            // Simple heuristic: check if query contains extension fields
            // In production, use proper query planning
            for extension in &self.extensions {
                // Try executing against the extension
                if let Ok(response) = extension.execute(request.clone()).await {
                    if !response.is_err() {
                        return response;
                    }
                }
            }
        }

        // Fallback to core schema
        self.core_schema.execute(request).await
    }

    /// Get combined SDL for all schemas
    pub fn sdl(&self) -> String {
        let mut sdl = self.core_schema.sdl();

        for extension in &self.extensions {
            sdl.push_str("\n\n# Extension: ");
            sdl.push_str(&extension.name);
            sdl.push_str("\n");
            sdl.push_str(&extension.sdl());
        }

        sdl
    }
}

pub type FederatedSchema = FederationGateway;

/// Build the federated schema
pub fn build_federated_schema(
    pool: SqlitePool,
    storage: RepositoryStorage,
    extension_manager: ExtensionManager,
) -> Result<FederatedSchema> {
    FederationGateway::new(pool, storage, extension_manager)
}