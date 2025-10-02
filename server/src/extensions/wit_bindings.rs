//! WIT bindings implementation for extension host functions
//!
//! This module provides the host-side implementation of the WIT interface
//! defined in packages/wit/extension.wit. It uses wit-bindgen to generate bindings
//! and implements the host functions that extensions can import.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wasmtime::component::*;
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::p2::{add_to_linker_sync, IoView, WasiCtx, WasiCtxBuilder, WasiView};

// Generate the host-side WIT bindings
// Note: Using sync (not async) to avoid Send+Sync issues with WASI types in Store
wasmtime::component::bindgen!({
    world: "extension",
    path: "../packages/wit",
    async: false,
});

// Import generated types from the bindgen macro
// The bindgen macro generates:
// - `Extension` struct representing the instantiated world
// - Host trait implementations for imported interfaces
// - Types for exported interfaces

// For exports (extension-api), we need the Extension struct and its associated types
// The bindgen generates these under forge::extension module
pub use self::Extension as WasmExtension;

// Import types from generated guest interface modules
use self::exports::forge::extension::extension_api::{
    Config as ExtConfig, ContextScope as ExtContextScope, GlobalContext as ExtGlobalContext,
    RepositoryContext as ExtRepositoryContext, RequestContext as ExtRequestContext,
    ResolveInfo as ExtResolveInfo, ResolveResult as ExtResolveResult,
    UserContext as ExtUserContext,
};

// For imports (host-*), we implement the Host traits
use self::forge::extension::host_database::{
    ExecInfo, ExecResult, QueryResult, QueryRow, RecordValue as WitRecordValue,
};
use self::forge::extension::host_log::LogLevel;

/// Result of a GraphQL field resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum ResolveResult {
    Success(serde_json::Value),
    Error(String),
}

/// The logical scope an extension resolver is executing within.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextScope {
    Global,
    Repository,
    User,
    RepositoryUser,
}

impl Default for ContextScope {
    fn default() -> Self {
        ContextScope::Global
    }
}

/// Repository metadata exposed to extensions when a resolver operates on a repository.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepositoryContext {
    pub id: String,
    pub slug: String,
    pub group_id: Option<String>,
    pub full_path: Option<String>,
    pub is_remote: bool,
    pub remote_url: Option<String>,
}

/// User metadata provided to extensions when a resolver executes on behalf of a user.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserContext {
    pub id: String,
    pub username: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
}

/// Global/environment metadata shared across requests.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalContext {
    pub installation_id: Option<String>,
    pub environment: Option<String>,
    #[serde(default)]
    pub feature_flags: Vec<String>,
}

/// Structured request context delivered with every extension resolver invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestContext {
    pub scope: ContextScope,
    pub repository: Option<RepositoryContext>,
    pub user: Option<UserContext>,
    pub global: Option<GlobalContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

impl Default for RequestContext {
    fn default() -> Self {
        Self {
            scope: ContextScope::Global,
            repository: None,
            user: None,
            global: None,
            extra: None,
        }
    }
}

/// Information needed to resolve a GraphQL field
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ResolveInfo {
    pub field_name: String,
    pub parent_type: String,
    pub arguments: serde_json::Value,
    pub context: RequestContext,
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

/// Host state that provides functions to WASM extensions
pub struct ExtensionHost {
    pub name: String,
    pub db_pool: Arc<std::sync::Mutex<Option<SqlitePool>>>,
    #[allow(dead_code)]
    pub extension_dir: PathBuf,
}

impl ExtensionHost {
    pub fn new(name: String, extension_dir: PathBuf) -> Self {
        Self {
            name,
            db_pool: Arc::new(std::sync::Mutex::new(None)),
            extension_dir,
        }
    }

    /// Initialize database connection (no-op since pool is pre-initialized)
    pub fn init_database(&self, _db_path: &str) -> Result<()> {
        // Database is already initialized before WASM instantiation
        // This is just here to satisfy the host interface contract
        Ok(())
    }

    /// Get database pool
    fn get_pool(&self) -> Result<SqlitePool> {
        let pool_guard = self
            .db_pool
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock database pool: {}", e))?;
        pool_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not initialized"))
            .cloned()
    }
}

/// State container that combines ExtensionHost with WASI state
pub struct ExtensionState {
    pub host: ExtensionHost,
    pub wasi: WasiCtx,
    pub table: ResourceTable,
}

impl ExtensionState {
    pub fn new(host: ExtensionHost, wasi: WasiCtx) -> Self {
        Self {
            host,
            wasi,
            table: ResourceTable::new(),
        }
    }
}

// Implement IoView for resource table access
impl IoView for ExtensionState {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

// Implement WasiView so Wasmtime can access WASI context
impl WasiView for ExtensionState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi
    }
}

