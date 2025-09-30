use super::models::GroupRecord;
use sqlx::SqlitePool;

pub async fn fetch_group_by_id(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<GroupRecord>, sqlx::Error> {
    sqlx::query_as::<_, GroupRecord>("SELECT id, slug, parent FROM groups WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn slug_conflicts_for_group(
    pool: &SqlitePool,
    parent_id: Option<&str>,
    slug: &str,
) -> Result<bool, sqlx::Error> {
    if let Some(parent_id) = parent_id {
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM groups WHERE slug = ? AND parent = ? LIMIT 1")
                .bind(slug)
                .bind(parent_id)
                .fetch_optional(pool)
                .await?;
        Ok(exists.is_some())
    } else {
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM groups WHERE slug = ? AND parent IS NULL LIMIT 1")
                .bind(slug)
                .fetch_optional(pool)
                .await?;
        Ok(exists.is_some())
    }
}

pub async fn resolve_group_by_path(
    pool: &SqlitePool,
    path: &str,
) -> Result<Option<GroupRecord>, sqlx::Error> {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return Ok(None);
    }

    let mut parent_id: Option<String> = None;
    let mut current: Option<GroupRecord> = None;

    for slug in segments {
        let record = if let Some(ref parent) = parent_id {
            sqlx::query_as::<_, GroupRecord>(
                "SELECT id, slug, parent FROM groups WHERE slug = ? AND parent = ?",
            )
            .bind(slug)
            .bind(parent)
            .fetch_optional(pool)
            .await
        } else {
            sqlx::query_as::<_, GroupRecord>(
                "SELECT id, slug, parent FROM groups WHERE slug = ? AND parent IS NULL",
            )
            .bind(slug)
            .fetch_optional(pool)
            .await
        }?;

        match record {
            Some(row) => {
                parent_id = Some(row.id.clone());
                current = Some(row);
            }
            None => return Ok(None),
        }
    }

    Ok(current)
}
