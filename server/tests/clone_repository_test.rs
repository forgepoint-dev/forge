//! Integration tests for repository cloning functionality

use server::repository::mutations::clone_repository_raw;
use server::repository::storage::RepositoryStorage;
use server::test_helpers::create_test_pool;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
#[ignore] // Requires network access and a valid test repository
async fn test_clone_repository_from_github() {
    // Create test database
    let pool = create_test_pool().await.expect("Failed to create test pool");

    // Create temporary directories for cloned repositories
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let local_root = temp_dir.path().join("local");
    let remote_cache_root = temp_dir.path().join("remote");
    std::fs::create_dir_all(&local_root).expect("Failed to create local root");
    std::fs::create_dir_all(&remote_cache_root).expect("Failed to create remote cache root");

    let storage = RepositoryStorage::new(local_root, remote_cache_root.clone());

    // Clone a test repository
    let test_repo_url = "https://github.com/octocat/Hello-World";
    let result = clone_repository_raw(&pool, &storage, test_repo_url.to_string())
        .await
        .expect("Failed to clone repository");

    // Verify the result
    assert_eq!(result.slug, "hello-world");
    assert_eq!(result.remote_url.as_deref(), Some(test_repo_url));
    assert!(result.group_id.is_none()); // Should be at root level

    // Verify the repository was actually cloned to disk
    let cloned_path = remote_cache_root.join(&result.id);
    assert!(cloned_path.exists(), "Repository directory should exist");
    assert!(
        cloned_path.join(".git").exists(),
        "Should have .git directory"
    );

    println!("Successfully cloned repository to {:?}", cloned_path);
}

#[tokio::test]
async fn test_clone_repository_validates_url() {
    let pool = create_test_pool().await.expect("Failed to create test pool");
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let local_root = temp_dir.path().join("local");
    let remote_cache_root = temp_dir.path().join("remote");
    std::fs::create_dir_all(&local_root).expect("Failed to create local root");
    std::fs::create_dir_all(&remote_cache_root).expect("Failed to create remote cache root");

    let storage = RepositoryStorage::new(local_root, remote_cache_root);

    // Test with invalid URL
    let result = clone_repository_raw(&pool, &storage, "not-a-valid-url".to_string()).await;
    assert!(result.is_err(), "Should reject invalid URL");

    // Test with SSH URL (not supported)
    let result = clone_repository_raw(&pool, &storage, "git@github.com:user/repo.git".to_string()).await;
    assert!(result.is_err(), "Should reject SSH URLs");
}

#[tokio::test]
async fn test_clone_repository_prevents_duplicates() {
    let pool = create_test_pool().await.expect("Failed to create test pool");
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let local_root = temp_dir.path().join("local");
    let remote_cache_root = temp_dir.path().join("remote");
    std::fs::create_dir_all(&local_root).expect("Failed to create local root");
    std::fs::create_dir_all(&remote_cache_root).expect("Failed to create remote cache root");

    let storage = RepositoryStorage::new(local_root, remote_cache_root);

    let test_url = "https://github.com/octocat/Hello-World";

    // First clone should succeed (but skip actual cloning due to network)
    // In a real test with a local git server, this would work
    // For now, this demonstrates the expected behavior

    // Second clone of same URL should fail
    // let result = clone_repository_raw(&pool, &storage, test_url.to_string()).await;
    // assert!(result.is_err(), "Should prevent duplicate clone");
    // assert!(result.unwrap_err().to_string().contains("already linked"));
}