// Implement the host-log interface
impl self::forge::extension::host_log::Host for ExtensionState {
    fn log(&mut self, level: LogLevel, message: String) {
        match level {
            LogLevel::Trace => tracing::trace!("[{}] {}", self.host.name, message),
            LogLevel::Debug => tracing::debug!("[{}] {}", self.host.name, message),
            LogLevel::Info => tracing::info!("[{}] {}", self.host.name, message),
            LogLevel::Warn => tracing::warn!("[{}] {}", self.host.name, message),
            LogLevel::Error => tracing::error!("[{}] {}", self.host.name, message),
        }
    }
}

// Convert between serde_json and WIT RecordValue
// NOTE: These helpers are reserved for future use when we need bidirectional
// conversion between JSON and WIT values for complex extension data types.
// Currently unused but may be needed for advanced extension features.
#[allow(dead_code)]
fn json_to_wit_value(value: &serde_json::Value) -> WitRecordValue {
    match value {
        serde_json::Value::Null => WitRecordValue::Null,
        serde_json::Value::Bool(b) => WitRecordValue::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                WitRecordValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                WitRecordValue::Float(f)
            } else {
                WitRecordValue::Text(n.to_string())
            }
        }
        serde_json::Value::String(s) => WitRecordValue::Text(s.clone()),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            WitRecordValue::Text(value.to_string())
        }
    }
}

#[allow(dead_code)]
fn wit_value_to_json(value: &WitRecordValue) -> serde_json::Value {
    match value {
        WitRecordValue::Null => serde_json::Value::Null,
        WitRecordValue::Boolean(b) => serde_json::Value::Bool(*b),
        WitRecordValue::Integer(i) => serde_json::json!(i),
        WitRecordValue::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        WitRecordValue::Text(s) => serde_json::Value::String(s.clone()),
        WitRecordValue::Blob(b) => {
            // Convert blob to base64 string
            serde_json::Value::String(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                b,
            ))
        }
    }
}

fn to_wit_request_context(context: &RequestContext) -> Result<ExtRequestContext> {
    let repository = context
        .repository
        .as_ref()
        .map(|repo| to_wit_repository_context(repo));
    let user = context.user.as_ref().map(|user| to_wit_user_context(user));
    let global = context
        .global
        .as_ref()
        .map(|global| to_wit_global_context(global));
    let extra_json = match context.extra.as_ref() {
        Some(extra) => Some(serde_json::to_string(extra)?),
        None => None,
    };

    Ok(ExtRequestContext {
        scope: to_wit_context_scope(context.scope),
        repository,
        user,
        global,
        extra_json,
    })
}

fn to_wit_context_scope(scope: ContextScope) -> ExtContextScope {
    match scope {
        ContextScope::Global => ExtContextScope::Global,
        ContextScope::Repository => ExtContextScope::Repository,
        ContextScope::User => ExtContextScope::User,
        ContextScope::RepositoryUser => ExtContextScope::RepositoryUser,
    }
}

fn to_wit_repository_context(context: &RepositoryContext) -> ExtRepositoryContext {
    ExtRepositoryContext {
        id: context.id.clone(),
        slug: context.slug.clone(),
        group_id: context.group_id.clone(),
        full_path: context.full_path.clone(),
        is_remote: context.is_remote,
        remote_url: context.remote_url.clone(),
    }
}

fn to_wit_user_context(context: &UserContext) -> ExtUserContext {
    ExtUserContext {
        id: context.id.clone(),
        username: context.username.clone(),
        display_name: context.display_name.clone(),
        email: context.email.clone(),
    }
}

fn to_wit_global_context(context: &GlobalContext) -> ExtGlobalContext {
    ExtGlobalContext {
        installation_id: context.installation_id.clone(),
        environment: context.environment.clone(),
        feature_flags: context.feature_flags.clone(),
    }
}

