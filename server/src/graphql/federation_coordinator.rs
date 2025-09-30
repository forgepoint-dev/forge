use anyhow::Result;
use async_graphql::{Request, Response, Value};
use std::sync::Arc;
use std::collections::HashMap;
use sqlx::SqlitePool;

use hive_router_query_planner::planner::Planner;
use hive_router_query_planner::state::supergraph_state::SchemaDocument;
use hive_router_query_planner::ast::operation::OperationDefinition;

use crate::extensions::{ExtensionManager, Extension};
use crate::repository::RepositoryStorage;
use super::schema_composer::SchemaComposer;

/// Federation coordinator that manages query planning and execution across extensions
///
/// NOTE: This is an experimental implementation of GraphQL federation for extensions.
/// The main schema currently uses ExtensionFieldRegistry for simpler dynamic field resolution.
/// Full federation support (entity resolution, type merging, etc.) is not yet complete.
#[allow(dead_code)]
pub struct FederationCoordinator {
    planner: Planner,
    extensions: HashMap<String, Arc<Extension>>,
    pool: SqlitePool,
    storage: RepositoryStorage,
    supergraph_sdl: String,
}

impl FederationCoordinator {
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
        _request: &Request,
    ) -> Result<Response> {
        // TODO: Implement proper query plan execution
        // This would involve:
        // 1. Walking the query plan nodes
        // 2. Executing each step (Fetch, Flatten, Parallel, Sequence)
        // 3. Merging results from multiple extensions
        // 4. Handling entity resolution for federated types
        //
        // For now, this is a placeholder. The main schema uses ExtensionFieldRegistry
        // for dynamic field resolution instead of full federation.

        let mut response = Response::new(Value::Null);
        response.errors.push(async_graphql::ServerError::new(
            "Federation query planning not fully implemented. Use build_schema() instead of create_federated_schema().",
            None,
        ));
        Ok(response)
    }


    /// Get the composed supergraph SDL
    #[allow(dead_code)]
    pub fn supergraph_sdl(&self) -> &str {
        &self.supergraph_sdl
    }

    /// Get available extensions
    #[allow(dead_code)]
    pub fn extensions(&self) -> &HashMap<String, Arc<Extension>> {
        &self.extensions
    }
}