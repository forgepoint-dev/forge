//! WASM module loading and lifecycle management

use anyhow::{Context, Result};
use std::path::Path;
use wasmtime::*;
use wasmtime_wasi::{WasiCtxBuilder, DirPerms, FilePerms};
use wasmtime_wasi::preview1::WasiP1Ctx;

use super::interface::ExtensionInstance;

/// Load a WASM module and create an extension instance
pub async fn load_wasm_module(wasm_path: &Path) -> Result<ExtensionInstance> {
    // Create Wasmtime engine with WASI support
    let engine = Engine::default();
    
    // Create WASI context with filesystem access
    let wasi = WasiCtxBuilder::new()
        .inherit_stdio()
        .inherit_env()
        .preopened_dir(".", ".", DirPerms::all(), FilePerms::all())
        .context("Failed to create WASI context")?
        .build_p1();

    // Create store with WASI context
    let mut store = Store::new(&engine, wasi);

    // Load the WASM module
    let module_bytes = std::fs::read(wasm_path)
        .with_context(|| format!("Failed to read WASM file: {:?}", wasm_path))?;
    
    let module = Module::new(&engine, &module_bytes)
        .with_context(|| format!("Failed to compile WASM module: {:?}", wasm_path))?;

    // Create WASI linker
    let mut linker = Linker::new(&engine);
    wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |state| state)?;

    // Instantiate the module
    let instance = linker
        .instantiate(&mut store, &module)
        .context("Failed to instantiate WASM module")?;

    Ok(ExtensionInstance::new(store, instance))
}