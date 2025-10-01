//! Integration test for WASM extensions

use server::extensions::{loader::ExtensionLimits, wasm_runtime};
use server::test_helpers;
use std::path::Path;
use tempfile::TempDir;

#[tokio::test]
async fn test_load_example_extension() {
    // Path to our compiled example extension
    let wasm_path = Path::new("extensions_dir/users.wasm");

    if !wasm_path.exists() {
        eprintln!("Skipping test - example extension not built");
        return;
    }

    // Create temporary directory for extension
    let temp_dir = TempDir::new().unwrap();
    let extension_dir = temp_dir.path();

    // Create in-memory database pool for testing
    let pool = test_helpers::create_test_pool_no_migrations()
        .await
        .unwrap();

    // Load the extension with in-memory database
    let limits = ExtensionLimits::default();
    let extension = wasm_runtime::Extension::load_with_pool(
        wasm_path,
        extension_dir,
        "test-users".to_string(),
        pool,
        &limits,
    )
    .await
    .expect("Failed to load extension");

    // Check schema
    let schema = extension.schema();
    assert!(schema.contains("type User"));
    assert!(schema.contains("id: ID!"));
    assert!(schema.contains("name: String!"));
    assert!(schema.contains("email: String!"));
    println!("Extension schema:\n{}", schema);

    // Test field resolution - resolve users query
    let result = extension
        .resolve_field(
            "users".to_string(),
            "Query".to_string(),
            serde_json::json!({}),
            serde_json::json!({}),
            None,
        )
        .await
        .expect("Failed to resolve field");

    // Check we got an array of users
    assert!(result.is_array());
    let users = result.as_array().unwrap();
    assert_eq!(users.len(), 3);

    // Verify first user
    let first_user = &users[0];
    assert_eq!(first_user["name"], "Alice");
    assert_eq!(first_user["email"], "alice@example.com");

    println!("Successfully resolved users: {:?}", result);
}

#[tokio::test]
async fn test_extension_field_resolution_with_arguments() {
    let wasm_path = Path::new("extensions_dir/users.wasm");

    if !wasm_path.exists() {
        eprintln!("Skipping test - example extension not built");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let extension_dir = temp_dir.path();

    // Create in-memory database pool for testing
    let pool = test_helpers::create_test_pool_no_migrations()
        .await
        .unwrap();

    let limits = ExtensionLimits::default();
    let extension = wasm_runtime::Extension::load_with_pool(
        wasm_path,
        extension_dir,
        "test-users".to_string(),
        pool,
        &limits,
    )
    .await
    .expect("Failed to load extension");

    // Test user query with ID argument
    let result = extension
        .resolve_field(
            "user".to_string(),
            "Query".to_string(),
            serde_json::json!({"id": "42"}),
            serde_json::json!({}),
            None,
        )
        .await
        .expect("Failed to resolve field");

    // Verify the user returned
    assert_eq!(result["id"], "42");
    assert_eq!(result["name"], "User 42");
    assert_eq!(result["email"], "user42@example.com");

    println!("Successfully resolved user with ID 42: {:?}", result);
}

#[tokio::test]
async fn test_extension_error_handling() {
    let wasm_path = Path::new("extensions_dir/users.wasm");

    if !wasm_path.exists() {
        eprintln!("Skipping test - example extension not built");
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let extension_dir = temp_dir.path();

    // Create in-memory database pool for testing
    let pool = test_helpers::create_test_pool_no_migrations()
        .await
        .unwrap();

    let limits = ExtensionLimits::default();
    let extension = wasm_runtime::Extension::load_with_pool(
        wasm_path,
        extension_dir,
        "test-users".to_string(),
        pool,
        &limits,
    )
    .await
    .expect("Failed to load extension");

    // Try to resolve unknown field
    let result = extension
        .resolve_field(
            "unknown_field".to_string(),
            "Query".to_string(),
            serde_json::json!({}),
            serde_json::json!({}),
            None,
        )
        .await;

    // Should return an error
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Unknown field"));
}
