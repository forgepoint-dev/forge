use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use cuid2::create_id;
use server::extensions::ExtensionManager;
use server::repository::RepositoryStorage;
use server::router::{GraphQLExecutionRequest, RouterState};
use server::test_helpers;
use tempfile::TempDir;

type SonicValue = sonic_rs::Value;

struct RouterTestContext {
    router: RouterState,
    pool: sqlx::SqlitePool,
    _local_dir: TempDir,
    _remote_dir: TempDir,
    _extensions_dir: TempDir,
    _extensions_db_dir: TempDir,
}

async fn setup_router_state() -> Result<RouterTestContext> {
    let pool = test_helpers::create_test_pool().await?;

    let local_dir = tempfile::tempdir()?;
    let remote_dir = tempfile::tempdir()?;
    let storage = RepositoryStorage::new(
        local_dir.path().to_path_buf(),
        remote_dir.path().to_path_buf(),
    );

    let extensions_dir = tempfile::tempdir()?;
    let extensions_db_dir = tempfile::tempdir()?;
    let extension_manager = ExtensionManager::new(
        extensions_dir.path().to_path_buf(),
        extensions_db_dir.path().to_path_buf(),
    );

    let router = RouterState::new(pool.clone(), storage, Arc::new(extension_manager))?;

    Ok(RouterTestContext {
        router,
        pool,
        _local_dir: local_dir,
        _remote_dir: remote_dir,
        _extensions_dir: extensions_dir,
        _extensions_db_dir: extensions_db_dir,
    })
}

#[tokio::test]
async fn query_core_data_through_router() -> Result<()> {
    let ctx = setup_router_state().await?;

    let group_id = create_id();
    sqlx::query("INSERT INTO groups (id, slug, parent) VALUES (?, ?, NULL)")
        .bind(&group_id)
        .bind("platform")
        .execute(&ctx.pool)
        .await?;

    let request = GraphQLExecutionRequest {
        query: "query { getAllGroups { id slug } }".to_string(),
        operation_name: None,
        variables: None,
    };

    let response = ctx.router.execute(request).await?;
    let groups = response
        .get("data")
        .and_then(|data| data.get("getAllGroups"))
        .and_then(|value| value.as_array())
        .expect("getAllGroups should be an array");

    assert_eq!(groups.len(), 1);
    assert_eq!(
        groups[0].get("id").and_then(|v| v.as_str()),
        Some(group_id.as_str())
    );
    assert_eq!(
        groups[0].get("slug").and_then(|v| v.as_str()),
        Some("platform")
    );

    Ok(())
}

#[tokio::test]
async fn mutation_creates_group_via_router() -> Result<()> {
    let ctx = setup_router_state().await?;

    let mut variables: HashMap<String, SonicValue> = HashMap::new();
    variables.insert("slug".to_string(), sonic_rs::to_value("new-group")?);

    let request = GraphQLExecutionRequest {
        query: "mutation CreateGroup($slug: String!) { createGroup(input: { slug: $slug, parent: null }) { slug } }".to_string(),
        operation_name: Some("CreateGroup".to_string()),
        variables: Some(variables),
    };

    let response = ctx.router.execute(request).await?;
    let slug = response
        .get("data")
        .and_then(|data| data.get("createGroup"))
        .and_then(|value| value.get("slug"))
        .and_then(|value| value.as_str())
        .expect("mutation should return slug");
    assert_eq!(slug, "new-group");

    let exists: Option<(String,)> = sqlx::query_as("SELECT slug FROM groups WHERE slug = ?")
        .bind("new-group")
        .fetch_optional(&ctx.pool)
        .await?;
    assert!(
        exists.is_some(),
        "group should be inserted into the database"
    );

    Ok(())
}

#[tokio::test]
async fn repository_group_field_is_projected() -> Result<()> {
    let ctx = setup_router_state().await?;

    let group_id = create_id();
    sqlx::query("INSERT INTO groups (id, slug, parent) VALUES (?, ?, NULL)")
        .bind(&group_id)
        .bind("apps")
        .execute(&ctx.pool)
        .await?;

    let repo_id = create_id();
    sqlx::query(
        "INSERT INTO repositories (id, slug, \"group\", remote_url) VALUES (?, ?, ?, NULL)",
    )
    .bind(&repo_id)
    .bind("portal")
    .bind(&group_id)
    .execute(&ctx.pool)
    .await?;

    let request = GraphQLExecutionRequest {
        query: format!(
            "query {{ getRepository(path: \"apps/portal\") {{ id slug group {{ id slug }} }} }}"
        ),
        operation_name: None,
        variables: None,
    };

    let response = ctx.router.execute(request).await?;
    let repo = response
        .get("data")
        .and_then(|data| data.get("getRepository"))
        .and_then(|value| value.as_object())
        .expect("repository response should be an object");

    assert_eq!(
        repo.get("id").and_then(|v| v.as_str()),
        Some(repo_id.as_str())
    );
    let group = repo
        .get("group")
        .and_then(|value| value.as_object())
        .expect("group field should be projected");
    assert_eq!(
        group.get("id").and_then(|v| v.as_str()),
        Some(group_id.as_str())
    );
    assert_eq!(group.get("slug").and_then(|v| v.as_str()), Some("apps"));

    Ok(())
}
