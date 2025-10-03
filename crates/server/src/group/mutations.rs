use sqlx::SqlitePool;

use super::db::{fetch_group_by_id, slug_conflicts_for_group};
use super::models::GroupRecord;
use crate::validation::slug::validate_slug;

#[derive(Clone, Debug)]
pub struct CreateGroupInput {
    pub slug: String,
    pub parent: Option<String>,
}

pub async fn create_group_raw(
    pool: &SqlitePool,
    input: CreateGroupInput,
) -> anyhow::Result<GroupRecord> {
    validate_slug(&input.slug)?;

    let parent_id = match input.parent {
        Some(ref id) => {
            let exists = fetch_group_by_id(pool, id).await?.is_some();
            if !exists {
                return Err(anyhow::anyhow!("parent group not found"));
            }
            Some(id.clone())
        }
        None => None,
    };

    if slug_conflicts_for_group(pool, parent_id.as_deref(), &input.slug).await? {
        return Err(anyhow::anyhow!("slug already exists in this group"));
    }

    let id = cuid2::create_id();
    sqlx::query("INSERT INTO groups (id, slug, parent) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(&input.slug)
        .bind(parent_id.as_ref())
        .execute(pool)
        .await?;

    Ok(GroupRecord {
        id,
        slug: input.slug,
        parent: parent_id,
    })
}
