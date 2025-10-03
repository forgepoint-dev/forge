use sqlx::SqlitePool;

use super::db::{remote_url_exists, slug_conflicts_for_repository};
use super::models::RepositoryRecord;
use crate::group::db::fetch_group_by_id;
use crate::repository::storage::RepositoryStorage;
use crate::validation::slug::validate_slug;
use crate::validation::url::normalize_remote_repository;

#[derive(Clone, Debug)]
pub struct CreateRepositoryInput {
    pub slug: String,
    pub group: Option<String>,
}

pub async fn create_repository_raw(
    pool: &SqlitePool,
    input: CreateRepositoryInput,
) -> anyhow::Result<RepositoryRecord> {
    validate_slug(&input.slug)?;

    let group_id = match input.group {
        Some(ref id) => {
            let exists = fetch_group_by_id(pool, id).await?.is_some();
            if !exists {
                return Err(anyhow::anyhow!("group not found"));
            }
            Some(id.clone())
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

pub async fn link_remote_repository_raw(
    pool: &SqlitePool,
    _storage: &RepositoryStorage,
    url: String,
) -> anyhow::Result<RepositoryRecord> {
    let (normalized_url, slug) = normalize_remote_repository(&url)?;

    if remote_url_exists(pool, &normalized_url).await? {
        return Err(anyhow::anyhow!("remote repository already linked"));
    }

    validate_slug(&slug)?;

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
