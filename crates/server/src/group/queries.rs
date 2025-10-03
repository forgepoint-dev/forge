use sqlx::SqlitePool;

use super::db::{fetch_group_by_id, resolve_group_by_path};
use super::models::GroupRecord;
use crate::repository::queries::get_repositories_for_group;

pub async fn get_all_groups_raw(pool: &SqlitePool) -> anyhow::Result<Vec<GroupRecord>> {
    let records =
        sqlx::query_as::<_, GroupRecord>("SELECT id, slug, parent FROM groups ORDER BY slug")
            .fetch_all(pool)
            .await?;
    Ok(records)
}

pub async fn get_group_raw(pool: &SqlitePool, path: String) -> anyhow::Result<Option<GroupRecord>> {
    resolve_group_by_path(pool, &path)
        .await
        .map_err(|e| anyhow::anyhow!(e))
}

pub async fn get_group_parent(
    pool: &SqlitePool,
    parent_id: &str,
) -> anyhow::Result<Option<GroupRecord>> {
    Ok(fetch_group_by_id(pool, parent_id).await?)
}

pub async fn repositories_for_group(
    pool: &SqlitePool,
    group_id: &str,
) -> anyhow::Result<Vec<crate::repository::models::RepositorySummary>> {
    get_repositories_for_group(pool, group_id).await
}
