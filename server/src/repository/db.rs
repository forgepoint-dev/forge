use super::models::RepositoryRecord;
use crate::group::db::resolve_group_by_path;
use sqlx::SqlitePool;

pub async fn slug_conflicts_for_repository(
    pool: &SqlitePool,
    group_id: Option<&str>,
    slug: &str,
) -> Result<bool, sqlx::Error> {
    if let Some(group_id) = group_id {
        let exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM repositories WHERE slug = ? AND \"group\" = ? LIMIT 1",
        )
        .bind(slug)
        .bind(group_id)
        .fetch_optional(pool)
        .await?;
        Ok(exists.is_some())
    } else {
        let exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM repositories WHERE slug = ? AND \"group\" IS NULL LIMIT 1",
        )
        .bind(slug)
        .fetch_optional(pool)
        .await?;
        Ok(exists.is_some())
    }
}

pub async fn resolve_repository_by_path(
    pool: &SqlitePool,
    path: &str,
) -> Result<Option<RepositoryRecord>, sqlx::Error> {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return Ok(None);
    }

    let (group_segments, repo_part) = segments.split_at(segments.len() - 1);
    let repo_slug = repo_part[0];

    let group_id = if group_segments.is_empty() {
        None
    } else {
        let group_path = group_segments.join("/");
        match resolve_group_by_path(pool, &group_path).await? {
            Some(group) => Some(group.id),
            None => return Ok(None),
        }
    };

    match group_id.as_deref() {
        Some(group_id) => {
            sqlx::query_as::<_, RepositoryRecord>(
                "SELECT id, slug, \"group\" as group_id, remote_url FROM repositories WHERE slug = ? AND \"group\" = ?",
            )
            .bind(repo_slug)
            .bind(group_id)
            .fetch_optional(pool)
            .await
        }
        None => {
            sqlx::query_as::<_, RepositoryRecord>(
                "SELECT id, slug, \"group\" as group_id, remote_url FROM repositories WHERE slug = ? AND \"group\" IS NULL",
            )
            .bind(repo_slug)
            .fetch_optional(pool)
            .await
        }
    }
}

pub async fn remote_url_exists(pool: &SqlitePool, remote_url: &str) -> Result<bool, sqlx::Error> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM repositories WHERE remote_url = ? LIMIT 1")
            .bind(remote_url)
            .fetch_optional(pool)
            .await?;

    Ok(exists.is_some())
}
