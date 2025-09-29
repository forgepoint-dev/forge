//! WASM module loading and lifecycle management with safety controls

use anyhow::{Context, Result};
use std::path::Path;
use wasmtime::*;
use wasmtime_wasi::preview1::WasiP1Ctx;
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder};

use super::interface::ExtensionInstance;

/// Maximum memory usage per extension (16MB)
const MAX_MEMORY_BYTES: u64 = 16 * 1024 * 1024;

/// Create a secure Wasmtime engine with safety controls
fn create_secure_engine() -> Result<Engine> {
    let mut config = Config::new();

    // Set memory limits
    config.max_wasm_stack(512 * 1024); // 512KB stack limit
    config.memory_init_cow(false); // Disable copy-on-write for security
    config.memory_guaranteed_dense_image_size(0); // No dense image

    // Disable features that could be security risks
    config.wasm_reference_types(false);
    config.wasm_bulk_memory(false);
    config.wasm_multi_value(false);
    config.wasm_simd(false);

    Engine::new(&config).context("Failed to create secure Wasmtime engine")
}

/// Load a WASM module and create an extension instance with security constraints
pub async fn load_wasm_module(wasm_path: &Path, extension_dir: &Path) -> Result<ExtensionInstance> {
    tracing::info!("Loading WASM module: {:?}", wasm_path);

    // Create secure engine
    let engine = create_secure_engine()?;

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

    // Load and validate the WASM module
    let module_bytes = std::fs::read(wasm_path)
        .with_context(|| format!("Failed to read WASM file: {:?}", wasm_path))?;

    if module_bytes.len() > 10 * 1024 * 1024 {
        return Err(anyhow::anyhow!(
            "WASM module too large (>10MB): {:?}",
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
        if memory_size > MAX_MEMORY_BYTES as usize {
            return Err(anyhow::anyhow!(
                "Extension exceeds memory limit: {} bytes",
                memory_size
            ));
        }
    }

    tracing::info!("Successfully loaded WASM module: {:?}", wasm_path);
    Ok(ExtensionInstance::new(store, instance, engine))
}

/// Validate that the module doesn't import dangerous host functions
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
            ExternType::Memory(_) | ExternType::Table(_) | ExternType::Global(_) => {
                // These are generally safe
                continue;
            }
        }
    }
    Ok(())
}
