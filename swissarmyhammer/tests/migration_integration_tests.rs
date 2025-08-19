use swissarmyhammer::issues::{
    filesystem::{FileSystemIssueStorage, MigrationConfig, MigrationResult},
    IssueStorage,
};
use tempfile::TempDir;

#[tokio::test]
async fn test_automatic_migration_integration() {
    let original_dir = std::env::current_dir().unwrap();

    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Setup: Create old issues directory with test files
    let issues_dir = temp_dir.path().join("issues");
    std::fs::create_dir_all(&issues_dir).unwrap();
    std::fs::write(issues_dir.join("test1.md"), "Test issue 1").unwrap();
    std::fs::write(issues_dir.join("test2.md"), "Test issue 2").unwrap();

    // Test: Create storage with new_default() - should automatically migrate
    let storage = FileSystemIssueStorage::new_default().unwrap();

    // Verify: Migration occurred
    let new_issues_dir = temp_dir.path().join(".swissarmyhammer").join("issues");
    assert!(new_issues_dir.exists());
    assert!(new_issues_dir.join("test1.md").exists());
    assert!(new_issues_dir.join("test2.md").exists());
    
    // Verify: Old directory no longer exists
    assert!(!issues_dir.exists());

    // Verify: Storage works correctly after migration
    let issues = storage.list_issues().await.unwrap();
    assert_eq!(issues.len(), 2);

    std::env::set_current_dir(original_dir).unwrap();
}

#[tokio::test]
async fn test_new_default_with_migration_info_integration() {
    let original_dir = std::env::current_dir().unwrap();

    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Setup: Create old issues directory
    let issues_dir = temp_dir.path().join("issues");
    std::fs::create_dir_all(&issues_dir).unwrap();
    std::fs::write(issues_dir.join("test.md"), "Test content").unwrap();

    // Test: Use new_default_with_migration_info()
    let (storage, migration_result) = FileSystemIssueStorage::new_default_with_migration_info().unwrap();

    // Verify: Migration result is returned
    assert!(migration_result.is_some());
    match migration_result.unwrap() {
        MigrationResult::Success(stats) => {
            assert_eq!(stats.files_moved, 1);
            assert!(stats.bytes_moved > 0);
        },
        _ => panic!("Expected MigrationResult::Success"),
    }

    // Verify: Storage is functional
    let issues = storage.list_issues().await.unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].name, "test");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_migration_config_integration() {
    let original_dir = std::env::current_dir().unwrap();

    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Setup: Create issues directory with multiple files
    let issues_dir = temp_dir.path().join("issues");
    std::fs::create_dir_all(&issues_dir).unwrap();
    for i in 0..5 {
        std::fs::write(issues_dir.join(format!("test{}.md", i)), "Test content").unwrap();
    }

    // Test: Config with file count limit
    let config = MigrationConfig {
        auto_migrate: true,
        max_file_count: 3, // Less than the 5 files we have
        ..Default::default()
    };

    let result = FileSystemIssueStorage::new_default_with_config(&config);
    assert!(result.is_err());
    let error = result.err().unwrap();
    assert!(error.to_string().contains("exceeds maximum for automatic migration"));

    // Test: Config with higher limit should succeed
    let config = MigrationConfig {
        auto_migrate: true,
        max_file_count: 10, // More than the 5 files we have
        ..Default::default()
    };

    let result = FileSystemIssueStorage::new_default_with_config(&config);
    assert!(result.is_ok());

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_migration_config_disabled() {
    let original_dir = std::env::current_dir().unwrap();

    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Setup: Create old issues directory
    let issues_dir = temp_dir.path().join("issues");
    std::fs::create_dir_all(&issues_dir).unwrap();
    std::fs::write(issues_dir.join("test.md"), "Test content").unwrap();

    // Test: Config with migration disabled
    let config = MigrationConfig {
        auto_migrate: false,
        ..Default::default()
    };

    let (_storage, migration_result) = FileSystemIssueStorage::new_default_with_config(&config).unwrap();

    // Verify: No migration occurred
    assert!(migration_result.is_none());
    
    // Verify: Old directory still exists
    assert!(issues_dir.exists());
    assert!(issues_dir.join("test.md").exists());

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_migration_status_integration() {
    let original_dir = std::env::current_dir().unwrap();

    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Test: No issues directory
    let status = FileSystemIssueStorage::migration_status().unwrap();
    assert!(status.contains("No issues directory found"));

    // Setup: Create old issues directory
    let issues_dir = temp_dir.path().join("issues");
    std::fs::create_dir_all(&issues_dir).unwrap();
    std::fs::write(issues_dir.join("test.md"), "Test content").unwrap();

    // Test: Migration needed
    let status = FileSystemIssueStorage::migration_status().unwrap();
    assert!(status.contains("Migration needed"));
    assert!(status.contains("1 files"));

    // Perform migration
    let _storage = FileSystemIssueStorage::new_default().unwrap();

    // Test: No migration needed after migration
    let status = FileSystemIssueStorage::migration_status().unwrap();
    assert!(status.contains("Using .swissarmyhammer/issues/"));

    std::env::set_current_dir(original_dir).unwrap();
}

#[tokio::test]
async fn test_concurrent_storage_creation_integration() {
    let original_dir = std::env::current_dir().unwrap();

    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Setup: Create old issues directory
    let issues_dir = temp_dir.path().join("issues");
    std::fs::create_dir_all(&issues_dir).unwrap();
    std::fs::write(issues_dir.join("test.md"), "Test content").unwrap();

    // Test: Concurrent storage creation
    let handle1 = tokio::spawn(async {
        FileSystemIssueStorage::new_default()
    });
    let handle2 = tokio::spawn(async {
        FileSystemIssueStorage::new_default()
    });
    let handle3 = tokio::spawn(async {
        FileSystemIssueStorage::new_default()
    });

    let (result1, result2, result3) = tokio::join!(handle1, handle2, handle3);
    
    // Verify: All creations succeeded
    assert!(result1.unwrap().is_ok());
    assert!(result2.unwrap().is_ok());
    assert!(result3.unwrap().is_ok());

    // Verify: Migration completed successfully
    let new_issues_dir = temp_dir.path().join(".swissarmyhammer").join("issues");
    assert!(new_issues_dir.exists());
    assert!(new_issues_dir.join("test.md").exists());
    assert!(!issues_dir.exists());

    std::env::set_current_dir(original_dir).unwrap();
}

#[tokio::test] 
async fn test_migration_with_existing_destination_integration() {
    let original_dir = std::env::current_dir().unwrap();

    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Setup: Create both old and new directories
    let issues_dir = temp_dir.path().join("issues");
    let new_issues_dir = temp_dir.path().join(".swissarmyhammer").join("issues");
    
    std::fs::create_dir_all(&issues_dir).unwrap();
    std::fs::create_dir_all(&new_issues_dir).unwrap();
    
    std::fs::write(issues_dir.join("old.md"), "Old content").unwrap();
    std::fs::write(new_issues_dir.join("new.md"), "New content").unwrap();

    // Test: No migration should occur when destination exists
    let storage = FileSystemIssueStorage::new_default().unwrap();

    // Verify: Both directories still exist, no migration occurred
    assert!(issues_dir.exists());
    assert!(new_issues_dir.exists());
    assert!(issues_dir.join("old.md").exists());
    assert!(new_issues_dir.join("new.md").exists());

    // Verify: Storage uses the new directory
    let issues = storage.list_issues().await.unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].name, "new");

    std::env::set_current_dir(original_dir).unwrap();
}

