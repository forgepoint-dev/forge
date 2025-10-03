use anyhow::Result;
use sqlx::{SqlitePool, Row, sqlite::{SqliteConnectOptions, SqliteJournalMode}};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct AuthFlowRecord {
    pub state: String,
    pub issuer: String,
    pub pds_url: String,
    pub code_verifier: String,
    pub dpop_pkcs8: Vec<u8>,
    pub dpop_jwk: String,
    pub dpop_nonce: Option<String>,
}

#[derive(Clone)]
pub struct SqliteAuthStore {
    pool: SqlitePool,
}

impl SqliteAuthStore {
    pub async fn new(db_path: &str) -> Result<Self> {
        if let Some(dir) = Path::new(db_path).parent() { std::fs::create_dir_all(dir)?; }
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePool::connect_with(options).await?;
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS auth_flows (
                state TEXT PRIMARY KEY,
                issuer TEXT NOT NULL,
                pds_url TEXT NOT NULL,
                code_verifier TEXT NOT NULL,
                dpop_pkcs8 BLOB NOT NULL,
                dpop_jwk TEXT NOT NULL,
                dpop_nonce TEXT,
                created_at INTEGER NOT NULL
            )"#,
        ).execute(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn insert(&self, rec: AuthFlowRecord) -> Result<()> {
        sqlx::query(
            r#"INSERT OR REPLACE INTO auth_flows (state, issuer, pds_url, code_verifier, dpop_pkcs8, dpop_jwk, dpop_nonce, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, strftime('%s','now'))"#,
        )
        .bind(&rec.state)
        .bind(&rec.issuer)
        .bind(&rec.pds_url)
        .bind(&rec.code_verifier)
        .bind(&rec.dpop_pkcs8)
        .bind(&rec.dpop_jwk)
        .bind(&rec.dpop_nonce)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get(&self, state: &str) -> Result<Option<AuthFlowRecord>> {
        let row = sqlx::query(
            r#"SELECT state, issuer, pds_url, code_verifier, dpop_pkcs8, dpop_jwk, dpop_nonce
               FROM auth_flows WHERE state = ?"#,
        )
        .bind(state)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| AuthFlowRecord {
            state: r.get::<String, _>(0),
            issuer: r.get::<String, _>(1),
            pds_url: r.get::<String, _>(2),
            code_verifier: r.get::<String, _>(3),
            dpop_pkcs8: r.get::<Vec<u8>, _>(4),
            dpop_jwk: r.get::<String, _>(5),
            dpop_nonce: r.get::<Option<String>, _>(6),
        }))
    }

    pub async fn update_nonce(&self, state: &str, nonce: Option<&str>) -> Result<()> {
        sqlx::query(
            r#"UPDATE auth_flows SET dpop_nonce = ? WHERE state = ?"#,
        )
        .bind(nonce)
        .bind(state)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, state: &str) -> Result<()> {
        sqlx::query("DELETE FROM auth_flows WHERE state = ?")
            .bind(state)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Prune flows older than `max_age_secs` seconds. Returns number of rows deleted.
    pub async fn prune_older_than(&self, max_age_secs: i64) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM auth_flows WHERE created_at < strftime('%s','now') - ?",
        )
        .bind(max_age_secs)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Run SQLite maintenance tasks: optimize and vacuum.
    pub async fn vacuum(&self) -> Result<()> {
        // PRAGMA optimize may be a no-op on some builds but is safe to execute.
        sqlx::query("PRAGMA optimize").execute(&self.pool).await?;
        sqlx::query("VACUUM").execute(&self.pool).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_insert_get_delete_prune() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("auth.db");
        let store = SqliteAuthStore::new(db_path.to_str().unwrap()).await.unwrap();

        let rec = AuthFlowRecord {
            state: "state1".into(),
            issuer: "https://bsky.social".into(),
            pds_url: "https://bsky.social".into(),
            code_verifier: "verifier".into(),
            dpop_pkcs8: vec![1,2,3],
            dpop_jwk: "{\"kty\":\"EC\"}".into(),
            dpop_nonce: None,
        };

        store.insert(rec).await.unwrap();
        let got = store.get("state1").await.unwrap().unwrap();
        assert_eq!(got.code_verifier, "verifier");

        store.update_nonce("state1", Some("abc")).await.unwrap();
        let got2 = store.get("state1").await.unwrap().unwrap();
        assert_eq!(got2.dpop_nonce.as_deref(), Some("abc"));

        // prune should not delete fresh rows
        let pruned = store.prune_older_than(1).await.unwrap();
        assert_eq!(pruned, 0);

        store.delete("state1").await.unwrap();
        assert!(store.get("state1").await.unwrap().is_none());
    }
}
