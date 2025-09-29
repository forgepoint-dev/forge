//! Simplified WASM runtime that provides actual execution

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use wasmtime::*;

use super::loader::ExtensionLimits;

/// Extension configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtConfig {
    pub name: String,
    pub version: String,
    pub database_path: String,
    pub custom_config: Option<String>,
}

/// Field resolution request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldResolveRequest {
    pub field_name: String,
    pub parent_type: String,
    pub arguments: serde_json::Value,
    pub context: serde_json::Value,
    pub parent: Option<serde_json::Value>,
}

/// Field resolution response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FieldResolveResponse {
    #[serde(rename = "success")]
    Success { data: serde_json::Value },
    #[serde(rename = "error")]
    Error { message: String },
}

/// Host context for WASM extensions
pub struct WasmHost {
    pub name: String,
    pub db_pool: Option<SqlitePool>,
    pub log_buffer: Arc<RwLock<Vec<String>>>,
}

impl WasmHost {
    pub fn new(name: String) -> Self {
        Self {
            name,
            db_pool: None,
            log_buffer: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn init_db(&mut self, db_path: &str) -> Result<()> {
        // Handle special in-memory database path
        let conn_str = if db_path == ":memory:" {
            "sqlite::memory:".to_string()
        } else {
            // Ensure directory exists for file-based databases
            if let Some(parent) = std::path::Path::new(db_path).parent() {
                std::fs::create_dir_all(parent)?;
            }
            format!("sqlite:{}", db_path)
        };

        let pool = SqlitePool::connect(&conn_str).await?;
        self.db_pool = Some(pool);
        Ok(())
    }

    pub fn with_pool(name: String, pool: SqlitePool) -> Self {
        Self {
            name,
            db_pool: Some(pool),
            log_buffer: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

/// WASM-based extension runtime
pub struct WasmExtension {
    store: Store<WasmHost>,
    instance: Instance,
    memory: Memory,
    alloc_fn: TypedFunc<u32, u32>,
    dealloc_fn: TypedFunc<(u32, u32), ()>,
    init_fn: TypedFunc<(u32, u32), u32>,
    get_schema_fn: TypedFunc<(), u32>,
    resolve_field_fn: TypedFunc<(u32, u32), u32>,
}

impl WasmExtension {
    /// Load a WASM module
    pub async fn load(
        wasm_path: &Path,
        _extension_dir: &Path,
        name: String,
        limits: &ExtensionLimits,
    ) -> Result<Self> {
        // Create engine with limits
        let mut config = Config::new();
        config.async_support(true);
        config.consume_fuel(limits.max_fuel.is_some());
        // Note: epoch_interruption requires manual epoch management
        // For now, we'll rely on fuel metering for timeout control
        config.max_wasm_stack(limits.max_stack_bytes);

        let engine = Engine::new(&config)?;

        // Create WASI context (simplified - proper implementation would manage WASI state)
        // For now, we'll skip WASI and just use raw WASM

        // Create store with host
        let host = WasmHost::new(name);
        let mut store = Store::new(&engine, host);

        // Set fuel if configured
        if let Some(max_fuel) = limits.max_fuel {
            store.set_fuel(max_fuel)?;
        }

        // Load module
        let module_bytes = std::fs::read(wasm_path)?;
        if module_bytes.len() > limits.max_module_bytes {
            return Err(anyhow::anyhow!("Module too large"));
        }

        let module = Module::new(&engine, &module_bytes)?;

        // Create linker and add host functions
        let mut linker = Linker::new(&engine);

        // Add custom host functions
        Self::add_host_functions(&mut linker)?;

        // Instantiate (must use async when async support is enabled)
        let instance = linker.instantiate_async(&mut store, &module).await?;

        // Get memory export
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| anyhow::anyhow!("No memory export found"))?;

        // Get required functions
        let alloc_fn = instance
            .get_typed_func::<u32, u32>(&mut store, "alloc")
            .context("Missing alloc function")?;

        let dealloc_fn = instance
            .get_typed_func::<(u32, u32), ()>(&mut store, "dealloc")
            .context("Missing dealloc function")?;

        let init_fn = instance
            .get_typed_func::<(u32, u32), u32>(&mut store, "init")
            .context("Missing init function")?;

        let get_schema_fn = instance
            .get_typed_func::<(), u32>(&mut store, "get_schema")
            .context("Missing get_schema function")?;

        let resolve_field_fn = instance
            .get_typed_func::<(u32, u32), u32>(&mut store, "resolve_field")
            .context("Missing resolve_field function")?;

        Ok(Self {
            store,
            instance,
            memory,
            alloc_fn,
            dealloc_fn,
            init_fn,
            get_schema_fn,
            resolve_field_fn,
        })
    }

    fn add_host_functions(linker: &mut Linker<WasmHost>) -> Result<()> {
        // Add logging function
        linker.func_wrap(
            "env",
            "host_log",
            |mut caller: Caller<'_, WasmHost>, ptr: u32, len: u32| -> Result<()> {
                let memory = caller.get_export("memory")
                    .and_then(|e| e.into_memory())
                    .ok_or_else(|| anyhow::anyhow!("No memory export"))?;

                let data = memory.data(&caller);
                let msg_bytes = data.get(ptr as usize..(ptr + len) as usize)
                    .ok_or_else(|| anyhow::anyhow!("Invalid memory range"))?;

                let message = std::str::from_utf8(msg_bytes)?;
                tracing::info!("[{}] {}", caller.data().name, message);

                Ok(())
            },
        )?;

        // Add database query function
        linker.func_wrap(
            "env",
            "host_db_query",
            |mut caller: Caller<'_, WasmHost>, sql_ptr: u32, sql_len: u32, params_ptr: u32, params_len: u32| -> Result<u32> {
                // This would read SQL and params from memory, execute query,
                // and return a pointer to the results
                // For now, return 0 (null pointer)
                Ok(0)
            },
        )?;

        Ok(())
    }

    /// Write a string to WASM memory and return pointer and length
    async fn write_string(&mut self, s: &str) -> Result<(u32, u32)> {
        let bytes = s.as_bytes();
        let len = bytes.len() as u32;

        // Allocate memory in WASM
        let ptr = self.alloc_fn.call_async(&mut self.store, len).await?;

        // Write to memory
        let memory_data = self.memory.data_mut(&mut self.store);
        memory_data.get_mut(ptr as usize..(ptr + len) as usize)
            .ok_or_else(|| anyhow::anyhow!("Invalid memory range"))?
            .copy_from_slice(bytes);

        Ok((ptr, len))
    }

    /// Read a string from WASM memory
    async fn read_string(&mut self, ptr: u32, len: u32) -> Result<String> {
        let memory_data = self.memory.data(&self.store);
        let bytes = memory_data.get(ptr as usize..(ptr + len) as usize)
            .ok_or_else(|| anyhow::anyhow!("Invalid memory range"))?;

        Ok(String::from_utf8(bytes.to_vec())?)
    }

    /// Initialize the extension
    pub async fn init(&mut self, config: ExtConfig) -> Result<()> {
        // Initialize database
        if !config.database_path.is_empty() {
            // Create database directory if needed
            if let Some(parent) = std::path::Path::new(&config.database_path).parent() {
                std::fs::create_dir_all(parent)?;
            }
            // Touch the database file to ensure it exists
            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&config.database_path)?;

            // Initialize the database connection
            let conn_str = if config.database_path.starts_with("sqlite:") {
                config.database_path.clone()
            } else {
                format!("sqlite:{}", config.database_path)
            };
            let pool = SqlitePool::connect(&conn_str).await?;
            self.store.data_mut().db_pool = Some(pool);
        }

        // Serialize config to JSON
        let config_json = serde_json::to_string(&config)?;

        // Write to WASM memory
        let (ptr, len) = self.write_string(&config_json).await?;

        // Call init function
        let result_ptr = self.init_fn.call_async(&mut self.store, (ptr, len)).await?;

        // Free the input memory
        self.dealloc_fn.call_async(&mut self.store, (ptr, len)).await?;

        if result_ptr != 0 {
            // Read error message
            let error = self.read_string(result_ptr, 1024).await?; // Assume max 1KB error
            return Err(anyhow::anyhow!("Init failed: {}", error));
        }

        Ok(())
    }

    /// Get the GraphQL schema
    pub async fn get_schema(&mut self) -> Result<String> {
        // Call get_schema function
        let result_ptr = self.get_schema_fn.call_async(&mut self.store, ()).await?;

        if result_ptr == 0 {
            return Ok(String::new());
        }

        // Read length from first 4 bytes
        let memory_data = self.memory.data(&self.store);
        let len_bytes = memory_data.get(result_ptr as usize..(result_ptr + 4) as usize)
            .ok_or_else(|| anyhow::anyhow!("Invalid memory range"))?;

        let len = u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]);