/// Test CLI integration by simulating MCP tool context creation
#[tokio::test]
async fn test_cli_integration_migration() {
    let original_dir = std::env::current_dir().unwrap();

    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Setup: Create old issues directory
    let issues_dir = temp_dir.path().join("issues");
    std::fs::create_dir_all(&issues_dir).unwrap();
    std::fs::write(issues_dir.join("cli_test.md"), "CLI test issue").unwrap();

    // Simulate CLI storage creation (similar to what happens in mcp_integration.rs)
    let (storage, migration_result) = FileSystemIssueStorage::new_default_with_migration_info().unwrap();

    // Verify: Migration occurred
    assert!(migration_result.is_some());
    match migration_result.unwrap() {
        MigrationResult::Success(stats) => {
            assert_eq!(stats.files_moved, 1);
            // This would trigger the CLI message: "✅ Migrated 1 issues to .swissarmyhammer/issues"
        },
        _ => panic!("Expected MigrationResult::Success"),
    }

    // Verify: Storage is working
    let issues = storage.list_issues().await.unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].name, "cli_test");

    // Verify: Files are in new location
    let new_issues_dir = temp_dir.path().join(".swissarmyhammer").join("issues");
    assert!(new_issues_dir.exists());
    assert!(new_issues_dir.join("cli_test.md").exists());
    assert!(!issues_dir.exists());

    std::env::set_current_dir(original_dir).unwrap();
}

/// Test MCP server integration scenario
#[tokio::test]
async fn test_mcp_server_integration_migration() {
    let original_dir = std::env::current_dir().unwrap();

    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Setup: Create old issues directory with nested structure
    let issues_dir = temp_dir.path().join("issues");
    let complete_dir = issues_dir.join("complete");
    std::fs::create_dir_all(&complete_dir).unwrap();
    
    std::fs::write(issues_dir.join("active.md"), "Active issue").unwrap();
    std::fs::write(complete_dir.join("completed.md"), "Completed issue").unwrap();

    // Simulate MCP server storage creation
    let (storage, migration_result) = FileSystemIssueStorage::new_default_with_migration_info().unwrap();

    // Verify: Migration occurred with complete directory structure
    assert!(migration_result.is_some());
    match migration_result.unwrap() {
        MigrationResult::Success(stats) => {
            assert_eq!(stats.files_moved, 2); // Both active and completed files
            // This would trigger MCP server logging: "MCP server performed automatic migration: 2 files moved..."
        },
        _ => panic!("Expected MigrationResult::Success"),
    }

    // Verify: Storage handles both active and completed issues
    let issues = storage.list_issues().await.unwrap();
    assert_eq!(issues.len(), 2);
    
    let issue_names: Vec<&str> = issues.iter().map(|i| i.name.as_str()).collect();
    assert!(issue_names.contains(&"active"));
    assert!(issue_names.contains(&"completed"));

    // Verify: Directory structure preserved
    let new_issues_dir = temp_dir.path().join(".swissarmyhammer").join("issues");
    let new_complete_dir = new_issues_dir.join("complete");
    assert!(new_issues_dir.exists());
    assert!(new_complete_dir.exists());
    assert!(new_issues_dir.join("active.md").exists());
    assert!(new_complete_dir.join("completed.md").exists());
    assert!(!issues_dir.exists());

    std::env::set_current_dir(original_dir).unwrap();
}