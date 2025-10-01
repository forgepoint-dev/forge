use std::collections::HashMap;

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use bytes::Bytes;
use graphql_parser::query::{
    Definition, Field, FragmentDefinition, OperationDefinition, Selection, SelectionSet,
    TypeCondition, Value as AstValue,
};
use hive_router_plan_executor::executors::common::{HttpExecutionRequest, SubgraphExecutor};
use serde_json::{Map, Value as JsonValue};
use sqlx::SqlitePool;

use crate::group::mutations::{CreateGroupInput, create_group_raw};
use crate::group::{
    models::GroupRecord,
    queries::{get_all_groups_raw, get_group_parent, get_group_raw, repositories_for_group},
};
use crate::repository::{
    models::{
        RepositoryEntriesPayload, RepositoryEntryKind, RepositoryEntryNode, RepositoryFilePayload,
        RepositoryRecord, RepositorySummary,
    },
    mutations::{CreateRepositoryInput, create_repository_raw, link_remote_repository_raw},
    queries::{
        browse_repository_raw, get_all_repositories_raw, get_repository_raw,
        read_repository_file_raw,
    },
    storage::RepositoryStorage,
};

use super::{graphql_error_body, sonic_to_serde};

pub(crate) struct CoreSubgraphExecutor {
    pool: SqlitePool,
    storage: RepositoryStorage,
}

type Vars = HashMap<String, JsonValue>;
type FragmentMap<'a> = HashMap<&'a str, &'a FragmentDefinition<'a, String>>;

impl CoreSubgraphExecutor {
    pub fn new(pool: SqlitePool, storage: RepositoryStorage) -> Self {
        Self { pool, storage }
    }

