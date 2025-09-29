use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

const FORGE_DB_FILENAME: &str = "forge.db";

/// Initialize the forge metadata database, running migrations as needed.
pub async fn init_pool() -> Result<(SqlitePool, PathBuf)> {
    let db_root =
        std::env::var("FORGE_DB_PATH").context("FORGE_DB_PATH environment variable must be set")?;

    let db_root_path = normalize_path(db_root)?;
    std::fs::create_dir_all(&db_root_path)
        .with_context(|| format!("failed to create DB path: {}", db_root_path.display()))?;

    let forge_db_path = db_root_path.join(FORGE_DB_FILENAME);
    let db_uri = format!("sqlite://{}", forge_db_path.to_string_lossy());

    let connect_options = SqliteConnectOptions::from_str(&db_uri)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok((pool, db_root_path))
}

pub(crate) fn normalize_path<P: Into<PathBuf>>(path: P) -> Result<PathBuf> {
    let path = path.into();
    if path.is_absolute() {
        return Ok(path);
    }

    let cwd = std::env::current_dir().context("failed to read current working directory")?;
    Ok(cwd.join(path))
}
