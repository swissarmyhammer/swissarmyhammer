use swissarmyhammer::issues::{
    filesystem::{FileSystemIssueStorage, MigrationConfig, MigrationResult},
    IssueStorage,
};
use tempfile::TempDir;

#[tokio::test]
async fn test_automatic_migration_integration() {
    let temp_dir = TempDir::new().unwrap();

    // Setup: Create old issues directory with test files
    let issues_dir = temp_dir.path().join("issues");
    std::fs::create_dir_all(&issues_dir).unwrap();
    std::fs::write(issues_dir.join("test1.md"), "Test issue 1").unwrap();
    std::fs::write(issues_dir.join("test2.md"), "Test issue 2").unwrap();

    // Test: Create storage with new_default_in() - should automatically migrate
    let (storage, _migration_result) =
        FileSystemIssueStorage::new_default_in(temp_dir.path()).unwrap();

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
}

#[tokio::test]
async fn test_new_default_with_migration_info_integration() {
    let temp_dir = TempDir::new().unwrap();

    // Setup: Create old issues directory
    let issues_dir = temp_dir.path().join("issues");
    std::fs::create_dir_all(&issues_dir).unwrap();
    std::fs::write(issues_dir.join("test.md"), "Test content").unwrap();

    // Test: Use new_default_in() which provides migration info
    let (storage, migration_result) =
        FileSystemIssueStorage::new_default_in(temp_dir.path()).unwrap();

    // Verify: Migration result is returned
    assert!(migration_result.is_some());
    match migration_result.unwrap() {
        MigrationResult::Success(stats) => {
            assert_eq!(stats.files_moved, 1);
            assert!(stats.bytes_moved > 0);
        }
        _ => panic!("Expected MigrationResult::Success"),
    }

    // Verify: Storage is functional
    let issues = storage.list_issues().await.unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].name, "test");
}

#[test]
fn test_migration_config_integration() {
    let temp_dir = TempDir::new().unwrap();

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

    let result = FileSystemIssueStorage::new_default_with_config_in(temp_dir.path(), &config);
    assert!(result.is_err());
    let error = result.err().unwrap();
    assert!(error
        .to_string()
        .contains("exceeds maximum for automatic migration"));

    // Test: Config with higher limit should succeed
    let config = MigrationConfig {
        auto_migrate: true,
        max_file_count: 10, // More than the 5 files we have
        ..Default::default()
    };

    let result = FileSystemIssueStorage::new_default_with_config_in(temp_dir.path(), &config);
    assert!(result.is_ok());
}

#[test]
fn test_migration_config_disabled() {
    let temp_dir = TempDir::new().unwrap();

    // Setup: Create old issues directory
    let issues_dir = temp_dir.path().join("issues");
    std::fs::create_dir_all(&issues_dir).unwrap();
    std::fs::write(issues_dir.join("test.md"), "Test content").unwrap();

    // Test: Config with migration disabled
    let config = MigrationConfig {
        auto_migrate: false,
        ..Default::default()
    };

    let (_storage, migration_result) =
        FileSystemIssueStorage::new_default_with_config_in(temp_dir.path(), &config).unwrap();

    // Verify: No migration occurred
    assert!(migration_result.is_none());

    // Verify: Old directory still exists
    assert!(issues_dir.exists());
    assert!(issues_dir.join("test.md").exists());
}

#[test]
fn test_migration_status_integration() {
    let temp_dir = TempDir::new().unwrap();

    // Test: No issues directory - we need a method that works with specific directory
    // For now, let's create the directory structure first
    let issues_dir = temp_dir.path().join("issues");
    std::fs::create_dir_all(&issues_dir).unwrap();
    std::fs::write(issues_dir.join("test.md"), "Test content").unwrap();

    // Test: Check migration status by checking if migration should occur
    let should_migrate = FileSystemIssueStorage::should_migrate_in(temp_dir.path()).unwrap();
    assert!(should_migrate);

    // Perform migration
    let (_storage, _result) = FileSystemIssueStorage::new_default_in(temp_dir.path()).unwrap();

    // Test: No migration needed after migration
    let should_migrate_after = FileSystemIssueStorage::should_migrate_in(temp_dir.path()).unwrap();
    assert!(!should_migrate_after);
}

