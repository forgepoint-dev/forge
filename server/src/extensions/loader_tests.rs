use super::*;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn test_extension_limits_default() {
    let limits = ExtensionLimits::default();

    assert_eq!(limits.max_memory_bytes, 16 * 1024 * 1024);
    assert_eq!(limits.max_module_bytes, 10 * 1024 * 1024);
    assert_eq!(limits.max_stack_bytes, 512 * 1024);
    assert_eq!(limits.max_fuel, Some(1_000_000_000));
    assert_eq!(limits.operation_timeout, Duration::from_secs(5));
    assert_eq!(limits.max_concurrent_ops, 10);
}

#[test]
fn test_extension_limits_development() {
    let limits = ExtensionLimits::development();

    assert_eq!(limits.max_memory_bytes, 32 * 1024 * 1024);
    assert_eq!(limits.max_module_bytes, 20 * 1024 * 1024);
    assert_eq!(limits.max_stack_bytes, 1024 * 1024);
    assert_eq!(limits.max_fuel, None);
    assert_eq!(limits.operation_timeout, Duration::from_secs(30));
    assert_eq!(limits.max_concurrent_ops, 50);
}

#[test]
fn test_extension_limits_production() {
    let limits = ExtensionLimits::production();
    let default_limits = ExtensionLimits::default();

    // Production should match defaults
    assert_eq!(limits.max_memory_bytes, default_limits.max_memory_bytes);
    assert_eq!(limits.max_module_bytes, default_limits.max_module_bytes);
    assert_eq!(limits.max_stack_bytes, default_limits.max_stack_bytes);
    assert_eq!(limits.max_fuel, default_limits.max_fuel);
    assert_eq!(limits.operation_timeout, default_limits.operation_timeout);
    assert_eq!(limits.max_concurrent_ops, default_limits.max_concurrent_ops);
}

#[test]
fn test_custom_limits() {
    let limits = ExtensionLimits {
        max_memory_bytes: 8 * 1024 * 1024,
        max_module_bytes: 5 * 1024 * 1024,
        max_stack_bytes: 256 * 1024,
        max_fuel: Some(500_000_000),
        operation_timeout: Duration::from_secs(2),
        max_concurrent_ops: 5,
    };

    assert_eq!(limits.max_memory_bytes, 8 * 1024 * 1024);
    assert_eq!(limits.max_module_bytes, 5 * 1024 * 1024);
    assert_eq!(limits.max_stack_bytes, 256 * 1024);
    assert_eq!(limits.max_fuel, Some(500_000_000));
    assert_eq!(limits.operation_timeout, Duration::from_secs(2));
    assert_eq!(limits.max_concurrent_ops, 5);
}

#[tokio::test]
async fn test_module_size_limit_enforcement() {
    let temp_dir = TempDir::new().unwrap();
    let extension_dir = temp_dir.path().join("extension");
    std::fs::create_dir_all(&extension_dir).unwrap();

    // Create a fake WASM file that's too large
    let wasm_path = temp_dir.path().join("large.wasm");
    let large_content = vec![0u8; 11 * 1024 * 1024]; // 11MB
    std::fs::write(&wasm_path, large_content).unwrap();

    let limits = ExtensionLimits {
        max_module_bytes: 10 * 1024 * 1024, // 10MB limit
        ..ExtensionLimits::default()
    };

    let result = load_wasm_module_with_limits(&wasm_path, &extension_dir, &limits).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{:#}", err); // Use alternate formatting to show full error chain
    assert!(err_str.contains("WASM module too large"), "Error was: {}", err_str);
}

#[tokio::test]
async fn test_valid_module_size_passes() {
    let temp_dir = TempDir::new().unwrap();
    let extension_dir = temp_dir.path().join("extension");
    std::fs::create_dir_all(&extension_dir).unwrap();

    // Create an invalid WASM module that passes size check but fails compilation
    // Invalid because it has valid header but truncated content
    let wasm_path = temp_dir.path().join("invalid.wasm");
    let wasm_content = vec![
        0x00, 0x61, 0x73, 0x6d, // WASM magic number
        0x01, 0x00, 0x00, 0x00, // Version 1
        0x01, // Section ID (type section) but no content
    ];
    std::fs::write(&wasm_path, wasm_content).unwrap();

    let limits = ExtensionLimits {
        max_module_bytes: 1024, // 1KB limit (our module is 8 bytes)
        ..ExtensionLimits::default()
    };

    // This will fail during module compilation but not due to size
    let result = load_wasm_module_with_limits(&wasm_path, &extension_dir, &limits).await;

    // The module will fail to compile (incomplete WASM), but not due to size limits
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(!err.to_string().contains("WASM module too large"));
}

#[test]
fn test_limits_clone() {
    let original = ExtensionLimits::development();
    let cloned = original.clone();

    assert_eq!(original.max_memory_bytes, cloned.max_memory_bytes);
    assert_eq!(original.max_module_bytes, cloned.max_module_bytes);
    assert_eq!(original.max_stack_bytes, cloned.max_stack_bytes);
    assert_eq!(original.max_fuel, cloned.max_fuel);
    assert_eq!(original.operation_timeout, cloned.operation_timeout);
    assert_eq!(original.max_concurrent_ops, cloned.max_concurrent_ops);
}

#[test]
fn test_limits_debug_format() {
    let limits = ExtensionLimits::default();
    let debug_str = format!("{:?}", limits);

    assert!(debug_str.contains("ExtensionLimits"));
    assert!(debug_str.contains("max_memory_bytes"));
    assert!(debug_str.contains("max_module_bytes"));
    assert!(debug_str.contains("max_stack_bytes"));
    assert!(debug_str.contains("max_fuel"));
    assert!(debug_str.contains("operation_timeout"));
    assert!(debug_str.contains("max_concurrent_ops"));
}