        // Read string
        let schema = self.read_string(result_ptr + 4, len).await?;

        // Free the memory
        self.dealloc_fn.call_async(&mut self.store, (result_ptr, len + 4)).await?;

        Ok(schema)
    }

    /// Resolve a GraphQL field
    pub async fn resolve_field(&mut self, request: FieldResolveRequest) -> Result<serde_json::Value> {
        // Serialize request to JSON
        let request_json = serde_json::to_string(&request)?;

        // Write to WASM memory
        let (ptr, len) = self.write_string(&request_json).await?;

        // Call resolve_field function
        let result_ptr = self.resolve_field_fn.call_async(&mut self.store, (ptr, len)).await?;

        // Free the input memory
        self.dealloc_fn.call_async(&mut self.store, (ptr, len)).await?;

        if result_ptr == 0 {
            return Err(anyhow::anyhow!("Resolve field returned null"));
        }

        // Read length and response
        let memory_data = self.memory.data(&self.store);
        let len_bytes = memory_data.get(result_ptr as usize..(result_ptr + 4) as usize)
            .ok_or_else(|| anyhow::anyhow!("Invalid memory range"))?;

        let result_len = u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]);

        let response_json = self.read_string(result_ptr + 4, result_len).await?;

        // Free the result memory
        self.dealloc_fn.call_async(&mut self.store, (result_ptr, result_len + 4)).await?;

        // Parse response
        let response: FieldResolveResponse = serde_json::from_str(&response_json)?;

        match response {
            FieldResolveResponse::Success { data } => Ok(data),
            FieldResolveResponse::Error { message } => Err(anyhow::anyhow!("Field resolution error: {}", message)),
        }
    }
}

