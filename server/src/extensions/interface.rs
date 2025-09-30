//! WIT bindings and communication with WASM extensions

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::time::timeout;
use wasmtime::{Engine, Instance, Store};
use wasmtime_wasi::preview1::WasiP1Ctx;

use super::loader::ExtensionLimits;
use super::schema::SchemaFragment;

/// Configuration passed to extensions during initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionConfig {
    pub name: String,
    pub db_path: String,
    pub config: Option<String>,
    pub api_version: String,
    pub capabilities: Vec<String>,
}

/// API information returned by extension
#[derive(Debug, Deserialize)]
pub struct ApiInfo {
    pub version: String,
    pub supported_capabilities: Vec<String>,
}

/// Metrics for extension operations
#[derive(Debug)]
#[allow(dead_code)] // Will be used for monitoring extension performance
pub struct ExtensionMetrics {
    pub init_calls: AtomicU64,
    pub schema_calls: AtomicU64,
    pub migrate_calls: AtomicU64,
    pub resolve_calls: AtomicU64,
    pub total_errors: AtomicU64,
    pub total_timeouts: AtomicU64,
}

impl ExtensionMetrics {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for ExtensionMetrics {
    fn default() -> Self {
        Self {
            init_calls: AtomicU64::new(0),
            schema_calls: AtomicU64::new(0),
            migrate_calls: AtomicU64::new(0),
            resolve_calls: AtomicU64::new(0),
            total_errors: AtomicU64::new(0),
            total_timeouts: AtomicU64::new(0),
        }
    }
}

/// Wrapper for a WASM extension instance with safety controls
#[allow(dead_code)] // Fields will be used for WASM execution
pub struct ExtensionInstance {
    store: Store<WasiP1Ctx>,
    instance: Instance,
    engine: Engine,
    metrics: Arc<ExtensionMetrics>,
    name: String,
    limits: ExtensionLimits,
    concurrent_ops: Arc<AtomicU64>,
}

#[allow(dead_code)] // Methods will be used for extension execution
impl ExtensionInstance {
    pub fn new(store: Store<WasiP1Ctx>, instance: Instance, engine: Engine, limits: ExtensionLimits) -> Self {
        Self {
            store,
            instance,
            engine,
            metrics: Arc::new(ExtensionMetrics::new()),
            name: "unknown".to_string(),
            limits,
            concurrent_ops: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn metrics(&self) -> Arc<ExtensionMetrics> {
        self.metrics.clone()
    }

    /// Get API information from extension with version checking
    pub async fn get_api_info(&mut self) -> Result<ApiInfo> {
        // Placeholder - in real implementation, this would call the WASM function
        Ok(ApiInfo {
            version: "0.1.0".to_string(),
            supported_capabilities: vec!["basic".to_string()],
        })
    }

    /// Initialize the extension with configuration and capability checking
    pub async fn init(&mut self, config: &ExtensionConfig) -> Result<()> {
        self.metrics.init_calls.fetch_add(1, Ordering::Relaxed);
        self.name = config.name.clone();

        // First check API compatibility
        let api_info = self.get_api_info().await?;
        if api_info.version != config.api_version {
            return Err(anyhow::anyhow!(
                "API version mismatch: extension provides {}, host expects {}",
                api_info.version,
                config.api_version
            ));
        }

        // Check capabilities
        for capability in &config.capabilities {
            if !api_info.supported_capabilities.contains(capability) {
                return Err(anyhow::anyhow!(
                    "Extension {} does not support required capability: {}",
                    config.name,
                    capability
                ));
            }
        }

        // Placeholder implementation - would call WASM function
        tracing::info!("Extension {} initialized successfully", config.name);
        Ok(())
    }

    /// Get the GraphQL schema from the extension
    pub async fn get_schema(&mut self) -> Result<SchemaFragment> {
        self.metrics.schema_calls.fetch_add(1, Ordering::Relaxed);

        // Placeholder - return empty schema fragment for now
        Ok(SchemaFragment::default())
    }

    /// Run database migrations with timeout
    pub async fn migrate(&mut self, db_path: &str) -> Result<()> {
        self.metrics.migrate_calls.fetch_add(1, Ordering::Relaxed);

        // Placeholder implementation - would call WASM function
        tracing::info!(
            "Extension {} migration completed for DB: {}",
            self.name,
            db_path
        );
        Ok(())
    }

    /// Resolve a GraphQL field with concurrency control
    pub async fn resolve_field(&mut self, field: &str, args: &str) -> Result<String> {
        self.metrics.resolve_calls.fetch_add(1, Ordering::Relaxed);

        // Check concurrent operation limit
        let current_ops = self.concurrent_ops.fetch_add(1, Ordering::SeqCst);
        if current_ops >= self.limits.max_concurrent_ops as u64 {
            self.concurrent_ops.fetch_sub(1, Ordering::SeqCst);
            return Err(anyhow::anyhow!(
                "Extension {} exceeded concurrent operation limit",
                self.name
            ));
        }

        // Ensure we decrement the counter when done
        let _guard = ConcurrentOpsGuard {
            counter: self.concurrent_ops.clone(),
        };

        // Apply timeout to the operation
        let result = timeout(self.limits.operation_timeout, async {
            self.resolve_field_inner(field, args).await
        })
        .await;

        match result {
            Ok(inner_result) => inner_result,
            Err(_) => {
                self.metrics.total_timeouts.fetch_add(1, Ordering::Relaxed);
                Err(anyhow::anyhow!(
                    "Extension {} timed out resolving field {}",
                    self.name,
                    field
                ))
            }
        }
    }

    async fn resolve_field_inner(&mut self, field: &str, args: &str) -> Result<String> {
        // Reset fuel before operation if configured
        if let Some(max_fuel) = self.limits.max_fuel {
            self.store.set_fuel(max_fuel).map_err(|e| {
                anyhow::anyhow!("Failed to set fuel for extension {}: {}", self.name, e)
            })?;
        }

        // Update epoch for timeout interruption
        self.engine.increment_epoch();
        self.store.set_epoch_deadline(1);

        // Placeholder - return empty result
        tracing::debug!(
            "Extension {} resolved field {} with args {}",
            self.name,
            field,
            args
        );
        Ok(String::new())
    }
}

impl fmt::Debug for ExtensionInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtensionInstance")
            .field("name", &self.name)
            .field("limits", &self.limits)
            .field("metrics", &self.metrics)
            .field("concurrent_ops", &self.concurrent_ops)
            .finish()
    }
}

/// Guard to ensure concurrent operation counter is decremented
#[derive(Debug)]
struct ConcurrentOpsGuard {
    counter: Arc<AtomicU64>,
}

impl Drop for ConcurrentOpsGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
#[path = "interface_tests.rs"]
mod tests;
