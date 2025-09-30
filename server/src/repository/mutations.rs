use async_graphql::{Context, ID, InputObject};
use sqlx::SqlitePool;

use super::db::{remote_url_exists, slug_conflicts_for_repository};
use super::models::{RepositoryNode, RepositoryRecord};
use crate::graphql::errors::{bad_user_input, internal_error};
use crate::group::db::fetch_group_by_id;
use crate::validation::slug::validate_slug;
use crate::validation::url::normalize_remote_repository;

#[derive(InputObject)]
pub struct CreateRepositoryInput {
    pub slug: String,
    #[graphql(name = "group")]
    pub group: Option<ID>,
}

pub async fn create_repository(
    ctx: &Context<'_>,
    input: CreateRepositoryInput,
) -> async_graphql::Result<RepositoryNode> {
    validate_slug(&input.slug)?;

    let pool = ctx.data::<SqlitePool>()?;
    let group_id = match input.group {
        Some(id) => {
            let id = id.to_string();
            let exists = fetch_group_by_id(pool, &id)
                .await
                .map_err(internal_error)?
                .is_some();
            if !exists {
                return Err(bad_user_input("group not found"));
            }
            Some(id)
        }
        None => None,
    };

    if slug_conflicts_for_repository(pool, group_id.as_deref(), &input.slug)
        .await
        .map_err(internal_error)?
    {
        return Err(bad_user_input("slug already exists in this group"));
    }

    let id = cuid2::create_id();
    sqlx::query("INSERT INTO repositories (id, slug, \"group\", remote_url) VALUES (?, ?, ?, ?) ")
        .bind(&id)
        .bind(&input.slug)
        .bind(group_id.as_ref())
        .bind::<Option<&str>>(None)
        .execute(pool)
        .await
        .map_err(internal_error)?;

    let record = RepositoryRecord {
        id,
        slug: input.slug,
        group_id,
        remote_url: None,
    };

    Ok(RepositoryNode::from(record))
}

pub async fn link_remote_repository(
    ctx: &Context<'_>,
    url: String,
) -> async_graphql::Result<RepositoryNode> {
    let pool = ctx.data::<SqlitePool>()?;

    let (normalized_url, slug) = normalize_remote_repository(&url)?;

    if remote_url_exists(pool, &normalized_url)
        .await
        .map_err(internal_error)?
    {
        return Err(bad_user_input("remote repository already linked"));
    }

    validate_slug(&slug)?;

    if slug_conflicts_for_repository(pool, None, &slug)
        .await
        .map_err(internal_error)?
    {
        return Err(bad_user_input("slug already exists at the root"));
    }

    let id = cuid2::create_id();
    sqlx::query(
        "INSERT INTO repositories (id, slug, \"group\", remote_url) VALUES (?, ?, NULL, ?)",
    )
    .bind(&id)
    .bind(&slug)
    .bind(&normalized_url)
    .execute(pool)
    .await
    .map_err(internal_error)?;

    let record = RepositoryRecord {
        id,
        slug,
        group_id: None,
        remote_url: Some(normalized_url),
    };

    Ok(RepositoryNode::from(record))
}

// Raw versions for dynamic schema
#[allow(dead_code)]
pub async fn create_repository_raw(
    pool: &SqlitePool,
    _storage: &super::storage::RepositoryStorage,
    input: CreateRepositoryInput,
) -> anyhow::Result<RepositoryRecord> {
    validate_slug(&input.slug)
        .map_err(|e| anyhow::anyhow!(e.message))?;

    let group_id = match input.group {
        Some(id) => {
            let id = id.to_string();
            let exists = fetch_group_by_id(pool, &id).await?.is_some();
            if !exists {
                return Err(anyhow::anyhow!("group not found"));
            }
            Some(id)
        }
        None => None,
    };

    if slug_conflicts_for_repository(pool, group_id.as_deref(), &input.slug).await? {
        return Err(anyhow::anyhow!("slug already exists in this group"));
    }

    let id = cuid2::create_id();
    sqlx::query("INSERT INTO repositories (id, slug, \"group\", remote_url) VALUES (?, ?, ?, ?) ")
        .bind(&id)
        .bind(&input.slug)
        .bind(group_id.as_ref())
        .bind::<Option<&str>>(None)
        .execute(pool)
        .await?;

    Ok(RepositoryRecord {
        id,
        slug: input.slug,
        group_id,
        remote_url: None,
    })
}

#[allow(dead_code)]
pub async fn link_remote_repository_raw(
    pool: &SqlitePool,
    _storage: &super::storage::RepositoryStorage,
    url: String,
) -> anyhow::Result<RepositoryRecord> {
    let (normalized_url, slug) = normalize_remote_repository(&url)
        .map_err(|e| anyhow::anyhow!(e.message))?;

    if remote_url_exists(pool, &normalized_url).await? {
        return Err(anyhow::anyhow!("remote repository already linked"));
    }

    validate_slug(&slug)
        .map_err(|e| anyhow::anyhow!(e.message))?;

    if slug_conflicts_for_repository(pool, None, &slug).await? {
        return Err(anyhow::anyhow!("slug already exists at the root"));
    }

    let id = cuid2::create_id();
    sqlx::query(
        "INSERT INTO repositories (id, slug, \"group\", remote_url) VALUES (?, ?, NULL, ?)",
    )
    .bind(&id)
    .bind(&slug)
    .bind(&normalized_url)
    .execute(pool)
    .await?;

    Ok(RepositoryRecord {
        id,
        slug,
        group_id: None,
        remote_url: Some(normalized_url),
    })
}
