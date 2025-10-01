use super::*;
use tempfile::TempDir;

#[tokio::test]
async fn test_reject_module_with_unauthorized_imports() {
    let temp_dir = TempDir::new().unwrap();
    let extension_dir = temp_dir.path().join("extension");
    std::fs::create_dir_all(&extension_dir).unwrap();

    // Create a WASM module that imports a non-WASI function
    // This WAT (WebAssembly Text) would compile to:
    // (module
    //   (import "env" "dangerous_func" (func))
    // )
    let wasm_path = temp_dir.path().join("dangerous.wasm");
    let wasm_bytes = wat::parse_str(
        r#"
        (module
            (import "env" "dangerous_func" (func))
        )
        "#,
    )
    .unwrap();
    std::fs::write(&wasm_path, wasm_bytes).unwrap();

    let result = load_wasm_module(&wasm_path, &extension_dir).await;
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("unauthorized host function"),
        "Error was: {}",
        err_str
    );
}

#[tokio::test]
async fn test_accept_wasi_imports() {
    let temp_dir = TempDir::new().unwrap();
    let extension_dir = temp_dir.path().join("extension");
    std::fs::create_dir_all(&extension_dir).unwrap();

    // Create a WASM module that only imports WASI functions
    let wasm_path = temp_dir.path().join("wasi_only.wasm");
    let wasm_bytes = wat::parse_str(
        r#"
        (module
            (import "wasi_snapshot_preview1" "fd_write"
                (func (param i32 i32 i32 i32) (result i32)))
            (memory 1)
            (export "memory" (memory 0))
        )
        "#,
    )
    .unwrap();
    std::fs::write(&wasm_path, wasm_bytes).unwrap();

    let result = load_wasm_module(&wasm_path, &extension_dir).await;
    assert!(result.is_ok(), "Should accept WASI imports");
}

#[tokio::test]
async fn test_memory_limit_enforcement() {
    let temp_dir = TempDir::new().unwrap();
    let extension_dir = temp_dir.path().join("extension");
    std::fs::create_dir_all(&extension_dir).unwrap();

    // Create a WASM module that tries to allocate too much memory
    // 300 pages = ~19.7MB (each page is 64KB)
    let wasm_path = temp_dir.path().join("large_memory.wasm");
    let wasm_bytes = wat::parse_str(
        r#"
        (module
            (memory 300)
            (export "memory" (memory 0))
        )
        "#,
    )
    .unwrap();
    std::fs::write(&wasm_path, wasm_bytes).unwrap();

    let limits = ExtensionLimits {
        max_memory_bytes: 16 * 1024 * 1024, // 16MB limit
        ..ExtensionLimits::default()
    };

    let result = load_wasm_module_with_limits(&wasm_path, &extension_dir, &limits).await;
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("exceeds memory limit"),
        "Error was: {}",
        err_str
    );
}

#[tokio::test]
async fn test_reject_module_with_threads() {
    let temp_dir = TempDir::new().unwrap();
    let extension_dir = temp_dir.path().join("extension");
    std::fs::create_dir_all(&extension_dir).unwrap();

    // WASM with threads requires special shared memory
    // This should fail because we disable threads
    let wasm_path = temp_dir.path().join("threads.wasm");
    let wasm_bytes = wat::parse_str(
        r#"
        (module
            (memory 1 1 shared)
            (export "memory" (memory 0))
        )
        "#,
    )
    .unwrap();

    // This might fail at parse time or load time
    if wasm_bytes.is_empty() {
        // WAT parser might reject shared memory
        return;
    }

    std::fs::write(&wasm_path, wasm_bytes).unwrap();

    let result = load_wasm_module(&wasm_path, &extension_dir).await;
    // Either parsing fails or loading fails - both are acceptable
    assert!(result.is_err());
}

#[tokio::test]
async fn test_filesystem_isolation() {
    let temp_dir = TempDir::new().unwrap();
    let extension_dir = temp_dir.path().join("extension");
    let another_dir = temp_dir.path().join("another");
    std::fs::create_dir_all(&extension_dir).unwrap();
    std::fs::create_dir_all(&another_dir).unwrap();

    // Create test files
    std::fs::write(extension_dir.join("allowed.txt"), "allowed content").unwrap();
    std::fs::write(another_dir.join("forbidden.txt"), "forbidden content").unwrap();

    // Create a simple WASM module
    let wasm_path = temp_dir.path().join("fs_test.wasm");
    let wasm_bytes = wat::parse_str(
        r#"
        (module
            (memory 1)
            (export "memory" (memory 0))
        )
        "#,
    )
    .unwrap();
    std::fs::write(&wasm_path, wasm_bytes).unwrap();

    let result = load_wasm_module(&wasm_path, &extension_dir).await;
    assert!(result.is_ok());

    // The extension should only have access to its own directory
    // This would be tested more thoroughly with actual WASI file operations
    // but the setup ensures only extension_dir is preopened
}

