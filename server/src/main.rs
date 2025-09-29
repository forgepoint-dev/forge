mod api;
mod db;
mod extensions;
mod graphql;
mod group;
mod repository;
mod supervisor;
mod validation;

use anyhow::Context as _;
use supervisor::Supervisor;

use api::run_api;
use graphql::build_schema;
use repository::RepositoryStorage;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let (pool, db_root_path) = db::init_pool().await?;

    let repos_root_raw = std::env::var("FORGE_REPOS_PATH")
        .with_context(|| "FORGE_REPOS_PATH environment variable must be set".to_string())?;
    let repos_root = db::normalize_path(repos_root_raw)?;

    std::fs::create_dir_all(&repos_root).with_context(|| {
        format!(
            "failed to create repository root directory: {}",
            repos_root.display()
        )
    })?;

    let remote_cache_root = db_root_path.join("remote-cache");
    std::fs::create_dir_all(&remote_cache_root).with_context(|| {
        format!(
            "failed to create remote cache directory: {}",
            remote_cache_root.display()
        )
    })?;

    let storage = RepositoryStorage::new(repos_root, remote_cache_root);

    let extensions_dir = db_root_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("server/extensions");

    let mut extension_manager =
        extensions::ExtensionManager::new(extensions_dir, db_root_path.clone());

    if let Err(e) = extension_manager.load_extensions().await {
        tracing::warn!("Failed to load extensions: {}", e);
    }

    let schema = build_schema(pool.clone(), storage.clone(), extension_manager);

    let mut supervisor = Supervisor::new();

    supervisor.spawn("api", move |shutdown| async move {
        run_api(schema, shutdown).await
    });

    supervisor.run().await
}