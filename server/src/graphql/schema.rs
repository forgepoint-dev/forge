use async_graphql::{Context, EmptySubscription, Object, Schema, Value, Enum, SimpleObject, InputObject};
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::extensions::ExtensionManager;
use super::extension_resolver::ExtensionFieldRegistry;
use crate::group::{CreateGroupInput, GroupNode};
use crate::repository::{
    CreateRepositoryInput, RepositoryEntriesPayload, RepositoryNode, RepositoryStorage,
};

// Extension types for Issues
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum IssueStatus {
    Open,
    Closed,
    InProgress,
}

#[derive(SimpleObject)]
pub struct Issue {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: IssueStatus,
    pub created_at: String,
}

#[derive(InputObject)]
pub struct CreateIssueInput {
    pub title: String,
    pub description: Option<String>,
}

#[derive(InputObject)]
pub struct UpdateIssueInput {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<IssueStatus>,
}

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

    // Extension fields - Issues
    #[graphql(name = "getAllIssues")]
    async fn get_all_issues(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Issue>> {
        let registry = ctx.data::<Arc<ExtensionFieldRegistry>>()?;
        let result = registry
            .resolve_field("Query", "getAllIssues", Value::Null, Value::Null)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        // Convert async_graphql::Value to Vec<Issue>
        let json_value = serde_json::to_value(&result)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        // Parse the JSON array into Issue objects
        let issues: Vec<Issue> = if let Some(array) = json_value.as_array() {
            array.iter().map(|item| {
                Issue {
                    id: item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    title: item.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    description: item.get("description").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    status: match item.get("status").and_then(|v| v.as_str()).unwrap_or("OPEN") {
                        "CLOSED" => IssueStatus::Closed,
                        "IN_PROGRESS" => IssueStatus::InProgress,
                        _ => IssueStatus::Open,
                    },
                    created_at: item.get("createdAt").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                }
            }).collect()
        } else {
            Vec::new()
        };

        Ok(issues)
    }

    #[graphql(name = "getIssue")]
    async fn get_issue(&self, ctx: &Context<'_>, id: String) -> async_graphql::Result<Option<Issue>> {
        let registry = ctx.data::<Arc<ExtensionFieldRegistry>>()?;
        let mut args = async_graphql::indexmap::IndexMap::new();
        args.insert(async_graphql::Name::new("id"), Value::String(id));

        let result = registry
            .resolve_field("Query", "getIssue", Value::Object(args), Value::Null)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        // Convert async_graphql::Value to Option<Issue>
        let json_value = serde_json::to_value(&result)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        if json_value.is_null() {
            return Ok(None);
        }

        let issue = Issue {
            id: json_value.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            title: json_value.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            description: json_value.get("description").and_then(|v| v.as_str()).map(|s| s.to_string()),
            status: match json_value.get("status").and_then(|v| v.as_str()).unwrap_or("OPEN") {
                "CLOSED" => IssueStatus::Closed,
                "IN_PROGRESS" => IssueStatus::InProgress,
                _ => IssueStatus::Open,
            },
            created_at: json_value.get("createdAt").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        };

        Ok(Some(issue))
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

    // Extension fields - Issues
    #[graphql(name = "createIssue")]
    async fn create_issue(
        &self,
        ctx: &Context<'_>,
        input: CreateIssueInput,
    ) -> async_graphql::Result<Issue> {
        let registry = ctx.data::<Arc<ExtensionFieldRegistry>>()?;
        let mut args = async_graphql::indexmap::IndexMap::new();

        // Convert CreateIssueInput to Value
        let input_json = serde_json::json!({
            "title": input.title,
            "description": input.description
        });
        let input_value: Value = serde_json::from_value(input_json)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        args.insert(async_graphql::Name::new("input"), input_value);

        let result = registry
            .resolve_field("Mutation", "createIssue", Value::Object(args), Value::Null)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        // Convert async_graphql::Value to Issue
        let json_value = serde_json::to_value(&result)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        let issue = Issue {
            id: json_value.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            title: json_value.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            description: json_value.get("description").and_then(|v| v.as_str()).map(|s| s.to_string()),
            status: match json_value.get("status").and_then(|v| v.as_str()).unwrap_or("OPEN") {
                "CLOSED" => IssueStatus::Closed,
                "IN_PROGRESS" => IssueStatus::InProgress,
                _ => IssueStatus::Open,
            },
            created_at: json_value.get("createdAt").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        };

        Ok(issue)
    }

    #[graphql(name = "updateIssue")]
    async fn update_issue(
        &self,
        ctx: &Context<'_>,
        id: String,
        input: UpdateIssueInput,
    ) -> async_graphql::Result<Option<Issue>> {
        let registry = ctx.data::<Arc<ExtensionFieldRegistry>>()?;
        let mut args = async_graphql::indexmap::IndexMap::new();
        args.insert(async_graphql::Name::new("id"), Value::String(id));

        // Convert UpdateIssueInput to Value
        let mut input_json = serde_json::Map::new();
        if let Some(title) = input.title {
            input_json.insert("title".to_string(), serde_json::Value::String(title));
        }
        if let Some(description) = input.description {
            input_json.insert("description".to_string(), serde_json::Value::String(description));
        }
        if let Some(status) = input.status {
            let status_str = match status {
                IssueStatus::Open => "OPEN",
                IssueStatus::Closed => "CLOSED",
                IssueStatus::InProgress => "IN_PROGRESS",
            };
            input_json.insert("status".to_string(), serde_json::Value::String(status_str.to_string()));
        }

        let input_value: Value = serde_json::from_value(serde_json::Value::Object(input_json))
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        args.insert(async_graphql::Name::new("input"), input_value);

        let result = registry
            .resolve_field("Mutation", "updateIssue", Value::Object(args), Value::Null)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        // Convert async_graphql::Value to Option<Issue>
        let json_value = serde_json::to_value(&result)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        if json_value.is_null() {
            return Ok(None);
        }

        let issue = Issue {
            id: json_value.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            title: json_value.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            description: json_value.get("description").and_then(|v| v.as_str()).map(|s| s.to_string()),
            status: match json_value.get("status").and_then(|v| v.as_str()).unwrap_or("OPEN") {
                "CLOSED" => IssueStatus::Closed,
                "IN_PROGRESS" => IssueStatus::InProgress,
                _ => IssueStatus::Open,
            },
            created_at: json_value.get("createdAt").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        };

        Ok(Some(issue))
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