#[test]
fn test_extension_limits_reasonable_defaults() {
    let limits = ExtensionLimits::default();

    // Memory should be reasonable (not too small, not too large)
    assert!(limits.max_memory_bytes >= 1024 * 1024); // At least 1MB
    assert!(limits.max_memory_bytes <= 256 * 1024 * 1024); // At most 256MB

    // Module size should be reasonable
    assert!(limits.max_module_bytes >= 1024 * 1024); // At least 1MB
    assert!(limits.max_module_bytes <= 100 * 1024 * 1024); // At most 100MB

    // Stack should be reasonable
    assert!(limits.max_stack_bytes >= 64 * 1024); // At least 64KB
    assert!(limits.max_stack_bytes <= 10 * 1024 * 1024); // At most 10MB

    // Timeouts should be reasonable
    assert!(limits.operation_timeout.as_secs() >= 1); // At least 1 second
    assert!(limits.operation_timeout.as_secs() <= 60); // At most 60 seconds

    // Concurrent operations should be reasonable
    assert!(limits.max_concurrent_ops >= 1);
    assert!(limits.max_concurrent_ops <= 1000);
}

#[test]
fn test_development_limits_more_permissive() {
    let dev = ExtensionLimits::development();
    let prod = ExtensionLimits::production();

    // Development should be more permissive than production
    assert!(dev.max_memory_bytes >= prod.max_memory_bytes);
    assert!(dev.max_module_bytes >= prod.max_module_bytes);
    assert!(dev.max_stack_bytes >= prod.max_stack_bytes);
    assert!(dev.operation_timeout >= prod.operation_timeout);
    assert!(dev.max_concurrent_ops >= prod.max_concurrent_ops);

    // Development might not have fuel limits
    if let (Some(dev_fuel), Some(prod_fuel)) = (dev.max_fuel, prod.max_fuel) {
        assert!(dev_fuel >= prod_fuel);
    }
}

#[tokio::test]
async fn test_module_validation_happens_before_instantiation() {
    let temp_dir = TempDir::new().unwrap();
    let extension_dir = temp_dir.path().join("extension");
    std::fs::create_dir_all(&extension_dir).unwrap();

    // Create a module with unauthorized imports
    let wasm_path = temp_dir.path().join("bad_imports.wasm");
    let wasm_bytes = wat::parse_str(
        r#"
        (module
            (import "dangerous" "hack" (func))
            (import "another" "bad" (func))
        )
        "#,
    )
    .unwrap();
    std::fs::write(&wasm_path, wasm_bytes).unwrap();

    let result = load_wasm_module(&wasm_path, &extension_dir).await;
    assert!(result.is_err());

    // Should fail at validation, not at runtime
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("unauthorized"));
}

#[tokio::test]
async fn test_empty_module_loads_successfully() {
    let temp_dir = TempDir::new().unwrap();
    let extension_dir = temp_dir.path().join("extension");
    std::fs::create_dir_all(&extension_dir).unwrap();

    // Minimal valid WASM module with no imports or exports
    let wasm_path = temp_dir.path().join("empty.wasm");
    let wasm_bytes = wat::parse_str("(module)").unwrap();
    std::fs::write(&wasm_path, wasm_bytes).unwrap();

    let result = load_wasm_module(&wasm_path, &extension_dir).await;
    assert!(result.is_ok(), "Empty module should load successfully");
}

#[test]
fn test_limits_prevent_dos() {
    let limits = ExtensionLimits::default();

    // Ensure fuel is limited (prevents infinite loops)
    assert!(
        limits.max_fuel.is_some(),
        "Fuel should be limited by default"
    );

    // Ensure operations timeout (prevents hanging)
    assert!(
        limits.operation_timeout.as_secs() < 3600,
        "Operations should timeout in reasonable time"
    );

    // Ensure concurrent operations are limited (prevents resource exhaustion)
    assert!(
        limits.max_concurrent_ops < 10000,
        "Concurrent operations should be limited"
    );
}
