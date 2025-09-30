use async_graphql::{Context, ID, Object};
use sqlx::SqlitePool;

use super::db::{fetch_group_by_id, resolve_repository_by_path};
use super::entries::{normalize_tree_path, read_repository_entries};
use super::models::{
    RepositoryEntriesPayload, RepositoryNode, RepositoryRecord, RepositorySummary,
    RepositorySummaryRow,
};
use super::storage::RepositoryStorage;
use crate::graphql::errors::internal_error;
use crate::group::models::GroupSummary;
use crate::validation::slug::validate_slug;

#[Object]
impl RepositoryNode {
    async fn id(&self) -> ID {
        ID::from(self.0.id.clone())
    }

    async fn slug(&self) -> &str {
        &self.0.slug
    }

    async fn group(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<GroupSummary>> {
        let Some(ref group_id) = self.0.group_id else {
            return Ok(None);
        };

        let pool = ctx.data::<SqlitePool>()?;
        let group = fetch_group_by_id(pool, group_id)
            .await
            .map_err(internal_error)?;
        Ok(group.map(GroupSummary::from))
    }

    #[graphql(name = "isRemote")]
    async fn is_remote(&self) -> bool {
        self.0.remote_url.is_some()
    }

    #[graphql(name = "remoteUrl")]
    async fn remote_url(&self) -> Option<&str> {
        self.0.remote_url.as_deref()
    }
}

pub async fn get_all_repositories(ctx: &Context<'_>) -> async_graphql::Result<Vec<RepositoryNode>> {
    let pool = ctx.data::<SqlitePool>()?;
    let records = sqlx::query_as::<_, RepositoryRecord>(
        "SELECT id, slug, \"group\" as group_id, remote_url FROM repositories ORDER BY slug",
    )
    .fetch_all(pool)
    .await
    .map_err(internal_error)?;

    Ok(records.into_iter().map(RepositoryNode::from).collect())
}

pub async fn get_repository(
    ctx: &Context<'_>,
    path: String,
) -> async_graphql::Result<Option<RepositoryNode>> {
    let pool = ctx.data::<SqlitePool>()?;
    let record = resolve_repository_by_path(pool, &path)
        .await
        .map_err(internal_error)?;
    Ok(record.map(RepositoryNode::from))
}

pub async fn browse_repository(
    ctx: &Context<'_>,
    path: String,
    tree_path: Option<String>,
) -> async_graphql::Result<Option<RepositoryEntriesPayload>> {
    let pool = ctx.data::<SqlitePool>()?;
    let storage = ctx.data::<RepositoryStorage>()?;

    let segments: Vec<String> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.to_string())
        .collect();

    if segments.is_empty() {
        return Ok(None);
    }

    for segment in &segments {
        validate_slug(segment)?;
    }

    let record = resolve_repository_by_path(pool, &path)
        .await
        .map_err(internal_error)?;

    let Some(record) = record else {
        return Ok(None);
    };

    let normalized_tree_path = normalize_tree_path(tree_path)?;

    let repository_path = if record.remote_url.is_some() {
        storage.ensure_remote_repository(&record).await?
    } else {
        storage.ensure_local_repository(&segments)?
    };

    let entries = read_repository_entries(repository_path, normalized_tree_path.clone()).await?;

    Ok(Some(RepositoryEntriesPayload {
        tree_path: normalized_tree_path,
        entries,
    }))
}

pub async fn get_repositories_for_group(
    pool: &SqlitePool,
    group_id: &str,
) -> async_graphql::Result<Vec<RepositorySummary>> {
    let rows = sqlx::query_as::<_, RepositorySummaryRow>(
        "SELECT id, slug, remote_url FROM repositories WHERE \"group\" = ? ORDER BY slug",
    )
    .bind(group_id)
    .fetch_all(pool)
    .await
    .map_err(internal_error)?;

    Ok(rows.into_iter().map(RepositorySummary::from).collect())
}

// Raw versions for dynamic schema
#[allow(dead_code)]
pub async fn get_all_repositories_raw(pool: &SqlitePool) -> anyhow::Result<Vec<RepositoryRecord>> {
    let records = sqlx::query_as::<_, RepositoryRecord>(
        "SELECT id, slug, \"group\" as group_id, remote_url FROM repositories ORDER BY slug",
    )
    .fetch_all(pool)
    .await?;
    Ok(records)
}

#[allow(dead_code)]
pub async fn get_repository_raw(
    pool: &SqlitePool,
    path: String,
) -> anyhow::Result<Option<RepositoryRecord>> {
    resolve_repository_by_path(pool, &path).await
        .map_err(|e| anyhow::anyhow!(e))
}

#[allow(dead_code)]
pub async fn browse_repository_raw(
    pool: &SqlitePool,
    storage: &RepositoryStorage,
    path: String,
    tree_path: Option<String>,
) -> anyhow::Result<Option<RepositoryEntriesPayload>> {
    use crate::validation::slug::validate_slug;

    let segments: Vec<String> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.to_string())
        .collect();

    if segments.is_empty() {
        return Ok(None);
    }

    for segment in &segments {
        validate_slug(segment)
            .map_err(|e| anyhow::anyhow!(e.message))?;
    }

    let record = resolve_repository_by_path(pool, &path).await?;
    let Some(record) = record else {
        return Ok(None);
    };

    let normalized_tree_path = normalize_tree_path(tree_path)
        .map_err(|e| anyhow::anyhow!(e.message))?;

    let repository_path = if record.remote_url.is_some() {
        storage.ensure_remote_repository(&record).await
            .map_err(|e| anyhow::anyhow!(e.message))?
    } else {
        storage.ensure_local_repository(&segments)
            .map_err(|e| anyhow::anyhow!(e.message))?
    };

    let entries = read_repository_entries(repository_path, normalized_tree_path.clone()).await
        .map_err(|e| anyhow::anyhow!(e.message))?;

    Ok(Some(RepositoryEntriesPayload {
        tree_path: normalized_tree_path,
        entries,
    }))
}