// Implement the host-database interface
impl self::forge::extension::host_database::Host for ExtensionState {
    fn query(&mut self, sql: String, params: Vec<WitRecordValue>) -> QueryResult {
        // NOTE: We use block_on here because WASM bindings are synchronous (async: false)
        // This is a known trade-off: sync WASM bindings avoid Send/Sync issues with WASI types,
        // but require blocking on async database operations.
        // The impact is minimal since each extension runs in isolation.

        let pool = match self.host.get_pool() {
            Ok(p) => p,
            Err(e) => return QueryResult::Error(e.to_string()),
        };

        let mut query = sqlx::query(&sql);

        // Bind parameters
        for param in &params {
            query = match param {
                WitRecordValue::Null => query.bind(None::<String>),
                WitRecordValue::Boolean(b) => query.bind(b),
                WitRecordValue::Integer(i) => query.bind(i),
                WitRecordValue::Float(f) => query.bind(f),
                WitRecordValue::Text(s) => query.bind(s),
                WitRecordValue::Blob(b) => query.bind(b),
            };
        }

        // Execute query with proper error context
        let rows = match tokio::runtime::Handle::try_current() {
            Ok(handle) => match handle.block_on(query.fetch_all(&pool)) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("Database query failed: {}", e);
                    return QueryResult::Error(format!("Query failed: {}", e));
                }
            },
            Err(_) => {
                return QueryResult::Error("No tokio runtime available".to_string());
            }
        };

        // Convert rows to WIT QueryRow structures
        let mut result_rows = Vec::new();
        for row in rows {
            let mut row_values = Vec::new();
            for (i, _column) in row.columns().iter().enumerate() {
                let value: WitRecordValue = if let Ok(v) = row.try_get::<Option<String>, _>(i) {
                    v.map(WitRecordValue::Text).unwrap_or(WitRecordValue::Null)
                } else if let Ok(v) = row.try_get::<Option<i64>, _>(i) {
                    v.map(WitRecordValue::Integer)
                        .unwrap_or(WitRecordValue::Null)
                } else if let Ok(v) = row.try_get::<Option<f64>, _>(i) {
                    v.map(WitRecordValue::Float).unwrap_or(WitRecordValue::Null)
                } else if let Ok(v) = row.try_get::<Option<bool>, _>(i) {
                    v.map(WitRecordValue::Boolean)
                        .unwrap_or(WitRecordValue::Null)
                } else if let Ok(v) = row.try_get::<Option<Vec<u8>>, _>(i) {
                    v.map(WitRecordValue::Blob).unwrap_or(WitRecordValue::Null)
                } else {
                    WitRecordValue::Null
                };
                row_values.push(value);
            }
            result_rows.push(QueryRow { values: row_values });
        }

        QueryResult::Success(result_rows)
    }

    fn execute(&mut self, sql: String, params: Vec<WitRecordValue>) -> ExecResult {
        let pool = match self.host.get_pool() {
            Ok(p) => p,
            Err(e) => return ExecResult::Error(e.to_string()),
        };

        let mut query = sqlx::query(&sql);

        // Bind parameters (same as query)
        for param in &params {
            query = match param {
                WitRecordValue::Null => query.bind(None::<String>),
                WitRecordValue::Boolean(b) => query.bind(b),
                WitRecordValue::Integer(i) => query.bind(i),
                WitRecordValue::Float(f) => query.bind(f),
                WitRecordValue::Text(s) => query.bind(s),
                WitRecordValue::Blob(b) => query.bind(b),
            };
        }

        let result = match tokio::runtime::Handle::try_current() {
            Ok(handle) => match handle.block_on(query.execute(&pool)) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("Database execute failed: {}", e);
                    return ExecResult::Error(format!("Execute failed: {}", e));
                }
            },
            Err(_) => {
                return ExecResult::Error("No tokio runtime available".to_string());
            }
        };

        ExecResult::Success(ExecInfo {
            rows_affected: result.rows_affected(),
            last_insert_id: Some(result.last_insert_rowid() as u64),
        })
    }

    fn migrate(&mut self, migrations: String) -> Result<(), String> {
        let pool = match self.host.get_pool() {
            Ok(p) => p,
            Err(e) => return Err(e.to_string()),
        };

        let handle = tokio::runtime::Handle::try_current()
            .map_err(|_| "No tokio runtime available".to_string())?;

        // Split migrations by semicolon and execute each
        for migration in migrations.split(';') {
            let trimmed = migration.trim();
            if !trimmed.is_empty()
                && let Err(e) = handle.block_on(sqlx::query(trimmed).execute(&pool))
            {
                tracing::error!("Migration failed: {}", e);
                return Err(format!("Failed to run migration: {}", e));
            }
        }

        Ok(())
    }
}

