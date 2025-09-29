use std::fs;
use std::path::Path;
use wasmtime::{Engine, Linker, Module, Store};
use wasi_common::sync::{add_to_linker, Dir, WasiCtxBuilder};
use wasi_common::WasiCtx;

pub fn load_extensions(extensions_dir: &str, db_dir: &str) -> anyhow::Result<()> {
    let engine = Engine::default();
    let mut linker: Linker<WasiCtx> = Linker::new(&engine);
    add_to_linker(&mut linker, |s: &mut WasiCtx| s)?;

    if !Path::new(extensions_dir).exists() {
        println!("Extensions directory not found: {}", extensions_dir);
        return Ok(());
    }

    for entry in fs::read_dir(extensions_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
            let module_name = path.file_stem().unwrap().to_str().unwrap().to_string();
            println!("Loading extension: {}", module_name);

            let preopened_dir = Dir::open_ambient_dir(db_dir, wasi_common::sync::ambient_authority())?;

            let wasi = WasiCtxBuilder::new()
                .inherit_stdio()
                .preopened_dir(preopened_dir, ".")?
                .build();

            let mut store = Store::new(&engine, wasi);

            let module = Module::from_file(&engine, &path)?;
            let instance = linker.instantiate(&mut store, &module)?;

            println!("Successfully instantiated extension: {}", module_name);

            let run_func = instance
                .get_typed_func::<(), i32>(&mut store, "run")?;

            let ptr = run_func.call(&mut store, ())?;

            let memory = instance
                .get_memory(&mut store, "memory")
                .expect("`memory` export not found");

            let mut buffer = Vec::new();
            let mut offset = ptr as usize;
            loop {
                let byte = memory.data(&store)[offset];
                if byte == 0 {
                    break;
                }
                buffer.push(byte);
                offset += 1;
            }

            let result_str = String::from_utf8(buffer)?;
            println!("Result from extension '{}': {}", module_name, result_str);
        }
    }
    Ok(())
}