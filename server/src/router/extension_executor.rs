use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use bytes::Bytes;
use graphql_parser::query::{
    Definition, Field, OperationDefinition, Selection, SelectionSet, Value as AstValue,
};
use hive_router_plan_executor::executors::common::{HttpExecutionRequest, SubgraphExecutor};
use serde_json::{Map, Value as JsonValue};

use crate::extensions::wasm_runtime::Extension as WasmExtension;

use super::{graphql_error_body, sonic_to_serde};

type Vars = HashMap<String, JsonValue>;

#[derive(Clone)]
struct FieldTypeMeta {
    base_type: String,
    is_list: bool,
}

#[derive(Clone, Default)]
struct ObjectTypeMeta {
    fields: HashMap<String, FieldTypeMeta>,
}

#[derive(Default)]
struct ExtensionSchemaMetadata {
    query_fields: HashMap<String, FieldTypeMeta>,
    mutation_fields: HashMap<String, FieldTypeMeta>,
    object_types: HashMap<String, ObjectTypeMeta>,
    enum_types: HashSet<String>,
    scalar_types: HashSet<String>,
}

pub(crate) struct ExtensionSubgraphExecutor {
    subgraph_name: String,
    runtime: Arc<WasmExtension>,
    schema: ExtensionSchemaMetadata,
}

impl ExtensionSubgraphExecutor {
    pub fn new(name: String, runtime: Arc<WasmExtension>, schema_sdl: &str) -> Result<Self> {
        let schema = ExtensionSchemaMetadata::from_sdl(schema_sdl)?;
        Ok(Self {
            subgraph_name: name,
            runtime,
            schema,
        })
    }