    pub async fn execute_operation<'a>(
        &self,
        execution_request: HttpExecutionRequest<'a>,
    ) -> Result<JsonValue> {
        let document = graphql_parser::parse_query::<String>(execution_request.query)
            .context("failed to parse GraphQL document")?;

        let operation = self
            .find_operation(&document, execution_request.operation_name)
            .context("operation not found")?;

        let fragments = collect_fragment_definitions(&document);

        let variables = self.build_variables(execution_request.variables)?;

        let data_value = match operation {
            OperationDefinition::Query(query) => {
                let map = self
                    .resolve_query_selection_set(
                        &query.selection_set,
                        &variables,
                        &fragments,
                        "Query",
                    )
                    .await?;
                JsonValue::Object(map)
            }
            OperationDefinition::Mutation(mutation) => {
                let map = self
                    .resolve_mutation_selection_set(
                        &mutation.selection_set,
                        &variables,
                        &fragments,
                        "Mutation",
                    )
                    .await?;
                JsonValue::Object(map)
            }
            OperationDefinition::Subscription(_) => {
                return Err(anyhow!("subscriptions are not supported"));
            }
            OperationDefinition::SelectionSet(selection_set) => {
                let map = self
                    .resolve_query_selection_set(selection_set, &variables, &fragments, "Query")
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

    async fn resolve_query_selection_set<'a>(
        &self,
        selection_set: &'a SelectionSet<'a, String>,
        variables: &Vars,
        fragments: &FragmentMap<'a>,
        type_name: &str,
    ) -> Result<Map<String, JsonValue>> {
        let mut map = Map::new();
        let fields = selection_fields(selection_set, type_name, fragments)?;
        for field in fields {
            let key = response_key(field);
            let value = self
                .resolve_query_field(field, variables, fragments)
                .await?;
            map.insert(key, value);
        }
        Ok(map)
    }

    async fn resolve_mutation_selection_set<'a>(
        &self,
        selection_set: &'a SelectionSet<'a, String>,
        variables: &Vars,
        fragments: &FragmentMap<'a>,
        type_name: &str,
    ) -> Result<Map<String, JsonValue>> {
        let mut map = Map::new();
        let fields = selection_fields(selection_set, type_name, fragments)?;
        for field in fields {
            let key = response_key(field);
            let value = self
                .resolve_mutation_field(field, variables, fragments)
                .await?;
            map.insert(key, value);
        }
        Ok(map)
    }

    async fn resolve_query_field<'a>(
        &self,
        field: &'a Field<'a, String>,
        variables: &Vars,
        fragments: &FragmentMap<'a>,
    ) -> Result<JsonValue> {
        match field.name.as_str() {
            "__typename" => Ok(JsonValue::String("Query".to_string())),
            "getAllGroups" => {
                let records = get_all_groups_raw(&self.pool).await?;
                let mut items = Vec::with_capacity(records.len());
                for record in records {
                    items.push(
                        self.project_group_node(&record, &field.selection_set, fragments)
                            .await?,
                    );
                }
                Ok(JsonValue::Array(items))
            }
            "getGroup" => {
                let path = self
                    .get_required_argument(field, "path", variables)?
                    .as_str()
                    .ok_or_else(|| anyhow!("path argument must be a string"))?
                    .to_string();
                let record = get_group_raw(&self.pool, path).await?;
                match record {
                    Some(record) => {
                        self.project_group_node(&record, &field.selection_set, fragments)
                            .await
                    }
                    None => Ok(JsonValue::Null),
                }
            }
            "getAllRepositories" => {
                let records = get_all_repositories_raw(&self.pool).await?;
                let mut items = Vec::with_capacity(records.len());
                for record in records {
                    items.push(
                        self.project_repository_node(&record, &field.selection_set, fragments)
                            .await?,
                    );
                }
                Ok(JsonValue::Array(items))
            }
            "getRepository" => {
                let path = self
                    .get_required_argument(field, "path", variables)?
                    .as_str()
                    .ok_or_else(|| anyhow!("path argument must be a string"))?
                    .to_string();
                let record = get_repository_raw(&self.pool, path).await?;
                match record {
                    Some(record) => {
                        self.project_repository_node(&record, &field.selection_set, fragments)
                            .await
                    }
                    None => Ok(JsonValue::Null),
                }
            }
            "browseRepository" => {
                let path = self
                    .get_required_argument(field, "path", variables)?
                    .as_str()
                    .ok_or_else(|| anyhow!("path argument must be a string"))?
                    .to_string();
                let tree_path = self
                    .get_optional_argument(field, "treePath", variables)?
                    .and_then(|v| v.as_str().map(|s| s.to_string()));
                let payload =
                    browse_repository_raw(&self.pool, &self.storage, path, tree_path).await?;
                match payload {
                    Some(payload) => self.project_repository_entries_payload(
                        &payload,
                        &field.selection_set,
                        fragments,
                    ),
                    None => Ok(JsonValue::Null),
                }
            }
            "readRepositoryFile" => {
                let path = self
                    .get_required_argument(field, "path", variables)?
                    .as_str()
                    .ok_or_else(|| anyhow!("path argument must be a string"))?
                    .to_string();
                let file_path = self
                    .get_required_argument(field, "filePath", variables)?
                    .as_str()
                    .ok_or_else(|| anyhow!("filePath argument must be a string"))?
                    .to_string();
                let payload =
                    read_repository_file_raw(&self.pool, &self.storage, path, file_path).await?;
                match payload {
                    Some(payload) => self.project_repository_file_payload(
                        &payload,
                        &field.selection_set,
                        fragments,
                    ),
                    None => Ok(JsonValue::Null),
                }
            }
            other => Err(anyhow!("Unsupported query field `{}`", other)),
        }
    }

    async fn resolve_mutation_field<'a>(
        &self,
        field: &'a Field<'a, String>,
        variables: &Vars,
        fragments: &FragmentMap<'a>,
    ) -> Result<JsonValue> {
        match field.name.as_str() {
            "__typename" => Ok(JsonValue::String("Mutation".to_string())),
            "createGroup" => {
                let input_value = self.get_required_argument(field, "input", variables)?;
                let input = self.parse_create_group_input(&input_value)?;
                let record = create_group_raw(&self.pool, input).await?;
                self.project_group_node(&record, &field.selection_set, fragments)
                    .await
            }
            "createRepository" => {
                let input_value = self.get_required_argument(field, "input", variables)?;
                let input = self.parse_create_repository_input(&input_value)?;
                let record = create_repository_raw(&self.pool, input).await?;
                self.project_repository_node(&record, &field.selection_set, fragments)
                    .await
            }
            "linkRemoteRepository" => {
                let url_value = self.get_required_argument(field, "url", variables)?;
                let url = url_value
                    .as_str()
                    .ok_or_else(|| anyhow!("url argument must be a string"))?
                    .to_string();
                let record = link_remote_repository_raw(&self.pool, &self.storage, url).await?;
                self.project_repository_node(&record, &field.selection_set, fragments)
                    .await
            }
            other => Err(anyhow!("Unsupported mutation field `{}`", other)),
        }
    }

    fn get_required_argument(
        &self,
        field: &Field<'_, String>,
        name: &str,
        variables: &Vars,
    ) -> Result<JsonValue> {
        field
            .arguments
            .iter()
            .find(|(arg_name, _)| arg_name == name)
            .map(|(_, value)| self.evaluate_value(value, variables))
            .transpose()?
            .ok_or_else(|| anyhow!("Missing required argument `{}`", name))
    }

    fn get_optional_argument(
        &self,
        field: &Field<'_, String>,
        name: &str,
        variables: &Vars,
    ) -> Result<Option<JsonValue>> {
        field
            .arguments
            .iter()
            .find(|(arg_name, _)| arg_name == name)
            .map(|(_, value)| self.evaluate_value(value, variables))
            .transpose()
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

    async fn project_group_node<'a>(
        &self,
        record: &GroupRecord,
        selection_set: &'a SelectionSet<'a, String>,
        fragments: &FragmentMap<'a>,
    ) -> Result<JsonValue> {
        let mut map = Map::new();
        let fields = selection_fields(selection_set, "GroupNode", fragments)?;
        for field in fields {
            let key = response_key(field);
            let value = match field.name.as_str() {
                "__typename" => JsonValue::String("GroupNode".to_string()),
                "id" => JsonValue::String(record.id.clone()),
                "slug" => JsonValue::String(record.slug.clone()),
                "parent" => {
                    if let Some(parent_id) = &record.parent {
                        let parent =
                            get_group_parent(&self.pool, parent_id)
                                .await?
                                .ok_or_else(|| {
                                    anyhow!(
                                        "Parent with id `{}` not found for group `{}`",
                                        parent_id,
                                        record.id
                                    )
                                })?;
                        self.project_group_summary(&parent, &field.selection_set, fragments)?
                    } else {
                        JsonValue::Null
                    }
                }
                "repositories" => {
                    let summaries = repositories_for_group(&self.pool, &record.id).await?;
                    let mut items = Vec::with_capacity(summaries.len());
                    for summary in summaries {
                        items.push(self.project_repository_summary(
                            &summary,
                            &field.selection_set,
                            fragments,
                        )?);
                    }
                    JsonValue::Array(items)
                }
                other => {
                    return Err(anyhow!("Unsupported field `{}` on GroupNode", other));
                }
            };
            map.insert(key, value);
        }
        Ok(JsonValue::Object(map))
    }

    fn project_group_summary<'a>(
        &self,
        record: &GroupRecord,
        selection_set: &'a SelectionSet<'a, String>,
        fragments: &FragmentMap<'a>,
    ) -> Result<JsonValue> {
        let mut map = Map::new();
        let fields = selection_fields(selection_set, "GroupSummary", fragments)?;
        for field in fields {
            let key = response_key(field);
            let value = match field.name.as_str() {
                "__typename" => JsonValue::String("GroupSummary".to_string()),
                "id" => JsonValue::String(record.id.clone()),
                "slug" => JsonValue::String(record.slug.clone()),
                other => {
                    return Err(anyhow!("Unsupported field `{}` on GroupSummary", other));
                }
            };
            map.insert(key, value);
        }
        Ok(JsonValue::Object(map))
    }

    async fn project_repository_node<'a>(
        &self,
        record: &RepositoryRecord,
        selection_set: &'a SelectionSet<'a, String>,
        fragments: &FragmentMap<'a>,
    ) -> Result<JsonValue> {
        let mut map = Map::new();
        let fields = selection_fields(selection_set, "RepositoryNode", fragments)?;
        for field in fields {
            let key = response_key(field);
            let value = match field.name.as_str() {
                "__typename" => JsonValue::String("RepositoryNode".to_string()),
                "id" => JsonValue::String(record.id.clone()),
                "slug" => JsonValue::String(record.slug.clone()),
                "isRemote" => JsonValue::Bool(record.remote_url.is_some()),
                "remoteUrl" => match &record.remote_url {
                    Some(url) => JsonValue::String(url.clone()),
                    None => JsonValue::Null,
                },
                "group" => {
                    if let Some(group_id) = &record.group_id {
                        if let Some(group) = get_group_parent(&self.pool, group_id).await? {
                            self.project_group_summary(&group, &field.selection_set, fragments)?
                        } else {
                            JsonValue::Null
                        }
                    } else {
                        JsonValue::Null
                    }
                }
                _ => JsonValue::Null,
            };
            map.insert(key, value);
        }
        Ok(JsonValue::Object(map))
    }

    fn project_repository_summary<'a>(
        &self,
        summary: &RepositorySummary,
        selection_set: &'a SelectionSet<'a, String>,
        fragments: &FragmentMap<'a>,
    ) -> Result<JsonValue> {
        let mut map = Map::new();
        let fields = selection_fields(selection_set, "RepositorySummary", fragments)?;
        for field in fields {
            let key = response_key(field);
            let value = match field.name.as_str() {
                "__typename" => JsonValue::String("RepositorySummary".to_string()),
                "id" => JsonValue::String(summary.id.clone()),
                "slug" => JsonValue::String(summary.slug.clone()),
                "isRemote" => JsonValue::Bool(summary.remote_url.is_some()),
                "remoteUrl" => match &summary.remote_url {
                    Some(url) => JsonValue::String(url.clone()),
                    None => JsonValue::Null,
                },
                _ => JsonValue::Null,
            };
            map.insert(key, value);
        }
        Ok(JsonValue::Object(map))
    }

    fn project_repository_entries_payload<'a>(
        &self,
        payload: &RepositoryEntriesPayload,
        selection_set: &'a SelectionSet<'a, String>,
        fragments: &FragmentMap<'a>,
    ) -> Result<JsonValue> {
        let mut map = Map::new();
        let fields = selection_fields(selection_set, "RepositoryEntriesPayload", fragments)?;
        for field in fields {
            let key = response_key(field);
            let value = match field.name.as_str() {
                "__typename" => JsonValue::String("RepositoryEntriesPayload".to_string()),
                "treePath" => JsonValue::String(payload.tree_path.clone()),
                "entries" => {
                    let mut items = Vec::with_capacity(payload.entries.len());
                    for entry in &payload.entries {
                        items.push(self.project_repository_entry(
                            entry,
                            &field.selection_set,
                            fragments,
                        )?);
                    }
                    JsonValue::Array(items)
                }
                _ => JsonValue::Null,
            };
            map.insert(key, value);
        }
        Ok(JsonValue::Object(map))
    }

    fn project_repository_file_payload<'a>(
        &self,
        payload: &RepositoryFilePayload,
        selection_set: &'a SelectionSet<'a, String>,
        fragments: &FragmentMap<'a>,
    ) -> Result<JsonValue> {
        let mut map = Map::new();
        let fields = selection_fields(selection_set, "RepositoryFilePayload", fragments)?;
        for field in fields {
            let key = response_key(field);
            let value = match field.name.as_str() {
                "__typename" => JsonValue::String("RepositoryFilePayload".to_string()),
                "path" => JsonValue::String(payload.path.clone()),
                "name" => JsonValue::String(payload.name.clone()),
                "size" => JsonValue::Number(payload.size.into()),
                "isBinary" => JsonValue::Bool(payload.is_binary),
                "text" => match &payload.text {
                    Some(text) => JsonValue::String(text.clone()),
                    None => JsonValue::Null,
                },
                "truncated" => JsonValue::Bool(payload.truncated),
                _ => JsonValue::Null,
            };
            map.insert(key, value);
        }
        Ok(JsonValue::Object(map))
    }

    fn project_repository_entry<'a>(
        &self,
        entry: &RepositoryEntryNode,
        selection_set: &'a SelectionSet<'a, String>,
        fragments: &FragmentMap<'a>,
    ) -> Result<JsonValue> {
        let mut map = Map::new();
        let fields = selection_fields(selection_set, "RepositoryEntry", fragments)?;
        for field in fields {
            let key = response_key(field);
            let value = match field.name.as_str() {
                "__typename" => JsonValue::String("RepositoryEntry".to_string()),
                "name" => JsonValue::String(entry.name.clone()),
                "path" => JsonValue::String(entry.path.clone()),
                "type" => JsonValue::String(match entry.kind {
                    RepositoryEntryKind::File => "FILE".to_string(),
                    RepositoryEntryKind::Directory => "DIRECTORY".to_string(),
                }),
                "size" => match entry.size {
                    Some(size) => JsonValue::Number(size.into()),
                    None => JsonValue::Null,
                },
                _ => JsonValue::Null,
            };
            map.insert(key, value);
        }
        Ok(JsonValue::Object(map))
    }

    fn parse_create_group_input(&self, value: &JsonValue) -> Result<CreateGroupInput> {
        let slug = value
            .get("slug")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| anyhow!("createGroup input.slug must be a string"))?
            .to_string();
        let parent = value
            .get("parent")
            .and_then(JsonValue::as_str)
            .map(|s| s.to_string());
        Ok(CreateGroupInput { slug, parent })
    }

    fn parse_create_repository_input(&self, value: &JsonValue) -> Result<CreateRepositoryInput> {
        let slug = value
            .get("slug")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| anyhow!("createRepository input.slug must be a string"))?
            .to_string();
        let group = value
            .get("group")
            .and_then(JsonValue::as_str)
            .map(|s| s.to_string());
        Ok(CreateRepositoryInput { slug, group })
    }
}

