mod api;
mod auth;
mod config;
mod db;
mod extensions;
mod graphql;
mod group;
mod repository;
mod router;
mod supervisor;
mod validation;

use anyhow::Context as _;
use std::path::PathBuf;
use std::sync::Arc;
use supervisor::Supervisor;

use api::auth_handlers::AuthState;
use api::run_api;
use auth::{AtProtoAuthClient, AuthConfig, SessionManager};
use repository::RepositoryStorage;
use router::RouterState;

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

    // Handle extensions directory - use ./extensions relative to server binary
    let extensions_dir = std::env::var("FORGE_EXTENSIONS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./extensions"));

    let mut extension_manager =
        extensions::ExtensionManager::new(extensions_dir.clone(), db_root_path.clone());

    // Load configuration and extensions
    match config::loader::load_with_discovery() {
        Ok(config) if !config.extensions.oci.is_empty() || !config.extensions.local.is_empty() => {
            tracing::info!("Loading extensions from configuration");
            if let Err(e) = extension_manager
                .load_extensions_from_config(&config.extensions)
                .await
            {
                tracing::error!("Failed to load extensions from config: {}", e);
            }
        }
        Ok(_) => {
            // No extensions in config, fall back to directory scanning
            tracing::info!("No extensions in config, scanning directory");
            if let Err(e) = extension_manager.load_extensions().await {
                tracing::warn!("Failed to load extensions from directory: {}", e);
            }
        }
        Err(e) => {
            tracing::warn!(
                "Failed to load config ({}), falling back to directory scan",
                e
            );
            if let Err(e) = extension_manager.load_extensions().await {
                tracing::warn!("Failed to load extensions from directory: {}", e);
            }
        }
    }

    let extension_manager = Arc::new(extension_manager);

    // Initialise Hive Router state
    let router_state = Arc::new(
        RouterState::new(pool.clone(), storage.clone(), extension_manager.clone())
            .context("Failed to initialise router state")?,
    );

    // Initialize authentication if configured
    let auth_state = initialize_auth();

    let mut supervisor = Supervisor::new();

    supervisor.spawn("api", move |shutdown| async move {
        run_api(router_state, auth_state, shutdown).await
    });

    supervisor.run().await
}

/// Initialize authentication if environment variables are configured
fn initialize_auth() -> Option<Arc<AuthState>> {
    let client_id = std::env::var("ATPROTO_CLIENT_ID").ok()?;
    let client_secret = std::env::var("ATPROTO_CLIENT_SECRET").ok()?;
    let redirect_uri = std::env::var("ATPROTO_REDIRECT_URI")
        .unwrap_or_else(|_| "http://localhost:8000/auth/callback".to_string());

    tracing::info!("Initializing ATProto authentication");

    let config = AuthConfig::bluesky_default(client_id, client_secret, redirect_uri);

    match AtProtoAuthClient::new(config) {
        Ok(oauth_client) => {
            let session_manager = SessionManager::new();
            Some(Arc::new(AuthState {
                oauth_client,
                session_manager,
            }))
        }
        Err(e) => {
            tracing::error!("Failed to initialize OAuth client: {}", e);
            None
        }
    }
}
