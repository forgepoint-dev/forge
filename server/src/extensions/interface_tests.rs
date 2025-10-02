use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::task::JoinSet;
use wasmtime::Engine;
use wasmtime_wasi::p2::WasiCtxBuilder;

#[test]
fn test_concurrent_ops_guard_decrements_on_drop() {
    let counter = Arc::new(AtomicU64::new(5));

    {
        let _guard = ConcurrentOpsGuard {
            counter: counter.clone(),
        };
        assert_eq!(counter.load(Ordering::SeqCst), 5);
        // guard drops here
    }

    // Counter should be decremented after guard drops
    assert_eq!(counter.load(Ordering::SeqCst), 4);
}

#[test]
fn test_concurrent_ops_guard_multiple_drops() {
    let counter = Arc::new(AtomicU64::new(10));

    {
        let _guard1 = ConcurrentOpsGuard {
            counter: counter.clone(),
        };
        let _guard2 = ConcurrentOpsGuard {
            counter: counter.clone(),
        };
        let _guard3 = ConcurrentOpsGuard {
            counter: counter.clone(),
        };
        assert_eq!(counter.load(Ordering::SeqCst), 10);
        // all guards drop here
    }

    // Counter should be decremented by 3
    assert_eq!(counter.load(Ordering::SeqCst), 7);
}

#[tokio::test]
async fn test_concurrent_operation_limit_enforced() {
    use wasmtime::Engine;
    use wasmtime::Store;
    use wasmtime_wasi::p2::WasiCtxBuilder;

    // Create a minimal extension instance for testing
    let limits = ExtensionLimits {
        max_concurrent_ops: 2,
        operation_timeout: Duration::from_secs(1),
        ..ExtensionLimits::default()
    };

    // Create minimal wasmtime components
    let engine = Engine::default();
    let wasi = WasiCtxBuilder::new().build_p1();
    let mut store = Store::new(&engine, wasi);
    let module = wasmtime::Module::new(&engine, "(module)").unwrap();
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();

    let mut ext = ExtensionInstance::new(store, instance, engine, limits);
    ext.set_name("test".to_string());

    // Manually increment the counter to simulate concurrent operations
    ext.concurrent_ops.store(2, Ordering::SeqCst);

    // This should fail because we're at the limit
    let result = ext.resolve_field("test", "{}").await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("exceeded concurrent operation limit")
    );
}

#[test]
fn test_extension_metrics_initialization() {
    let metrics = ExtensionMetrics::new();

    assert_eq!(metrics.init_calls.load(Ordering::Relaxed), 0);
    assert_eq!(metrics.schema_calls.load(Ordering::Relaxed), 0);
    assert_eq!(metrics.migrate_calls.load(Ordering::Relaxed), 0);
    assert_eq!(metrics.resolve_calls.load(Ordering::Relaxed), 0);
    assert_eq!(metrics.total_errors.load(Ordering::Relaxed), 0);
    assert_eq!(metrics.total_timeouts.load(Ordering::Relaxed), 0);
}

#[test]
fn test_extension_metrics_increment() {
    let metrics = ExtensionMetrics::new();

    metrics.init_calls.fetch_add(1, Ordering::Relaxed);
    metrics.schema_calls.fetch_add(2, Ordering::Relaxed);
    metrics.migrate_calls.fetch_add(3, Ordering::Relaxed);
    metrics.resolve_calls.fetch_add(4, Ordering::Relaxed);
    metrics.total_errors.fetch_add(5, Ordering::Relaxed);
    metrics.total_timeouts.fetch_add(6, Ordering::Relaxed);

    assert_eq!(metrics.init_calls.load(Ordering::Relaxed), 1);
    assert_eq!(metrics.schema_calls.load(Ordering::Relaxed), 2);
    assert_eq!(metrics.migrate_calls.load(Ordering::Relaxed), 3);
    assert_eq!(metrics.resolve_calls.load(Ordering::Relaxed), 4);
    assert_eq!(metrics.total_errors.load(Ordering::Relaxed), 5);
    assert_eq!(metrics.total_timeouts.load(Ordering::Relaxed), 6);
}

#[test]
fn test_extension_config_serialization() {
    let config = ExtensionConfig {
        name: "test_ext".to_string(),
        db_path: "/path/to/db".to_string(),
        config: Some("custom config".to_string()),
        api_version: "1.0.0".to_string(),
        capabilities: vec!["cap1".to_string(), "cap2".to_string()],
    };

    // Test serialization
    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("test_ext"));
    assert!(json.contains("/path/to/db"));
    assert!(json.contains("custom config"));
    assert!(json.contains("1.0.0"));
    assert!(json.contains("cap1"));
    assert!(json.contains("cap2"));

    // Test deserialization
    let deserialized: ExtensionConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, config.name);
    assert_eq!(deserialized.db_path, config.db_path);
    assert_eq!(deserialized.config, config.config);
    assert_eq!(deserialized.api_version, config.api_version);
    assert_eq!(deserialized.capabilities, config.capabilities);
}

#[test]
fn test_api_info_deserialization() {
    let json = r#"{
        "version": "2.0.0",
        "supported_capabilities": ["basic", "advanced"]
    }"#;

    let api_info: ApiInfo = serde_json::from_str(json).unwrap();
    assert_eq!(api_info.version, "2.0.0");
    assert_eq!(api_info.supported_capabilities, vec!["basic", "advanced"]);
}

#[tokio::test]
async fn test_concurrent_operations_counter_accuracy() {
    let counter = Arc::new(AtomicU64::new(0));
    let mut tasks = JoinSet::new();

    // Spawn multiple tasks that increment and decrement the counter
    for _ in 0..100 {
        let counter_clone = counter.clone();
        tasks.spawn(async move {
            // Simulate an operation
            counter_clone.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_micros(10)).await;
            counter_clone.fetch_sub(1, Ordering::SeqCst);
        });
    }

    // Wait for all tasks to complete
    while tasks.join_next().await.is_some() {}

    // Counter should be back to 0
    assert_eq!(counter.load(Ordering::SeqCst), 0);
}

#[test]
fn test_extension_instance_debug_format() {
    let limits = ExtensionLimits::default();
    let engine = Engine::default();
    let wasi = WasiCtxBuilder::new().build_p1();
    let mut store = Store::new(&engine, wasi);
    let module = wasmtime::Module::new(&engine, "(module)").unwrap();
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();

    let ext = ExtensionInstance::new(store, instance, engine, limits);

    let debug_str = format!("{:?}", ext);
    assert!(debug_str.contains("ExtensionInstance"));
    assert!(debug_str.contains("name"));
    assert!(debug_str.contains("limits"));
    assert!(debug_str.contains("metrics"));
    assert!(debug_str.contains("concurrent_ops"));
}