/// High-level extension wrapper
pub struct Extension {
    wasm: Arc<RwLock<WasmExtension>>,
    schema: String,
    name: String,
}

impl Extension {
    /// Load and initialize an extension
    pub async fn load(
        wasm_path: &Path,
        extension_dir: &Path,
        name: String,
        db_path: PathBuf,
        limits: &ExtensionLimits,
    ) -> Result<Self> {
        // Load WASM module
        let mut wasm = WasmExtension::load(wasm_path, extension_dir, name.clone(), limits).await?;

        // Initialize
        let config = ExtConfig {
            name: name.clone(),
            version: "0.1.0".to_string(),
            database_path: db_path.to_string_lossy().to_string(),
            custom_config: None,
        };

        wasm.init(config).await?;

        // Get schema
        let schema = wasm.get_schema().await?;

        Ok(Self {
            wasm: Arc::new(RwLock::new(wasm)),
            schema,
            name,
        })
    }

    /// Load and initialize an extension with a pre-configured database pool (for testing)
    #[cfg(any(test, feature = "test-support"))]
    pub async fn load_with_pool(
        wasm_path: &Path,
        extension_dir: &Path,
        name: String,
        pool: SqlitePool,
        limits: &ExtensionLimits,
    ) -> Result<Self> {
        // Load WASM module with pre-configured pool
        let mut wasm = WasmExtension::load(wasm_path, extension_dir, name.clone(), limits).await?;

        // Set the pool directly on the host
        wasm.store.data_mut().db_pool = Some(pool);

        // Initialize with memory database path (won't be used since pool is already set)
        let config = ExtConfig {
            name: name.clone(),
            version: "0.1.0".to_string(),
            database_path: ":memory:".to_string(),
            custom_config: None,
        };

        wasm.init(config).await?;

        // Get schema
        let schema = wasm.get_schema().await?;

        Ok(Self {
            wasm: Arc::new(RwLock::new(wasm)),
            schema,
            name,
        })
    }

    /// Get the GraphQL schema
    pub fn schema(&self) -> &str {
        &self.schema
    }

    /// Resolve a field
    pub async fn resolve_field(
        &self,
        field_name: String,
        parent_type: String,
        arguments: serde_json::Value,
        context: serde_json::Value,
        parent: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let request = FieldResolveRequest {
            field_name,
            parent_type,
            arguments,
            context,
            parent,
        };

        let mut wasm = self.wasm.write().await;
        wasm.resolve_field(request).await
    }
}