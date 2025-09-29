//! WIT bindings implementation for extension host functions

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{Column, Row, SqlitePool};
use std::path::PathBuf;
use wasmtime::component::*;
use wasmtime::{Caller, Config, Engine, Store};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder};

/// Result of a GraphQL field resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResolveResult {
    Success(serde_json::Value),
    Error(String),
}

/// Information needed to resolve a GraphQL field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveInfo {
    pub field_name: String,
    pub parent_type: String,
    pub arguments: serde_json::Value,
    pub context: serde_json::Value,
    pub parent: Option<serde_json::Value>,
}

/// Configuration for an extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionConfig {
    pub name: String,
    pub version: String,
    pub database_path: String,
    pub custom_config: Option<String>,
}

/// Extension information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionInfo {
    pub name: String,
    pub version: String,
    pub capabilities: Vec<String>,
}

/// Host implementation that provides functions to WASM extensions
pub struct ExtensionHost {
    name: String,
    db_pool: Option<SqlitePool>,
    extension_dir: PathBuf,
}

impl ExtensionHost {
    pub fn new(name: String, extension_dir: PathBuf) -> Self {
        Self {
            name,
            db_pool: None,
            extension_dir,
        }
    }

    /// Initialize database connection
    pub async fn init_database(&mut self, db_path: &str) -> Result<()> {
        let pool = SqlitePool::connect(db_path)
            .await
            .context("Failed to connect to extension database")?;
        self.db_pool = Some(pool);
        Ok(())
    }

    /// Log a message from the extension
    pub fn log(&self, level: &str, message: &str) {
        match level {
            "trace" => tracing::trace!("[{}] {}", self.name, message),
            "debug" => tracing::debug!("[{}] {}", self.name, message),
            "info" => tracing::info!("[{}] {}", self.name, message),
            "warn" => tracing::warn!("[{}] {}", self.name, message),
            "error" => tracing::error!("[{}] {}", self.name, message),
            _ => tracing::info!("[{}] {}", self.name, message),
        }
    }

    /// Execute a database query
    pub async fn query(&self, sql: &str, params: Vec<serde_json::Value>) -> Result<Vec<serde_json::Value>> {
        let pool = self.db_pool.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not initialized"))?;

        let mut query = sqlx::query(sql);

        // Bind parameters
        for param in params {
            query = match param {
                serde_json::Value::Null => query.bind(None::<String>),
                serde_json::Value::Bool(b) => query.bind(b),
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        query.bind(i)
                    } else if let Some(f) = n.as_f64() {
                        query.bind(f)
                    } else {
                        query.bind(n.to_string())
                    }
                },
                serde_json::Value::String(s) => query.bind(s),
                _ => query.bind(param.to_string()),
            };
        }

        let rows = query.fetch_all(pool).await?;

        // Convert rows to JSON
        let mut results = Vec::new();
        for row in rows {
            let mut obj = serde_json::Map::new();
            for (i, column) in row.columns().iter().enumerate() {
                let name = column.name();
                let value: serde_json::Value = if let Ok(v) = row.try_get::<Option<String>, _>(i) {
                    v.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null)
                } else if let Ok(v) = row.try_get::<Option<i64>, _>(i) {
                    v.map(|n| serde_json::Value::Number(n.into())).unwrap_or(serde_json::Value::Null)
                } else if let Ok(v) = row.try_get::<Option<f64>, _>(i) {
                    v.and_then(|n| serde_json::Number::from_f64(n))
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::Null)
                } else if let Ok(v) = row.try_get::<Option<bool>, _>(i) {
                    v.map(serde_json::Value::Bool).unwrap_or(serde_json::Value::Null)
                } else {
                    serde_json::Value::Null
                };
                obj.insert(name.to_string(), value);
            }
            results.push(serde_json::Value::Object(obj));
        }

        Ok(results)
    }

    /// Execute a database statement
    pub async fn execute(&self, sql: &str, params: Vec<serde_json::Value>) -> Result<u64> {
        let pool = self.db_pool.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not initialized"))?;

        let mut query = sqlx::query(sql);

        // Bind parameters (same as above)
        for param in params {
            query = match param {
                serde_json::Value::Null => query.bind(None::<String>),
                serde_json::Value::Bool(b) => query.bind(b),
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        query.bind(i)
                    } else if let Some(f) = n.as_f64() {
                        query.bind(f)
                    } else {
                        query.bind(n.to_string())
                    }
                },
                serde_json::Value::String(s) => query.bind(s),
                _ => query.bind(param.to_string()),
            };
        }

        let result = query.execute(pool).await?;
        Ok(result.rows_affected())
    }

    /// Run database migrations
    pub async fn migrate(&self, migrations: &str) -> Result<()> {
        let pool = self.db_pool.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not initialized"))?;

        // Split migrations by semicolon and execute each
        for migration in migrations.split(';') {
            let trimmed = migration.trim();
            if !trimmed.is_empty() {
                sqlx::query(trimmed)
                    .execute(pool)
                    .await
                    .with_context(|| format!("Failed to run migration: {}", trimmed))?;
            }
        }

        Ok(())
    }
}