#[tokio::test]
async fn test_concurrent_storage_creation_integration() {
    let temp_dir = TempDir::new().unwrap();

    // Setup: Create old issues directory
    let issues_dir = temp_dir.path().join("issues");
    std::fs::create_dir_all(&issues_dir).unwrap();
    std::fs::write(issues_dir.join("test.md"), "Test content").unwrap();

    // Test: Concurrent storage creation - each with its own temp directory to avoid conflicts
    let temp_dir_path = temp_dir.path().to_path_buf();
    let handle1 = {
        let path = temp_dir_path.clone();
        tokio::spawn(async move { FileSystemIssueStorage::new_default_in(&path) })
    };
    let handle2 = {
        let path = temp_dir_path.clone();
        tokio::spawn(async move { FileSystemIssueStorage::new_default_in(&path) })
    };
    let handle3 = {
        let path = temp_dir_path.clone();
        tokio::spawn(async move { FileSystemIssueStorage::new_default_in(&path) })
    };

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
}

#[tokio::test]
async fn test_migration_with_existing_destination_integration() {
    let temp_dir = TempDir::new().unwrap();

    // Setup: Create both old and new directories
    let issues_dir = temp_dir.path().join("issues");
    let new_issues_dir = temp_dir.path().join(".swissarmyhammer").join("issues");

    std::fs::create_dir_all(&issues_dir).unwrap();
    std::fs::create_dir_all(&new_issues_dir).unwrap();

    std::fs::write(issues_dir.join("old.md"), "Old content").unwrap();
    std::fs::write(new_issues_dir.join("new.md"), "New content").unwrap();

    // Test: No migration should occur when destination exists
    let (storage, migration_result) =
        FileSystemIssueStorage::new_default_in(temp_dir.path()).unwrap();

    // Verify: No migration occurred
    assert!(migration_result.is_none());

    // Verify: Both directories still exist, no migration occurred
    assert!(issues_dir.exists());
    assert!(new_issues_dir.exists());
    assert!(issues_dir.join("old.md").exists());
    assert!(new_issues_dir.join("new.md").exists());

    // Verify: Storage uses the new directory
    let issues = storage.list_issues().await.unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].name, "new");
}

/// Test CLI integration by simulating MCP tool context creation
#[tokio::test]
async fn test_cli_integration_migration() {
    let temp_dir = TempDir::new().unwrap();

    // Setup: Create old issues directory
    let issues_dir = temp_dir.path().join("issues");
    std::fs::create_dir_all(&issues_dir).unwrap();
    std::fs::write(issues_dir.join("cli_test.md"), "CLI test issue").unwrap();

    // Simulate CLI storage creation (similar to what happens in mcp_integration.rs)
    let (storage, migration_result) =
        FileSystemIssueStorage::new_default_in(temp_dir.path()).unwrap();

    // Verify: Migration occurred
    assert!(migration_result.is_some());
    match migration_result.unwrap() {
        MigrationResult::Success(stats) => {
            assert_eq!(stats.files_moved, 1);
            // This would trigger the CLI message: "✅ Migrated 1 issues to .swissarmyhammer/issues"
        }
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
}

/// Test MCP server integration scenario
#[tokio::test]
async fn test_mcp_server_integration_migration() {
    let temp_dir = TempDir::new().unwrap();

    // Setup: Create old issues directory with nested structure
    let issues_dir = temp_dir.path().join("issues");
    let complete_dir = issues_dir.join("complete");
    std::fs::create_dir_all(&complete_dir).unwrap();

    std::fs::write(issues_dir.join("active.md"), "Active issue").unwrap();
    std::fs::write(complete_dir.join("completed.md"), "Completed issue").unwrap();

    // Simulate MCP server storage creation
    let (storage, migration_result) =
        FileSystemIssueStorage::new_default_in(temp_dir.path()).unwrap();

    // Verify: Migration occurred with complete directory structure
    assert!(migration_result.is_some());
    match migration_result.unwrap() {
        MigrationResult::Success(stats) => {
            assert_eq!(stats.files_moved, 2); // Both active and completed files
                                              // This would trigger MCP server logging: "MCP server performed automatic migration: 2 files moved..."
        }
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
}
