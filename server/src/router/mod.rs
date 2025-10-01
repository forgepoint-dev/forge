mod core_executor;
mod extension_executor;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use hive_router_plan_executor::execute_query_plan;
use hive_router_plan_executor::execution::plan::QueryPlanExecutionContext;
use hive_router_plan_executor::executors::{common::SubgraphExecutor, map::SubgraphExecutorMap};
use hive_router_plan_executor::introspection::partition::partition_operation;
use hive_router_plan_executor::introspection::resolve::IntrospectionContext;
use hive_router_plan_executor::introspection::schema::{SchemaMetadata, SchemaWithMetadata};
use hive_router_plan_executor::projection::plan::FieldProjectionPlan;
use hive_router_plan_executor::variables::collect_variables;
use hive_router_query_planner::ast::normalization::normalize_operation;
use hive_router_query_planner::planner::{Planner, plan_nodes::QueryPlan};
use hive_router_query_planner::state::supergraph_state::SchemaDocument;
use hive_router_query_planner::utils::{
    cancellation::CancellationToken, parsing::safe_parse_operation,
};
use serde_json::Value as JsonValue;
use sonic_rs::Value as SonicValue;
use sqlx::SqlitePool;

use crate::extensions::ExtensionManager;
use crate::extensions::wit_bindings::GlobalContext;
use crate::graphql::schema_composer::SchemaComposer;
use crate::repository::RepositoryStorage;

use self::core_executor::CoreSubgraphExecutor;
use self::extension_executor::ExtensionSubgraphExecutor;

/// Coordinates query planning and execution using Hive Router's planner and executor stacks.
pub struct RouterState {
    planner: Planner,
    schema_metadata: SchemaMetadata,
    subgraph_executors: Arc<SubgraphExecutorMap>,
}

impl RouterState {
    pub fn new(
        pool: SqlitePool,
        storage: RepositoryStorage,
        extension_manager: Arc<ExtensionManager>,
    ) -> Result<Self> {
        // Compose the supergraph SDL from core + extensions
        let mut composer = SchemaComposer::new();
        for (name, extension) in extension_manager.get_extensions() {
            let schema_sdl = extension.runtime.schema();
            composer
                .add_subgraph(name.clone(), schema_sdl.to_string())
                .with_context(|| format!("failed to register schema for extension `{}`", name))?;
        }
        let supergraph_sdl = composer.compose()?;

        // Parse SDL and initialise planner
        let parsed_supergraph: SchemaDocument = graphql_parser::parse_schema(&supergraph_sdl)
            .map_err(|e| anyhow!("Failed to parse composed supergraph: {e}"))?
            .into_static();
        let planner = Planner::new_from_supergraph(&parsed_supergraph)
            .map_err(|e| anyhow!("Failed to create query planner: {e}"))?;

        // Build schema metadata used by executor for projection / validation
        let schema_metadata = planner.consumer_schema.schema_metadata();

        let mut executor_map = SubgraphExecutorMap::new();
        executor_map.insert_boxed_arc(
            "CORE".to_string(),
            CoreSubgraphExecutor::new(pool.clone(), storage.clone()).to_boxed_arc(),
        );
        executor_map.insert_boxed_arc(
            "core".to_string(),
            CoreSubgraphExecutor::new(pool.clone(), storage.clone()).to_boxed_arc(),
        );

        let global_context = GlobalContext::default();

        for (name, extension) in extension_manager.get_extensions() {
            let executor = ExtensionSubgraphExecutor::new(
                name.clone(),
                extension.runtime.clone(),
                extension.runtime.schema(),
                pool.clone(),
                global_context.clone(),
            )
            .with_context(|| format!("failed to initialise executor for extension `{}`", name))?;
            executor_map.insert_boxed_arc(name.clone(), executor.to_boxed_arc());

            let upper_name = name.to_uppercase();
            let executor_upper = ExtensionSubgraphExecutor::new(
                upper_name.clone(),
                extension.runtime.clone(),
                extension.runtime.schema(),
                pool.clone(),
                global_context.clone(),
            )
            .with_context(|| format!("failed to initialise executor for extension `{}`", name))?;
            executor_map.insert_boxed_arc(upper_name, executor_upper.to_boxed_arc());
        }

        Ok(Self {
            planner,
            schema_metadata,
            subgraph_executors: Arc::new(executor_map),
        })
    }

