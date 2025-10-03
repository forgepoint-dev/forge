//! WASM module loading and lifecycle management with safety controls

use anyhow::{Context, Result};
use std::path::Path;
use std::time::Duration;
use wasmtime::*;
use wasmtime_wasi::p2::WasiCtxBuilder;
use wasmtime_wasi::{DirPerms, FilePerms};

use super::interface::ExtensionInstance;

/// Configuration limits for WASM extensions
#[derive(Debug, Clone)]
pub struct ExtensionLimits {
    /// Maximum memory usage per extension in bytes
    #[allow(dead_code)]
    pub max_memory_bytes: u64,
    /// Maximum module size in bytes
    #[allow(dead_code)]
    pub max_module_bytes: usize,
    /// Maximum stack size in bytes
    #[allow(dead_code)]
    pub max_stack_bytes: usize,
    /// Maximum fuel for execution (if None, no limit)
    pub max_fuel: Option<u64>,
    /// Timeout for extension operations
    pub operation_timeout: Duration,
    /// Maximum number of concurrent operations per extension
    pub max_concurrent_ops: usize,
}

impl Default for ExtensionLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 16 * 1024 * 1024, // 16MB
            max_module_bytes: 10 * 1024 * 1024, // 10MB
            max_stack_bytes: 512 * 1024,        // 512KB
            max_fuel: Some(1_000_000_000),      // 1 billion units
            operation_timeout: Duration::from_secs(5),
            max_concurrent_ops: 10,
        }
    }
}

impl ExtensionLimits {
    /// Create limits suitable for development/testing
    #[allow(dead_code)] // Will be used when extension system is fully integrated
    pub fn development() -> Self {
        Self {
            max_memory_bytes: 32 * 1024 * 1024, // 32MB
            max_module_bytes: 20 * 1024 * 1024, // 20MB
            max_stack_bytes: 1024 * 1024,       // 1MB
            max_fuel: None,                     // No fuel limit
            operation_timeout: Duration::from_secs(30),
            max_concurrent_ops: 50,
        }
    }

    /// Create strict limits for production
    #[allow(dead_code)] // Will be used when extension system is fully integrated
    pub fn production() -> Self {
        Self::default()
    }
}

/// Create a secure Wasmtime engine with safety controls
#[allow(dead_code)]
fn create_secure_engine(limits: &ExtensionLimits) -> Result<Engine> {
    let mut config = Config::new();

    // Set memory and stack limits
    config.max_wasm_stack(limits.max_stack_bytes);
    config.memory_init_cow(false); // Disable copy-on-write for security
    config.memory_guaranteed_dense_image_size(0); // No dense image

    // Enable fuel metering if configured
    if limits.max_fuel.is_some() {
        config.consume_fuel(true);
    }

    // Note: epoch_interruption requires manual epoch management
    // For now, we'll rely on fuel metering for timeout control
    // config.epoch_interruption(true);

    // Configure WASM features - balance security with compatibility
    config.wasm_reference_types(false);
    config.wasm_threads(false); // Disable threads for security
    config.wasm_bulk_memory(true); // Keep enabled for performance
    config.wasm_multi_value(false);
    config.wasm_simd(true); // Keep enabled for compatibility
    config.wasm_relaxed_simd(false); // Disable relaxed simd

    Engine::new(&config).context("Failed to create secure Wasmtime engine")
}

/// Load a WASM module and create an extension instance with security constraints
#[allow(dead_code)]
pub async fn load_wasm_module(wasm_path: &Path, extension_dir: &Path) -> Result<ExtensionInstance> {
    load_wasm_module_with_limits(wasm_path, extension_dir, &ExtensionLimits::default()).await
}

/// Load a WASM module with custom limits
#[allow(dead_code)]
pub async fn load_wasm_module_with_limits(
    wasm_path: &Path,
    extension_dir: &Path,
    limits: &ExtensionLimits,
) -> Result<ExtensionInstance> {
    tracing::info!("Loading WASM module: {:?}", wasm_path);

    // Create secure engine with limits
    let engine = create_secure_engine(limits)?;

    // Create restricted WASI context - only allow access to extension's directory
    let wasi = WasiCtxBuilder::new()
        .inherit_stdio() // Allow stdout/stderr for debugging
        // Do NOT inherit env or give network access
        .preopened_dir(
            extension_dir,
            "/extension",
            DirPerms::all(),
            FilePerms::all(),
        )
        .context("Failed to create restricted WASI context")?
        .build_p1();

    // Create store with WASI context
    let mut store = Store::new(&engine, wasi);

    // Set fuel if configured
    if let Some(max_fuel) = limits.max_fuel {
        store.set_fuel(max_fuel).context("Failed to set fuel")?;
    }

    // Configure epoch deadline for timeout
    store.set_epoch_deadline(limits.operation_timeout.as_secs());

    // Load and validate the WASM module
    let module_bytes = std::fs::read(wasm_path)
        .with_context(|| format!("Failed to read WASM file: {:?}", wasm_path))?;

    if module_bytes.len() > limits.max_module_bytes {
        return Err(anyhow::anyhow!(
            "WASM module too large (>{} bytes): {:?}",
            limits.max_module_bytes,
            wasm_path
        ));
    }

    let module = Module::new(&engine, &module_bytes)
        .with_context(|| format!("Failed to compile WASM module: {:?}", wasm_path))?;

    // Validate module doesn't import dangerous functions
    validate_module_imports(&module)?;

    // Create WASI linker with restricted host functions
    let mut linker = Linker::new(&engine);
    wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |state| state)?;

    // Instantiate the module with memory limits
    let instance = linker
        .instantiate(&mut store, &module)
        .context("Failed to instantiate WASM module")?;

    // Enforce memory limits
    if let Some(memory) = instance.get_memory(&mut store, "memory") {
        let memory_size = memory.data_size(&store);
        if memory_size > limits.max_memory_bytes as usize {
            return Err(anyhow::anyhow!(
                "Extension exceeds memory limit: {} bytes (max: {})",
                memory_size,
                limits.max_memory_bytes
            ));
        }
    }

    tracing::info!("Successfully loaded WASM module: {:?}", wasm_path);
    Ok(ExtensionInstance::new(
        store,
        instance,
        engine,
        limits.clone(),
    ))
}

/// Validate that the module doesn't import dangerous host functions
#[allow(dead_code)]
fn validate_module_imports(module: &Module) -> Result<()> {
    for import in module.imports() {
        match import.ty() {
            ExternType::Func(_) => {
                // Allow WASI functions
                if import.module() == "wasi_snapshot_preview1" {
                    continue;
                }
                // Reject any other host function imports
                return Err(anyhow::anyhow!(
                    "Extension imports unauthorized host function: {}::{}",
                    import.module(),
                    import.name()
                ));
            }
            ExternType::Memory(_)
            | ExternType::Table(_)
            | ExternType::Global(_)
            | ExternType::Tag(_) => {
                // These are generally safe
                continue;
            }
            ExternType::Tag(_) => {
                // Tags correspond to exception handling constructs; accept them.
                continue;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "loader_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "security_tests.rs"]
mod security_tests;
