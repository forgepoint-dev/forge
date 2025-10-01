use sqlx::SqlitePool;

use super::db::resolve_repository_by_path;
use super::entries::{normalize_tree_path, read_repository_entries};
use super::models::{
    RepositoryEntriesPayload, RepositoryRecord, RepositorySummary, RepositorySummaryRow,
};
use super::storage::RepositoryStorage;
use crate::validation::slug::validate_slug;

pub async fn get_all_repositories_raw(pool: &SqlitePool) -> anyhow::Result<Vec<RepositoryRecord>> {
    let records = sqlx::query_as::<_, RepositoryRecord>(
        "SELECT id, slug, \"group\" as group_id, remote_url FROM repositories ORDER BY slug",
    )
    .fetch_all(pool)
    .await?;

    Ok(records)
}

pub async fn get_repository_raw(
    pool: &SqlitePool,
    path: String,
) -> anyhow::Result<Option<RepositoryRecord>> {
    resolve_repository_by_path(pool, &path)
        .await
        .map_err(|e| anyhow::anyhow!(e))
}

pub async fn browse_repository_raw(
    pool: &SqlitePool,
    storage: &RepositoryStorage,
    path: String,
    tree_path: Option<String>,
) -> anyhow::Result<Option<RepositoryEntriesPayload>> {
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

    let record = resolve_repository_by_path(pool, &path).await?;
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
) -> anyhow::Result<Vec<RepositorySummary>> {
    let rows = sqlx::query_as::<_, RepositorySummaryRow>(
        "SELECT id, slug, remote_url FROM repositories WHERE \"group\" = ? ORDER BY slug",
    )
    .bind(group_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(RepositorySummary::from).collect())
}