    /// Execute a GraphQL request and return the GraphQL response JSON.
    pub async fn execute(&self, request: GraphQLExecutionRequest) -> Result<JsonValue> {
        let operation_name = request.operation_name.clone();

        let parsed_operation = safe_parse_operation(&request.query)
            .map_err(|e| anyhow!("Failed to parse query: {e}"))?;

        let normalized = normalize_operation(
            &self.planner.supergraph,
            &parsed_operation,
            operation_name.as_deref(),
        )
        .map_err(|e| anyhow!("Failed to normalize query: {e}"))?;

        let (root_type_name, projection_plan) =
            FieldProjectionPlan::from_operation(&normalized.operation, &self.schema_metadata);

        let partitioned = partition_operation(normalized.operation.clone());

        let variable_values = collect_variables(
            &partitioned.downstream_operation,
            request.variables.clone(),
            &self.schema_metadata,
        )
        .map_err(|err| anyhow!("Failed to collect variables: {err}"))?;

        let query_plan = if partitioned
            .downstream_operation
            .selection_set
            .items
            .is_empty()
        {
            QueryPlan {
                kind: "QueryPlan".to_string(),
                node: None,
            }
        } else {
            let cancellation_token = CancellationToken::new();
            self.planner
                .plan_from_normalized_operation(
                    &partitioned.downstream_operation,
                    Default::default(),
                    &cancellation_token,
                )
                .map_err(|e| anyhow!("Query planning failed: {e}"))?
        };

        let introspection_context = IntrospectionContext {
            query: partitioned.introspection_operation.as_ref(),
            schema: &self.planner.consumer_schema.document,
            metadata: &self.schema_metadata,
        };

        let execution_context = QueryPlanExecutionContext {
            query_plan: &query_plan,
            projection_plan: &projection_plan,
            variable_values: &variable_values,
            extensions: None,
            introspection_context: &introspection_context,
            operation_type_name: root_type_name,
            executors: &self.subgraph_executors,
        };

        match execute_query_plan(execution_context).await {
            Ok(bytes) => {
                let json: JsonValue = serde_json::from_slice(&bytes)
                    .map_err(|e| anyhow!("Executor produced invalid JSON: {e}"))?;
                Ok(json)
            }
            Err(err) => {
                let error_body = graphql_error_body(JsonValue::String(err.to_string()));
                Ok(error_body)
            }
        }
    }
}

/// Representation of a GraphQL execution request with variables already converted to `sonic_rs` values.
pub struct GraphQLExecutionRequest {
    pub query: String,
    pub operation_name: Option<String>,
    pub variables: Option<HashMap<String, SonicValue>>,
}

impl GraphQLExecutionRequest {
    pub fn from_payload(payload: &crate::api::server::GraphQLRequest) -> Result<Self> {
        let variables = match &payload.variables {
            JsonValue::Null => None,
            JsonValue::Object(map) => {
                let mut out = HashMap::with_capacity(map.len());
                for (k, v) in map {
                    let sonic = sonic_rs::to_value(v)
                        .map_err(|e| anyhow!("Invalid variable value for {k}: {e}"))?;
                    out.insert(k.clone(), sonic);
                }
                Some(out)
            }
            _ => {
                return Err(anyhow!("variables payload must be an object or null"));
            }
        };

        Ok(Self {
            query: payload.query.clone(),
            operation_name: payload.operation_name.clone(),
            variables,
        })
    }
}

pub(super) fn graphql_error_body(message: JsonValue) -> JsonValue {
    JsonValue::Object(serde_json::Map::from_iter([(
        "errors".to_string(),
        JsonValue::Array(vec![JsonValue::Object(serde_json::Map::from_iter([(
            "message".to_string(),
            message,
        )]))]),
    )]))
}

pub(super) fn sonic_to_serde(value: &SonicValue) -> Result<JsonValue> {
    let json_str = sonic_rs::to_string(value)?;
    let json: JsonValue = serde_json::from_str(&json_str)?;
    Ok(json)
}