    pub async fn execute_operation<'a>(
        &self,
        execution_request: HttpExecutionRequest<'a>,
    ) -> Result<JsonValue> {
        if execution_request.representations.is_some() {
            return Err(anyhow!(
                "Entity representations are not supported yet for extension subgraphs"
            ));
        }

        let document = graphql_parser::parse_query::<String>(execution_request.query)
            .context("failed to parse extension operation")?;
        let operation = self
            .find_operation(&document, execution_request.operation_name)
            .context("operation not found")?;
        let variables = self.build_variables(execution_request.variables)?;

        let data_value = match operation {
            OperationDefinition::Query(query) => {
                let map = self
                    .resolve_query_selection_set(&query.selection_set, &variables)
                    .await?;
                JsonValue::Object(map)
            }
            OperationDefinition::Mutation(mutation) => {
                let map = self
                    .resolve_mutation_selection_set(&mutation.selection_set, &variables)
                    .await?;
                JsonValue::Object(map)
            }
            OperationDefinition::Subscription(_) => {
                return Err(anyhow!("subscriptions are not supported"));
            }
            OperationDefinition::SelectionSet(selection_set) => {
                let map = self
                    .resolve_query_selection_set(selection_set, &variables)
                    .await?;
                JsonValue::Object(map)
            }
        };

        let mut response = Map::new();
        response.insert("data".to_string(), data_value);
        Ok(JsonValue::Object(response))
    }

    fn find_operation<'a>(
        &self,
        document: &'a graphql_parser::query::Document<'a, String>,
        operation_name: Option<&'a str>,
    ) -> Option<&'a OperationDefinition<'a, String>> {
        document
            .definitions
            .iter()
            .find_map(|definition| match definition {
                Definition::Operation(op) => match (operation_name, op) {
                    (None, _) => Some(op),
                    (Some(name), OperationDefinition::Query(query))
                        if query.name.as_deref() == Some(name) =>
                    {
                        Some(op)
                    }
                    (Some(name), OperationDefinition::Mutation(mutation))
                        if mutation.name.as_deref() == Some(name) =>
                    {
                        Some(op)
                    }
                    (Some(name), OperationDefinition::Subscription(subscription))
                        if subscription.name.as_deref() == Some(name) =>
                    {
                        Some(op)
                    }
                    (Some(name), OperationDefinition::SelectionSet(_)) if name.is_empty() => {
                        Some(op)
                    }
                    _ => None,
                },
                _ => None,
            })
    }

    fn build_variables(&self, variables: Option<HashMap<&str, &sonic_rs::Value>>) -> Result<Vars> {
        let mut out = HashMap::new();
        if let Some(vars) = variables {
            for (name, value) in vars {
                out.insert(name.to_string(), sonic_to_serde(value)?);
            }
        }
        Ok(out)
    }

    async fn resolve_query_selection_set(
        &self,
        selection_set: &SelectionSet<'_, String>,
        variables: &Vars,
    ) -> Result<Map<String, JsonValue>> {
        let mut map = Map::new();
        for selection in &selection_set.items {
            if let Selection::Field(field) = selection {
                let key = response_key(field);
                let value = self.resolve_query_field(field, variables).await?;
                map.insert(key, value);
            }
        }
        Ok(map)
    }

    async fn resolve_mutation_selection_set(
        &self,
        selection_set: &SelectionSet<'_, String>,
        variables: &Vars,
    ) -> Result<Map<String, JsonValue>> {
        let mut map = Map::new();
        for selection in &selection_set.items {
            if let Selection::Field(field) = selection {
                let key = response_key(field);
                let value = self.resolve_mutation_field(field, variables).await?;
                map.insert(key, value);
            }
        }
        Ok(map)
    }

    async fn resolve_query_field(
        &self,
        field: &Field<'_, String>,
        variables: &Vars,
    ) -> Result<JsonValue> {
        let field_meta = self
            .schema
            .query_fields
            .get(&field.name)
            .ok_or_else(|| anyhow!("Unsupported query field `{}`", field.name))?;
        let args = self.build_argument_map(field, variables)?;
        let args_value = JsonValue::Object(args);
        let result = self
            .runtime
            .resolve_field(
                field.name.clone(),
                "Query".to_string(),
                args_value,
                JsonValue::Null,
                None,
            )
            .await
            .with_context(|| {
                format!(
                    "extension `{}` failed to resolve field `{}`",
                    self.subgraph_name, field.name
                )
            })?;
        self.project_by_type(&result, &field.selection_set, field_meta)
    }

    async fn resolve_mutation_field(
        &self,
        field: &Field<'_, String>,
        variables: &Vars,
    ) -> Result<JsonValue> {
        let field_meta = self
            .schema
            .mutation_fields
            .get(&field.name)
            .ok_or_else(|| anyhow!("Unsupported mutation field `{}`", field.name))?;
        let args = self.build_argument_map(field, variables)?;
        let args_value = JsonValue::Object(args);
        let result = self
            .runtime
            .resolve_field(
                field.name.clone(),
                "Mutation".to_string(),
                args_value,
                JsonValue::Null,
                None,
            )
            .await
            .with_context(|| {
                format!(
                    "extension `{}` failed to resolve field `{}`",
                    self.subgraph_name, field.name
                )
            })?;
        self.project_by_type(&result, &field.selection_set, field_meta)
    }

    fn build_argument_map(
        &self,
        field: &Field<'_, String>,
        variables: &Vars,
    ) -> Result<Map<String, JsonValue>> {
        let mut map = Map::new();
        for (name, value) in &field.arguments {
            map.insert(name.clone(), self.evaluate_value(value, variables)?);
        }
        Ok(map)
    }

    fn evaluate_value(&self, value: &AstValue<'_, String>, variables: &Vars) -> Result<JsonValue> {
        Ok(match value {
            AstValue::Variable(name) => variables
                .get(name)
                .cloned()
                .ok_or_else(|| anyhow!("Variable `{}` not provided", name))?,
            AstValue::Int(i) => {
                let value = i
                    .as_i64()
                    .ok_or_else(|| anyhow!("integer value is out of range"))?;
                JsonValue::Number(value.into())
            }
            AstValue::Float(f) => JsonValue::Number(
                serde_json::Number::from_f64(*f)
                    .ok_or_else(|| anyhow!("Failed to convert float value `{}`", f))?,
            ),
            AstValue::String(s) => JsonValue::String(s.clone()),
            AstValue::Boolean(b) => JsonValue::Bool(*b),
            AstValue::Null => JsonValue::Null,
            AstValue::Enum(e) => JsonValue::String(e.clone()),
            AstValue::List(items) => JsonValue::Array(
                items
                    .iter()
                    .map(|item| self.evaluate_value(item, variables))
                    .collect::<Result<Vec<_>>>()?,
            ),
            AstValue::Object(obj) => {
                let mut map = Map::new();
                for (name, value) in obj {
                    map.insert(name.clone(), self.evaluate_value(value, variables)?);
                }
                JsonValue::Object(map)
            }
        })
    }

    fn project_by_type(
        &self,
        value: &JsonValue,
        selection_set: &SelectionSet<'_, String>,
        field_type: &FieldTypeMeta,
    ) -> Result<JsonValue> {
        if field_type.is_list {
            let items = match value {
                JsonValue::Array(items) => items,
                JsonValue::Null => return Ok(JsonValue::Null),
                _ => {
                    return Err(anyhow!(
                        "Field expected to resolve to a list but got {:?}",
                        value
                    ));
                }
            };
            let mut projected = Vec::with_capacity(items.len());
            for item in items {
                projected.push(self.project_single(item, selection_set, &field_type.base_type)?);
            }
            Ok(JsonValue::Array(projected))
        } else {
            self.project_single(value, selection_set, &field_type.base_type)
        }
    }

    fn project_single(
        &self,
        value: &JsonValue,
        selection_set: &SelectionSet<'_, String>,
        type_name: &str,
    ) -> Result<JsonValue> {
        if selection_set.items.is_empty() || !self.schema.object_types.contains_key(type_name) {
            return Ok(value.clone());
        }

        let object = match value {
            JsonValue::Object(map) => map,
            JsonValue::Null => return Ok(JsonValue::Null),
            other => {
                return Err(anyhow!(
                    "Expected object value for type `{}` but got {:?}",
                    type_name,
                    other
                ));
            }
        };

        let type_meta = self
            .schema
            .object_types
            .get(type_name)
            .ok_or_else(|| anyhow!("Unknown object type `{}`", type_name))?;

        let mut map = Map::new();
        for selection in &selection_set.items {
            if let Selection::Field(field) = selection {
                let key = response_key(field);
                if field.name == "__typename" {
                    map.insert(key, JsonValue::String(type_name.to_string()));
                    continue;
                }

                let field_meta = type_meta.fields.get(&field.name).ok_or_else(|| {
                    anyhow!("Unknown field `{}` on type `{}`", field.name, type_name)
                })?;

                let child_value = object.get(&field.name).cloned().unwrap_or(JsonValue::Null);
                let projected_child =
                    self.project_by_type(&child_value, &field.selection_set, field_meta)?;
                map.insert(key, projected_child);
            }
        }

        Ok(JsonValue::Object(map))
    }
}

