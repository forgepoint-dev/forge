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
use auth::{AtProtoAuthClient, AuthConfig, SessionManager, SqliteAuthStore};
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

    // Initialize authentication (public client by default)
    let auth_state = initialize_auth_async().await;

    let mut supervisor = Supervisor::new();

    // Spawn auth cleanup task if enabled
    if let Some(auth_state_arc) = auth_state.clone() {
        let store_for_clean = auth_state_arc.auth_store.clone();
        supervisor.spawn("auth-cleaner", move |shutdown| {
            let store = store_for_clean.clone();
            async move {
                let ttl: i64 = std::env::var("FORGE_AUTH_FLOW_TTL_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(30 * 60);
                let interval_ms: u64 = std::env::var("FORGE_AUTH_CLEAN_INTERVAL_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(5 * 60) * 1000;
                let mut ticker = tokio::time::interval(std::time::Duration::from_millis(interval_ms));
                loop {
                    tokio::select! {
                        _ = shutdown.cancelled() => { break; }
                        _ = ticker.tick() => {
                            match store.prune_older_than(ttl).await {
                                Ok(n) if n > 0 => tracing::info!("pruned {} stale auth flows", n),
                                Ok(_) => {},
                                Err(e) => tracing::warn!("auth flow prune failed: {}", e),
                            }
                        }
                    }
                }
                Ok(())
            }
        });
        // Periodic VACUUM/optimize
        let store_for_vacuum = auth_state_arc.auth_store.clone();
        supervisor.spawn("auth-vacuum", move |shutdown| {
            async move {
                let every_ms: u64 = std::env::var("FORGE_AUTH_VACUUM_INTERVAL_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(6 * 60 * 60) * 1000; // default 6h
                let mut ticker = tokio::time::interval(std::time::Duration::from_millis(every_ms));
                loop {
                    tokio::select! {
                        _ = shutdown.cancelled() => { break; }
                        _ = ticker.tick() => {
                            if let Err(e) = store_for_vacuum.vacuum().await { tracing::warn!("auth vacuum failed: {}", e); }
                            else { tracing::info!("auth db vacuumed"); }
                        }
                    }
                }
                Ok(())
            }
        });
    }

    supervisor.spawn("api", move |shutdown| async move {
        run_api(router_state, auth_state, shutdown).await
    });

    supervisor.run().await
}

/// Initialize authentication if environment variables are configured
async fn initialize_auth_async() -> Option<Arc<AuthState>> {
    // Public client by default, with dynamic client metadata URL as client_id
    let mut redirect_uri = std::env::var("ATPROTO_REDIRECT_URI")
        .unwrap_or_else(|_| "http://127.0.0.1:8000/auth/callback".to_string());
    if redirect_uri.contains("://localhost") {
        let fixed = redirect_uri.replace("://localhost", "://127.0.0.1");
        tracing::warn!("ATPROTO_REDIRECT_URI uses localhost; rewriting to {} per RFC 8252 loopback guidance", fixed);
        redirect_uri = fixed;
    }

    // Compute base URL for the server to host client metadata
    let public_base = std::env::var("FORGE_PUBLIC_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8000".to_string());
    let base_trimmed = public_base.trim_end_matches('/').to_string();
    // Determine scope prior to building client_id for localhost mode
    let scope = std::env::var("ATPROTO_OAUTH_SCOPE").unwrap_or_else(|_| "atproto".to_string());
    let client_id = if base_trimmed.contains("://localhost") || base_trimmed.contains("://127.0.0.1") {
        // Use special localhost client as per ATProto OAuth profile.
        // Declare redirect_uri (path-sensitive, port ignored) and scope in client_id query.
        let scope_q = urlencoding::encode(&scope);
        let enc_redirect = urlencoding::encode(&redirect_uri);
        format!("http://localhost?redirect_uri={}&scope={}", enc_redirect, scope_q)
    } else {
        format!("{}/client-metadata.json", base_trimmed)
    };

    // Optional secret if someone wants to use client_secret
    let client_secret = std::env::var("ATPROTO_CLIENT_SECRET").ok();

    tracing::info!("Initializing ATProto OAuth authentication (public client)");

    let config = AuthConfig {
        client_id,
        client_secret,
        redirect_uri,
        scope,
    };

    match AtProtoAuthClient::new(config) {
        Ok(oauth_client) => {
            let session_manager = SessionManager::new();
            let auth_db_path = std::env::var("FORGE_AUTH_DB_PATH").unwrap_or_else(|_| "server/.forge/auth.db".to_string());
            let auth_store = match SqliteAuthStore::new(&auth_db_path).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("Failed to init auth store: {}", e);
                    return None;
                }
            };
            Some(Arc::new(AuthState {
                oauth_client,
                session_manager,
                auth_store,
            }))
        }
        Err(e) => {
            tracing::error!("Failed to initialize ATProto OAuth client: {}", e);
            None
        }
    }
}