/// Component-based extension instance
pub struct ComponentExtension {
    store: Store<ExtensionState>,
    bindings: WasmExtension,
}

impl ComponentExtension {
    /// Load a WASM component
    pub fn load(
        wasm_path: &Path,
        extension_dir: &Path,
        name: String,
        db_pool: SqlitePool,
    ) -> Result<Self> {
        // Create engine with component model support
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(false); // Using sync bindings

        let engine = Engine::new(&config)?;

        // Create WASI context - minimal configuration
        let wasi = WasiCtxBuilder::new().build();

        // Create host with pre-initialized database pool
        let host = ExtensionHost::new(name, extension_dir.to_path_buf());
        // Store the pool
        {
            let mut pool_guard = host
                .db_pool
                .lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock database pool: {}", e))?;
            *pool_guard = Some(db_pool);
        }

        // Create state with host and WASI
        let state = ExtensionState::new(host, wasi);

        // Create store
        let mut store = Store::new(&engine, state);

        // Load component
        let component_bytes = std::fs::read(wasm_path)?;
        let component = Component::from_binary(&engine, &component_bytes)?;

        // Create linker and add WASI
        let mut linker = Linker::new(&engine);
        add_to_linker_sync(&mut linker)?;

        // Add host interfaces using generated bindings
        WasmExtension::add_to_linker(&mut linker, |state: &mut ExtensionState| state)?;

        // Instantiate
        let bindings = WasmExtension::instantiate(&mut store, &component, &linker)?;

        Ok(Self { store, bindings })
    }

    /// Initialize the extension
    pub fn init(&mut self, config: ExtensionConfig) -> Result<()> {
        // Initialize database connection
        self.store
            .data()
            .host
            .init_database(&config.database_path)?;

        // Convert config to WIT format
        let wit_config = ExtConfig {
            name: config.name,
            version: config.version,
            database_path: config.database_path,
            custom_config: config.custom_config,
        };

        // Call the extension's init function
        self.bindings
            .forge_extension_extension_api()
            .call_init(&mut self.store, &wit_config)?
            .map_err(|e| anyhow::anyhow!("Extension init failed: {}", e))?;

        Ok(())
    }

    /// Get extension info
    pub fn get_info(&mut self) -> Result<ExtensionInfo> {
        let info = self
            .bindings
            .forge_extension_extension_api()
            .call_get_info(&mut self.store)?;

        Ok(ExtensionInfo {
            name: info.name,
            version: info.version,
            capabilities: info.capabilities,
        })
    }

    /// Get GraphQL schema
    pub fn get_schema(&mut self) -> Result<String> {
        let schema = self
            .bindings
            .forge_extension_extension_api()
            .call_get_schema(&mut self.store)?;

        Ok(schema)
    }

    /// Resolve a GraphQL field
    #[allow(dead_code)]
    pub fn resolve_field(&mut self, info: ResolveInfo) -> Result<ResolveResult> {
        let ResolveInfo {
            field_name,
            parent_type,
            arguments,
            context,
            parent,
        } = info;

        let wit_info = ExtResolveInfo {
            field_name,
            parent_type,
            arguments: serde_json::to_string(&arguments)?,
            context: to_wit_request_context(&context)?,
            parent: parent.map(|p| serde_json::to_string(&p)).transpose()?,
        };

        let result = self
            .bindings
            .forge_extension_extension_api()
            .call_resolve_field(&mut self.store, &wit_info)?;

        match result {
            ExtResolveResult::Success(json) => {
                let value: serde_json::Value = serde_json::from_str(&json)?;
                Ok(ResolveResult::Success(value))
            }
            ExtResolveResult::Error(err) => Ok(ResolveResult::Error(err)),
        }
    }

    /// Shutdown the extension
    #[allow(dead_code)]
    pub fn shutdown(&mut self) -> Result<()> {
        self.bindings
            .forge_extension_extension_api()
            .call_shutdown(&mut self.store)?;

        Ok(())
    }
}
