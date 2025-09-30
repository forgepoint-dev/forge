use anyhow::Result;
use async_graphql::{Request, Response, Value};
use std::sync::Arc;
use std::collections::HashMap;
use sqlx::SqlitePool;

// use crate::extensions::wasm_runtime::GraphQLRequest; // Removed for simplified implementation

use hive_router_query_planner::planner::Planner;
use hive_router_query_planner::state::supergraph_state::SchemaDocument;
use hive_router_query_planner::ast::operation::OperationDefinition;

use crate::extensions::{ExtensionManager, Extension};
use crate::repository::RepositoryStorage;
use super::schema_composer::SchemaComposer;

/// Federation coordinator that manages query planning and execution across extensions
pub struct FederationCoordinator {
    planner: Planner,
    extensions: HashMap<String, Arc<Extension>>,
    pool: SqlitePool,
    storage: RepositoryStorage,
    supergraph_sdl: String,
}

impl FederationCoordinator {
    pub fn new(
        pool: SqlitePool,
        storage: RepositoryStorage,
        extension_manager: ExtensionManager,
    ) -> Result<Self> {
        // Compose supergraph from extensions
        let mut composer = SchemaComposer::new();

        let extensions = extension_manager.get_extensions();
        for (name, extension) in extensions {
            let schema_sdl = extension.runtime.schema();
            composer.add_subgraph(name.clone(), schema_sdl.to_string());
        }

        let supergraph_sdl = composer.compose()?;
        tracing::debug!("Composed supergraph SDL:\n{}", supergraph_sdl);

        // Parse the supergraph SDL
        let parsed_supergraph: SchemaDocument = graphql_parser::parse_schema(&supergraph_sdl)
            .map_err(|e| anyhow::anyhow!("Failed to parse composed supergraph: {}", e))?
            .into_static();

        // Create the query planner
        let planner = Planner::new_from_supergraph(&parsed_supergraph)
            .map_err(|e| anyhow::anyhow!("Failed to create query planner: {}", e))?;

        // Convert extensions to Arc for sharing
        let extensions: HashMap<String, Arc<Extension>> = extension_manager
            .get_extensions()
            .iter()
            .map(|(name, ext)| {
                (name.clone(), Arc::new(Extension {
                    name: ext.name.clone(),
                    db_path: ext.db_path.clone(),
                    schema: ext.schema.clone(),
                    runtime: ext.runtime.clone(),
                }))
            })
            .collect();

        Ok(Self {
            planner,
            extensions,
            pool,
            storage,
            supergraph_sdl,
        })
    }

    /// Execute a GraphQL request using federation
    pub async fn execute(&self, request: Request) -> Response {
        match self.execute_federated(request).await {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("Federation execution error: {}", e);
                let mut response = Response::new(Value::Null);
                response.errors.push(async_graphql::ServerError::new(
                    format!("Federation error: {}", e),
                    None,
                ));
                response
            }
        }
    }

    async fn execute_federated(&self, request: Request) -> Result<Response> {
        // Parse the incoming query
        let query_doc = graphql_parser::parse_query(&request.query)
            .map_err(|e| anyhow::anyhow!("Failed to parse query: {}", e))?;

        // Convert to internal operation format
        // This is simplified - in reality we'd need proper AST conversion
        let operation = self.convert_to_operation_definition(&query_doc)?;

        // Plan the query
        let query_plan = self.planner
            .plan_from_normalized_operation(&operation, Default::default())
            .map_err(|e| anyhow::anyhow!("Query planning failed: {}", e))?;

        tracing::debug!("Query plan: {:?}", query_plan);

        // Execute the query plan
        self.execute_query_plan(&query_plan, &request).await
    }

    fn convert_to_operation_definition(
        &self,
        query_doc: &graphql_parser::query::Document<String>,
    ) -> Result<OperationDefinition> {
        // Find the first operation definition in the document
        for definition in &query_doc.definitions {
            if let graphql_parser::query::Definition::Operation(operation) = definition {
                // Convert using the From implementation provided by hive-router
                return Ok(operation.clone().into());
            }
        }

        anyhow::bail!("No operation definition found in query document")
    }

    async fn execute_query_plan(
        &self,
        _query_plan: &hive_router_query_planner::planner::plan_nodes::QueryPlan,
        request: &Request,
    ) -> Result<Response> {
        // This is a simplified implementation
        // In reality, we'd execute the query plan step by step

        // For now, let's try to determine which extension to call based on the query
        if request.query.contains("getAllIssues") {
            // Route to issues extension for getAllIssues query
            if let Some(extension) = self.extensions.values().next() {
                let result = extension.runtime
                    .resolve_field("getAllIssues", "{}")
                    .await?;

                // Parse the JSON result and create GraphQL response
                let json_value: serde_json::Value = serde_json::from_str(&result)?;
                let graphql_value: Value = serde_json::from_str(&serde_json::to_string(&json_value)?)?;

                let mut resp = Response::new(Value::Null);

                // Create the response data structure
                let mut data_map = async_graphql::indexmap::IndexMap::new();
                data_map.insert(async_graphql::Name::new("getAllIssues"), graphql_value);
                resp.data = Value::Object(data_map);

                return Ok(resp);
            }
        }

        // Fallback to core schema execution
        self.execute_core_query(request).await
    }

    async fn execute_core_query(&self, _request: &Request) -> Result<Response> {
        // Execute core queries directly without extensions
        // This would use the original static schema approach for core fields

        let mut response = Response::new(Value::Null);
        response.errors.push(async_graphql::ServerError::new(
            "Core query execution not implemented yet",
            None,
        ));
        Ok(response)
    }

    /// Get the composed supergraph SDL
    pub fn supergraph_sdl(&self) -> &str {
        &self.supergraph_sdl
    }

    /// Get available extensions
    pub fn extensions(&self) -> &HashMap<String, Arc<Extension>> {
        &self.extensions
    }
}