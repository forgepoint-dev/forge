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
use std::path::PathBuf;

use api::{run_api};
use graphql::build_schema;
use repository::RepositoryStorage;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let (pool, db_root_path) = db::init_pool().await?;

    // Handle repository paths - use temp dir in memory mode
    let repos_root = if std::env::var("FORGE_IN_MEMORY_DB").unwrap_or_default() == "true" {
        tracing::info!("Using temporary directory for repositories (in-memory mode)");
        std::env::temp_dir().join("forge-repos")
    } else {
        let repos_root_raw = std::env::var("FORGE_REPOS_PATH")
            .with_context(|| "FORGE_REPOS_PATH environment variable must be set".to_string())?;
        db::normalize_path(repos_root_raw)?
    };

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

    // Handle extensions directory
    let extensions_dir = if std::env::var("FORGE_IN_MEMORY_DB").unwrap_or_default() == "true" {
        // In memory mode, look for extensions relative to current directory
        std::env::var("FORGE_EXTENSIONS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("extensions_dir"))
    } else {
        // Normal mode
        std::env::var("FORGE_EXTENSIONS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                db_root_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .join("server/extensions")
            })
    };

    let mut extension_manager =
        extensions::ExtensionManager::new(extensions_dir.clone(), db_root_path.clone());

    if let Err(e) = extension_manager.load_extensions().await {
        tracing::warn!("Failed to load extensions: {}", e);
    }

    // Create the GraphQL schema
    let schema = build_schema(pool.clone(), storage.clone(), extension_manager);

    let mut supervisor = Supervisor::new();

    supervisor.spawn("api", move |shutdown| async move {
        run_api(schema, shutdown).await
    });

    supervisor.run().await
}