/// Component-based extension instance
pub struct ComponentExtension {
    store: Store<ExtensionHost>,
    instance: Instance,
    component: Component,
    engine: Engine,
}

impl ComponentExtension {
    /// Load a WASM component
    pub async fn load(
        wasm_path: &PathBuf,
        extension_dir: &PathBuf,
        name: String,
    ) -> Result<Self> {
        // Create engine with component model support
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);

        let engine = Engine::new(&config)?;

        // Create WASI context
        let wasi = WasiCtxBuilder::new()
            .inherit_stdio()
            .preopened_dir(
                extension_dir,
                "/",
                DirPerms::all(),
                FilePerms::all(),
            )?
            .build_p1();

        // Create host
        let host = ExtensionHost::new(name, extension_dir.clone());

        // Create store
        let mut store = Store::new(&engine, host);

        // Load component
        let component_bytes = std::fs::read(wasm_path)?;
        let component = Component::from_binary(&engine, &component_bytes)?;

        // Create linker and add WASI
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |state: &mut ExtensionHost| {
            // This would need proper WASI state management
            unimplemented!("WASI state management")
        })?;

        // Add host functions
        Self::add_host_functions(&mut linker)?;

        // Instantiate
        let instance = linker.instantiate(&mut store, &component)?;

        Ok(Self {
            store,
            instance,
            component,
            engine,
        })
    }

    fn add_host_functions(linker: &mut Linker<ExtensionHost>) -> Result<()> {
        // Add logging
        linker.func_wrap(
            "host-log",
            "log",
            |mut caller: Caller<'_, ExtensionHost>, level: String, message: String| {
                caller.data().log(&level, &message);
            },
        )?;

        // Add database query
        linker.func_wrap_async(
            "host-database",
            "query",
            |mut caller: Caller<'_, ExtensionHost>, sql: String, params: String| {
                Box::new(async move {
                    let params: Vec<serde_json::Value> = serde_json::from_str(&params)
                        .map_err(|e| anyhow::anyhow!("Invalid params: {}", e))?;

                    let results = caller.data().query(&sql, params).await?;

                    Ok(serde_json::to_string(&results)?)
                })
            },
        )?;

        // Add database execute
        linker.func_wrap_async(
            "host-database",
            "execute",
            |mut caller: Caller<'_, ExtensionHost>, sql: String, params: String| {
                Box::new(async move {
                    let params: Vec<serde_json::Value> = serde_json::from_str(&params)
                        .map_err(|e| anyhow::anyhow!("Invalid params: {}", e))?;

                    let rows = caller.data().execute(&sql, params).await?;

                    Ok(rows.to_string())
                })
            },
        )?;

        // Add migrate
        linker.func_wrap_async(
            "host-database",
            "migrate",
            |mut caller: Caller<'_, ExtensionHost>, migrations: String| {
                Box::new(async move {
                    caller.data().migrate(&migrations).await?;
                    Ok(())
                })
            },
        )?;

        Ok(())
    }

    /// Initialize the extension
    pub async fn init(&mut self, config: ExtensionConfig) -> Result<()> {
        // Initialize database
        self.store.data_mut().init_database(&config.database_path).await?;

        // Call the extension's init function
        let init = self.instance
            .get_typed_func::<(String,), (), _>(&mut self.store, "init")?;

        let config_json = serde_json::to_string(&config)?;
        init.call(&mut self.store, (config_json,))?;

        Ok(())
    }

    /// Get extension info
    pub async fn get_info(&mut self) -> Result<ExtensionInfo> {
        let get_info = self.instance
            .get_typed_func::<(), String, _>(&mut self.store, "get-info")?;

        let info_json = get_info.call(&mut self.store, ())?;
        let info: ExtensionInfo = serde_json::from_str(&info_json)?;

        Ok(info)
    }

    /// Get GraphQL schema
    pub async fn get_schema(&mut self) -> Result<String> {
        let get_schema = self.instance
            .get_typed_func::<(), String, _>(&mut self.store, "get-schema")?;

        let schema = get_schema.call(&mut self.store, ())?;

        Ok(schema)
    }

    /// Resolve a GraphQL field
    pub async fn resolve_field(&mut self, info: ResolveInfo) -> Result<ResolveResult> {
        let resolve = self.instance
            .get_typed_func::<(String,), String, _>(&mut self.store, "resolve-field")?;

        let info_json = serde_json::to_string(&info)?;
        let result_json = resolve.call(&mut self.store, (info_json,))?;

        // Parse result
        let result: ResolveResult = serde_json::from_str(&result_json)?;

        Ok(result)
    }

    /// Shutdown the extension
    pub async fn shutdown(&mut self) -> Result<()> {
        let shutdown = self.instance
            .get_typed_func::<(), (), _>(&mut self.store, "shutdown")?;

        shutdown.call(&mut self.store, ())?;

        Ok(())
    }
}