fn response_key(field: &Field<'_, String>) -> String {
    field
        .alias
        .as_ref()
        .cloned()
        .unwrap_or_else(|| field.name.clone())
}

#[async_trait]
impl SubgraphExecutor for ExtensionSubgraphExecutor {
    async fn execute<'a>(&self, execution_request: HttpExecutionRequest<'a>) -> Bytes {
        match self.execute_operation(execution_request).await {
            Ok(json) => match sonic_rs::to_vec(&json) {
                Ok(bytes) => Bytes::from(bytes),
                Err(err) => {
                    let body = graphql_error_body(JsonValue::String(format!(
                        "failed to serialize extension response for `{}`: {}",
                        self.subgraph_name, err
                    )));
                    Bytes::from(serde_json::to_vec(&body).expect("serialization failure"))
                }
            },
            Err(err) => {
                let body = graphql_error_body(JsonValue::String(err.to_string()));
                Bytes::from(serde_json::to_vec(&body).expect("serialization failure"))
            }
        }
    }
}

impl ExtensionSchemaMetadata {
    fn from_sdl(sdl: &str) -> Result<Self> {
        let document = async_graphql_parser::parse_schema(sdl)
            .context("failed to parse extension schema SDL")?;

        let mut metadata = ExtensionSchemaMetadata::default();
        metadata
            .scalar_types
            .extend(["ID", "String", "Int", "Float", "Boolean"].map(|s| s.to_string()));

        use async_graphql_parser::types::TypeSystemDefinition;

        for definition in &document.definitions {
            if let TypeSystemDefinition::Type(type_def) = definition {
                metadata.process_type_definition(&type_def.node);
            }
        }

        Ok(metadata)
    }

    fn process_type_definition(&mut self, type_def: &async_graphql_parser::types::TypeDefinition) {
        use async_graphql_parser::types::TypeKind;

        match &type_def.kind {
            TypeKind::Object(obj) => {
                let type_name = type_def.name.node.to_string();
                let mut field_map = HashMap::new();
                for field in &obj.fields {
                    let field_name = field.node.name.node.to_string();
                    let field_type = FieldTypeMeta::from_type(&field.node.ty.node);
                    if type_name == "Query" {
                        self.query_fields
                            .insert(field_name.clone(), field_type.clone());
                    } else if type_name == "Mutation" {
                        self.mutation_fields
                            .insert(field_name.clone(), field_type.clone());
                    }
                    field_map.insert(field_name, field_type);
                }

                if type_name != "Query" && type_name != "Mutation" {
                    let entry = self
                        .object_types
                        .entry(type_name.clone())
                        .or_insert_with(ObjectTypeMeta::default);
                    entry.fields.extend(field_map);
                }
            }
            TypeKind::Scalar => {
                self.scalar_types.insert(type_def.name.node.to_string());
            }
            TypeKind::Enum(_) => {
                self.enum_types.insert(type_def.name.node.to_string());
            }
            _ => {}
        }
    }
}

impl FieldTypeMeta {
    fn from_type(ty: &async_graphql_parser::types::Type) -> Self {
        use async_graphql_parser::types::BaseType;

        match &ty.base {
            BaseType::Named(name) => FieldTypeMeta {
                base_type: name.to_string(),
                is_list: false,
            },
            BaseType::List(inner) => {
                let mut inner_meta = FieldTypeMeta::from_type(inner);
                inner_meta.is_list = true;
                inner_meta
            }
        }
    }
}