fn collect_fragment_definitions<'a>(
    document: &'a graphql_parser::query::Document<'a, String>,
) -> FragmentMap<'a> {
    let mut fragments: FragmentMap<'a> = HashMap::new();
    for definition in &document.definitions {
        if let Definition::Fragment(fragment) = definition {
            fragments.insert(fragment.name.as_str(), fragment);
        }
    }
    fragments
}

fn selection_fields<'a>(
    selection_set: &'a SelectionSet<'a, String>,
    type_name: &str,
    fragments: &FragmentMap<'a>,
) -> Result<Vec<&'a Field<'a, String>>> {
    let mut fields = Vec::new();
    collect_selection_fields(selection_set, type_name, fragments, &mut fields)?;
    Ok(fields)
}

fn collect_selection_fields<'a>(
    selection_set: &'a SelectionSet<'a, String>,
    type_name: &str,
    fragments: &FragmentMap<'a>,
    out: &mut Vec<&'a Field<'a, String>>,
) -> Result<()> {
    for selection in &selection_set.items {
        match selection {
            Selection::Field(field) => out.push(field),
            Selection::FragmentSpread(fragment_spread) => {
                let fragment = fragments
                    .get(fragment_spread.fragment_name.as_str())
                    .ok_or_else(|| {
                        anyhow!("Unknown fragment `{}`", fragment_spread.fragment_name)
                    })?;
                if type_condition_matches(type_name, &fragment.type_condition) {
                    collect_selection_fields(&fragment.selection_set, type_name, fragments, out)?;
                }
            }
            Selection::InlineFragment(inline_fragment) => {
                if inline_fragment
                    .type_condition
                    .as_ref()
                    .map(|cond| type_condition_matches(type_name, cond))
                    .unwrap_or(true)
                {
                    collect_selection_fields(
                        &inline_fragment.selection_set,
                        type_name,
                        fragments,
                        out,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn type_condition_matches(type_name: &str, type_condition: &TypeCondition<'_, String>) -> bool {
    match type_condition {
        TypeCondition::On(target) => target == type_name,
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
impl SubgraphExecutor for CoreSubgraphExecutor {
    async fn execute<'a>(&self, execution_request: HttpExecutionRequest<'a>) -> Bytes {
        match self.execute_operation(execution_request).await {
            Ok(json) => match sonic_rs::to_vec(&json) {
                Ok(bytes) => Bytes::from(bytes),
                Err(err) => {
                    let body = graphql_error_body(JsonValue::String(format!(
                        "failed to serialize core subgraph response: {}",
                        err
                    )));
                    Bytes::from(serde_json::to_vec(&body).expect("serialization failed"))
                }
            },
            Err(err) => {
                let body = graphql_error_body(JsonValue::String(err.to_string()));
                Bytes::from(serde_json::to_vec(&body).expect("serialization failed"))
            }
        }
    }
}
