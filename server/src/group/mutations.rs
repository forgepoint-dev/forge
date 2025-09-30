use async_graphql::{Context, ID, InputObject};
use sqlx::SqlitePool;

use super::db::{fetch_group_by_id, slug_conflicts_for_group};
use super::models::{GroupNode, GroupRecord};
use crate::graphql::errors::{bad_user_input, internal_error};
use crate::validation::slug::validate_slug;

#[derive(InputObject)]
pub struct CreateGroupInput {
    pub slug: String,
    #[graphql(name = "parent")]
    pub parent: Option<ID>,
}

pub async fn create_group(
    ctx: &Context<'_>,
    input: CreateGroupInput,
) -> async_graphql::Result<GroupNode> {
    validate_slug(&input.slug)?;

    let pool = ctx.data::<SqlitePool>()?;
    let parent_id = match input.parent {
        Some(id) => {
            let id = id.to_string();
            let exists = fetch_group_by_id(pool, &id)
                .await
                .map_err(internal_error)?
                .is_some();
            if !exists {
                return Err(bad_user_input("parent group not found"));
            }
            Some(id)
        }
        None => None,
    };

    if slug_conflicts_for_group(pool, parent_id.as_deref(), &input.slug)
        .await
        .map_err(internal_error)?
    {
        return Err(bad_user_input("slug already exists in this group"));
    }

    let id = cuid2::create_id();
    sqlx::query("INSERT INTO groups (id, slug, parent) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(&input.slug)
        .bind(parent_id.as_ref())
        .execute(pool)
        .await
        .map_err(internal_error)?;

    let record = GroupRecord {
        id,
        slug: input.slug,
        parent: parent_id,
    };

    Ok(GroupNode::from(record))
}

// Raw version for dynamic schema
pub async fn create_group_raw(
    pool: &SqlitePool,
    input: CreateGroupInput,
) -> anyhow::Result<GroupRecord> {
    validate_slug(&input.slug)
        .map_err(|e| anyhow::anyhow!(e.message))?;

    let parent_id = match input.parent {
        Some(id) => {
            let id = id.to_string();
            let exists = fetch_group_by_id(pool, &id).await?.is_some();
            if !exists {
                return Err(anyhow::anyhow!("parent group not found"));
            }
            Some(id)
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
