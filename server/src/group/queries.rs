use async_graphql::{Context, ID, Object};
use sqlx::SqlitePool;

use super::db::{fetch_group_by_id, resolve_group_by_path};
use super::models::{GroupNode, GroupRecord, GroupSummary};
use crate::graphql::errors::internal_error;
use crate::repository::models::RepositorySummary;
use crate::repository::queries::get_repositories_for_group;

#[Object]
impl GroupNode {
    async fn id(&self) -> ID {
        ID::from(self.0.id.clone())
    }

    async fn slug(&self) -> &str {
        &self.0.slug
    }

    async fn parent(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<GroupSummary>> {
        let Some(ref parent_id) = self.0.parent else {
            return Ok(None);
        };

        let pool = ctx.data::<SqlitePool>()?;
        let parent = fetch_group_by_id(pool, parent_id)
            .await
            .map_err(internal_error)?;
        Ok(parent.map(GroupSummary::from))
    }

    async fn repositories(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<RepositorySummary>> {
        let pool = ctx.data::<SqlitePool>()?;
        get_repositories_for_group(pool, &self.0.id).await
    }
}

pub async fn get_all_groups(ctx: &Context<'_>) -> async_graphql::Result<Vec<GroupNode>> {
    let pool = ctx.data::<SqlitePool>()?;
    let records =
        sqlx::query_as::<_, GroupRecord>("SELECT id, slug, parent FROM groups ORDER BY slug")
            .fetch_all(pool)
            .await
            .map_err(internal_error)?;

    Ok(records.into_iter().map(GroupNode::from).collect())
}

pub async fn get_group(
    ctx: &Context<'_>,
    path: String,
) -> async_graphql::Result<Option<GroupNode>> {
    let pool = ctx.data::<SqlitePool>()?;
    let record = resolve_group_by_path(pool, &path)
        .await
        .map_err(internal_error)?;
    Ok(record.map(GroupNode::from))
}

// Raw versions for dynamic schema
#[allow(dead_code)]
pub async fn get_all_groups_raw(pool: &SqlitePool) -> anyhow::Result<Vec<GroupRecord>> {
    let records =
        sqlx::query_as::<_, GroupRecord>("SELECT id, slug, parent FROM groups ORDER BY slug")
            .fetch_all(pool)
            .await?;
    Ok(records)
}

#[allow(dead_code)]
pub async fn get_group_raw(pool: &SqlitePool, path: String) -> anyhow::Result<Option<GroupRecord>> {
    resolve_group_by_path(pool, &path).await
        .map_err(|e| anyhow::anyhow!(e))
}
