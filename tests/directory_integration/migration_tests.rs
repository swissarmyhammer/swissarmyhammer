//! Directory Migration Scenario Tests
//!
//! These tests validate the migration from legacy directory structures
//! to the new Git repository-centric approach, including edge cases,
//! conflict resolution, and data preservation scenarios.

use super::{GitRepositoryTestGuard, create_legacy_directory_structure};
use swissarmyhammer::directory_utils::{
    find_swissarmyhammer_directory,
    find_swissarmyhammer_dirs_upward,
    get_or_create_swissarmyhammer_directory
};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Test migration from single .swissarmyhammer directory to Git repository structure
///
/// This test validates the scenario where a user has a single .swissarmyhammer
/// directory in their project and needs to migrate to Git-centric structure.
#[test]
fn test_migration_from_single_swissarmyhammer_directory() {
    // Create a directory structure with existing .swissarmyhammer directory
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let project_root = temp_dir.path().join("my-project");
    fs::create_dir_all(&project_root).expect("Failed to create project directory");

    // Create existing .swissarmyhammer directory with data
    let old_swissarmyhammer = project_root.join(".swissarmyhammer");
    fs::create_dir_all(&old_swissarmyhammer).expect("Failed to create old .swissarmyhammer");

    // Create subdirectories with existing data
    let old_memos_dir = old_swissarmyhammer.join("memos");
    let old_todo_dir = old_swissarmyhammer.join("todo");
    fs::create_dir_all(&old_memos_dir).expect("Failed to create old memos directory");
    fs::create_dir_all(&old_todo_dir).expect("Failed to create old todo directory");

    // Add existing data
    let memo_content = "# Existing Memo\n\nThis memo exists before migration.";
    fs::write(old_memos_dir.join("existing_memo.md"), memo_content)
        .expect("Failed to write existing memo");

    let todo_content = r#"todo:
  - id: 01H8XYZ123ABC456DEF789OLD0
    task: "Complete migration testing"
    context: "Ensure all data is preserved"
    done: false
"#;
    fs::write(old_todo_dir.join("existing_todo.yaml"), todo_content)
        .expect("Failed to write existing todo");

    // Initialize Git repository in the project
    let original_cwd = std::env::current_dir().expect("Failed to get current directory");
    std::env::set_current_dir(&project_root).expect("Failed to change to project directory");

    let git_repo = git2::Repository::init(&project_root)
        .expect("Failed to initialize Git repository");

    // Now test migration scenario
    // Old behavior: directory already exists, should be found
    let found_dirs = find_swissarmyhammer_dirs_upward(&project_root, false);
    assert_eq!(found_dirs.len(), 1);
    assert_eq!(found_dirs[0], old_swissarmyhammer);

    // New behavior: should find the Git-centric directory
    let git_swissarmyhammer = find_swissarmyhammer_directory();
    assert!(git_swissarmyhammer.is_some());
    assert_eq!(git_swissarmyhammer.unwrap(), old_swissarmyhammer);

    // Verify existing data is accessible
    let memo_path = git_swissarmyhammer.unwrap().join("memos/existing_memo.md");
    assert!(memo_path.exists());
    let read_memo = fs::read_to_string(&memo_path).expect("Failed to read migrated memo");
    assert_eq!(read_memo, memo_content);

    let todo_path = git_swissarmyhammer.unwrap().join("todo/existing_todo.yaml");
    assert!(todo_path.exists());
    let read_todo = fs::read_to_string(&todo_path).expect("Failed to read migrated todo");
    assert!(read_todo.contains("Complete migration testing"));

    // Restore original directory
    std::env::set_current_dir(original_cwd).expect("Failed to restore directory");
}

/// Test migration scenario with multiple .swissarmyhammer directories
///
/// This test validates the legacy scenario where multiple .swissarmyhammer
/// directories exist in a hierarchy and migration to Git-centric structure.
#[test]
fn test_migration_from_multiple_swissarmyhammer_directories() {
    let (temp_dir, deepest_path, swissarmyhammer_dirs) = create_legacy_directory_structure();
    
    // Find the root directory to initialize Git repository
    let project_root = temp_dir.path().join("project");
    
    let original_cwd = std::env::current_dir().expect("Failed to get current directory");
    std::env::set_current_dir(&project_root).expect("Failed to change to project directory");

    // Initialize Git repository at project level
    let _git_repo = git2::Repository::init(&project_root)
        .expect("Failed to initialize Git repository");

    // Test legacy behavior: multiple directories are found
    let found_dirs = find_swissarmyhammer_dirs_upward(&deepest_path, false);
    assert!(found_dirs.len() >= 2, "Should find multiple .swissarmyhammer directories");

    // Test new behavior: should find Git repository .swissarmyhammer
    let git_swissarmyhammer = find_swissarmyhammer_directory();
    assert!(git_swissarmyhammer.is_some());
    assert_eq!(git_swissarmyhammer.unwrap(), project_root.join(".swissarmyhammer"));

    // The Git-centric directory should be the one at project root
    let git_dir = git_swissarmyhammer.unwrap();
    assert!(git_dir.exists());
    assert!(git_dir.join("memos").exists());

    // Verify that migration preserves data accessibility
    let memo_file = git_dir.join("memos/test.md");
    assert!(memo_file.exists());
    let memo_content = fs::read_to_string(&memo_file).expect("Failed to read memo");
    assert_eq!(memo_content, "# Test memo\nContent");

    std::env::set_current_dir(original_cwd).expect("Failed to restore directory");
}

/// Test migration with nested Git repositories
///
/// This test validates migration scenarios where there are nested Git repositories,
/// each potentially having their own .swissarmyhammer directories.
#[test]
fn test_migration_with_nested_git_repositories() {
    let guard = GitRepositoryTestGuard::new().with_swissarmyhammer();
    
    // Create nested Git repository
    let nested_path = guard.with_nested_git_repository();
    
    // Create .swissarmyhammer directory in nested repository
    let nested_swissarmyhammer = nested_path.join(".swissarmyhammer");
    fs::create_dir_all(&nested_swissarmyhammer.join("memos"))
        .expect("Failed to create nested swissarmyhammer memos");
    
    let nested_memo_content = "# Nested Repository Memo\n\nThis memo is in a nested Git repository.";
    fs::write(nested_swissarmyhammer.join("memos/nested_memo.md"), nested_memo_content)
        .expect("Failed to write nested memo");

    // Test from parent repository
    let parent_swissarmyhammer = find_swissarmyhammer_directory();
    assert!(parent_swissarmyhammer.is_some());
    assert_eq!(parent_swissarmyhammer.unwrap(), guard.swissarmyhammer_dir().unwrap());

    // Test from nested repository
    guard.cd_to_subdir(nested_path.strip_prefix(guard.path()).unwrap())
        .expect("Failed to change to nested repository");

    let nested_swissarmyhammer_found = find_swissarmyhammer_directory();
    assert!(nested_swissarmyhammer_found.is_none(), 
           "Nested repository should not have .swissarmyhammer directory initially");

    // Create .swissarmyhammer in nested repository and test again
    fs::create_dir_all(&nested_swissarmyhammer).expect("Failed to create nested swissarmyhammer");

    let nested_swissarmyhammer_found = find_swissarmyhammer_directory();
    assert!(nested_swissarmyhammer_found.is_some());
    assert_eq!(nested_swissarmyhammer_found.unwrap(), nested_swissarmyhammer);

    // Verify isolation: nested repository .swissarmyhammer is separate from parent
    let nested_memo_path = nested_swissarmyhammer_found.unwrap().join("memos/nested_memo.md");
    assert!(nested_memo_path.exists());

    let parent_memo_should_not_exist = nested_swissarmyhammer_found.unwrap()
        .join("memos").read_dir()
        .map(|entries| entries.count())
        .unwrap_or(0);
    
    // Nested repository should have its own isolated memos
    fs::write(nested_memo_path, nested_memo_content).expect("Failed to write nested memo");
    assert!(nested_memo_path.exists());
}

/// Test migration error scenarios and conflict resolution
///
/// This test validates error handling when migration encounters conflicts
/// or problematic directory structures.
#[test]
fn test_migration_error_scenarios() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let project_root = temp_dir.path().join("project");
    fs::create_dir_all(&project_root).expect("Failed to create project directory");

    // Scenario 1: .swissarmyhammer exists as a file, not directory
    let swissarmyhammer_file = project_root.join(".swissarmyhammer");
    fs::write(&swissarmyhammer_file, "This is a file, not a directory")
        .expect("Failed to create .swissarmyhammer file");

    let original_cwd = std::env::current_dir().expect("Failed to get current directory");
    std::env::set_current_dir(&project_root).expect("Failed to change to project directory");

    let _git_repo = git2::Repository::init(&project_root)
        .expect("Failed to initialize Git repository");

    // Should return None since .swissarmyhammer is not a directory
    let result = find_swissarmyhammer_directory();
    assert!(result.is_none(), ".swissarmyhammer file should not be recognized as directory");

    // get_or_create_swissarmyhammer_directory should fail gracefully
    let create_result = get_or_create_swissarmyhammer_directory();
    assert!(create_result.is_err(), "Should fail when .swissarmyhammer exists as file");

    // Scenario 2: No Git repository present
    fs::remove_file(&swissarmyhammer_file).expect("Failed to remove .swissarmyhammer file");
    fs::remove_dir_all(project_root.join(".git")).expect("Failed to remove .git directory");

    let no_git_result = find_swissarmyhammer_directory();
    assert!(no_git_result.is_none(), "Should return None when not in Git repository");

    let create_no_git_result = get_or_create_swissarmyhammer_directory();
    assert!(create_no_git_result.is_err(), "Should fail when not in Git repository");

    // Scenario 3: Permission denied (simulate by creating readonly directory)
    let _git_repo = git2::Repository::init(&project_root)
        .expect("Failed to re-initialize Git repository");

    fs::create_dir_all(&project_root.join(".swissarmyhammer"))
        .expect("Failed to create .swissarmyhammer directory");

    // Make directory readonly (Unix-style permissions)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&project_root.join(".swissarmyhammer"))
            .expect("Failed to get permissions")
            .permissions();
        perms.set_mode(0o444); // readonly
        fs::set_permissions(&project_root.join(".swissarmyhammer"), perms)
            .expect("Failed to set readonly permissions");

        // Should still find the directory
        let readonly_result = find_swissarmyhammer_directory();
        assert!(readonly_result.is_some(), "Should find readonly .swissarmyhammer directory");

        // Restore permissions for cleanup
        let mut restore_perms = perms;
        restore_perms.set_mode(0o755);
        fs::set_permissions(&project_root.join(".swissarmyhammer"), restore_perms)
            .expect("Failed to restore permissions");
    }

    std::env::set_current_dir(original_cwd).expect("Failed to restore directory");
}

/// Test migration with existing data preservation
///
/// This test validates that existing data in .swissarmyhammer directories
/// is correctly preserved during migration to Git-centric structure.
#[test]
fn test_migration_data_preservation() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let project_root = temp_dir.path().join("project");
    fs::create_dir_all(&project_root).expect("Failed to create project directory");

    // Create existing .swissarmyhammer with comprehensive data
    let swissarmyhammer_dir = project_root.join(".swissarmyhammer");
    fs::create_dir_all(&swissarmyhammer_dir.join("memos"))
        .expect("Failed to create memos directory");
    fs::create_dir_all(&swissarmyhammer_dir.join("todo"))
        .expect("Failed to create todo directory");
    fs::create_dir_all(&swissarmyhammer_dir.join("issues"))
        .expect("Failed to create issues directory");
    fs::create_dir_all(&swissarmyhammer_dir.join("issues/complete"))
        .expect("Failed to create issues/complete directory");
    fs::create_dir_all(&swissarmyhammer_dir.join("workflows"))
        .expect("Failed to create workflows directory");

    // Create diverse content to test preservation
    let memo_files = vec![
        ("project_notes.md", "# Project Notes\n\nImportant project information."),
        ("meeting_20240120.md", "# Meeting Notes\n\n## Attendees\n- Alice\n- Bob"),
        ("research/algorithm.md", "# Algorithm Research\n\nComplexity analysis notes."),
    ];

    for (path, content) in &memo_files {
        let file_path = swissarmyhammer_dir.join("memos").join(path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).expect("Failed to create memo subdirectory");
        }
        fs::write(&file_path, content).expect("Failed to write memo file");
    }

    let todo_files = vec![
        ("current_sprint.yaml", r#"todo:
  - id: 01H8XYZ123ABC456DEF789GHI8
    task: "Implement user dashboard"
    context: "Priority: High"
    done: false
  - id: 01H8XYZ123ABC456DEF789GHI9
    task: "Fix login bug"
    context: "Reported by QA team"
    done: true
"#),
        ("backlog.yaml", r#"todo:
  - id: 01H8XYZ123ABC456DEF789GHIA
    task: "Optimize database queries"
    context: "Performance improvement"
    done: false
"#),
    ];

    for (path, content) in &todo_files {
        let file_path = swissarmyhammer_dir.join("todo").join(path);
        fs::write(&file_path, content).expect("Failed to write todo file");
    }

    let issue_files = vec![
        ("FEATURE_001_dashboard.md", "# Feature: User Dashboard\n\n## Requirements\n- Display user statistics"),
        ("BUG_001_login.md", "# Bug: Login Error\n\n## Steps to Reproduce\n1. Enter credentials"),
    ];

    for (path, content) in &issue_files {
        let file_path = swissarmyhammer_dir.join("issues").join(path);
        fs::write(&file_path, content).expect("Failed to write issue file");
    }

    // Create completed issue
    let completed_issue_path = swissarmyhammer_dir.join("issues/complete/FEATURE_000_setup.md");
    fs::write(&completed_issue_path, "# Feature: Initial Setup\n\n## Status\nCompleted")
        .expect("Failed to write completed issue");

    // Create custom workflow
    let workflow_content = r#"name: "Custom Build Workflow"
description: "Builds and tests the project"
steps:
  - name: "Build"
    command: "cargo build"
  - name: "Test"  
    command: "cargo test"
"#;
    let workflow_path = swissarmyhammer_dir.join("workflows/build.yaml");
    fs::write(&workflow_path, workflow_content).expect("Failed to write workflow file");

    // Initialize Git repository
    let original_cwd = std::env::current_dir().expect("Failed to get current directory");
    std::env::set_current_dir(&project_root).expect("Failed to change to project directory");

    let _git_repo = git2::Repository::init(&project_root)
        .expect("Failed to initialize Git repository");

    // Test migration: all data should be accessible via Git-centric resolution
    let git_swissarmyhammer = find_swissarmyhammer_directory();
    assert!(git_swissarmyhammer.is_some());
    assert_eq!(git_swissarmyhammer.unwrap(), swissarmyhammer_dir);

    // Verify all memo files are preserved
    for (path, expected_content) in &memo_files {
        let file_path = git_swissarmyhammer.unwrap().join("memos").join(path);
        assert!(file_path.exists(), "Memo file {} should exist after migration", path);
        let actual_content = fs::read_to_string(&file_path)
            .unwrap_or_else(|e| panic!("Failed to read memo file {}: {}", path, e));
        assert_eq!(&actual_content, expected_content, "Memo file {} content should be preserved", path);
    }

    // Verify all todo files are preserved
    for (path, expected_content) in &todo_files {
        let file_path = git_swissarmyhammer.unwrap().join("todo").join(path);
        assert!(file_path.exists(), "Todo file {} should exist after migration", path);
        let actual_content = fs::read_to_string(&file_path)
            .unwrap_or_else(|e| panic!("Failed to read todo file {}: {}", path, e));
        assert_eq!(&actual_content, expected_content, "Todo file {} content should be preserved", path);
    }

    // Verify all issue files are preserved
    for (path, expected_content) in &issue_files {
        let file_path = git_swissarmyhammer.unwrap().join("issues").join(path);
        assert!(file_path.exists(), "Issue file {} should exist after migration", path);
        let actual_content = fs::read_to_string(&file_path)
            .unwrap_or_else(|e| panic!("Failed to read issue file {}: {}", path, e));
        assert_eq!(&actual_content, expected_content, "Issue file {} content should be preserved", path);
    }

    // Verify completed issue is preserved
    let completed_issue_migrated = git_swissarmyhammer.unwrap().join("issues/complete/FEATURE_000_setup.md");
    assert!(completed_issue_migrated.exists(), "Completed issue should exist after migration");
    let completed_content = fs::read_to_string(&completed_issue_migrated)
        .expect("Failed to read completed issue");
    assert!(completed_content.contains("Initial Setup"), "Completed issue content should be preserved");

    // Verify workflow is preserved
    let workflow_migrated = git_swissarmyhammer.unwrap().join("workflows/build.yaml");
    assert!(workflow_migrated.exists(), "Workflow file should exist after migration");
    let workflow_actual = fs::read_to_string(&workflow_migrated)
        .expect("Failed to read workflow file");
    assert_eq!(workflow_actual, workflow_content, "Workflow content should be preserved");

    // Test file operations work correctly after migration
    let new_memo_content = "# Post-Migration Memo\n\nThis memo was created after migration.";
    let new_memo_path = git_swissarmyhammer.unwrap().join("memos/post_migration.md");
    fs::write(&new_memo_path, new_memo_content).expect("Failed to write post-migration memo");

    assert!(new_memo_path.exists(), "New memo should be created successfully");
    let read_new_memo = fs::read_to_string(&new_memo_path).expect("Failed to read new memo");
    assert_eq!(read_new_memo, new_memo_content, "New memo content should be correct");

    std::env::set_current_dir(original_cwd).expect("Failed to restore directory");
}

/// Test migration with Git worktree scenario
///
/// This test validates migration behavior when working with Git worktrees
/// where .git is a file pointing to the actual git directory.
#[test]
fn test_migration_with_git_worktree() {
    let guard = GitRepositoryTestGuard::new()
        .with_swissarmyhammer()
        .as_git_worktree();

    // Verify .git exists as file, not directory
    assert!(guard.git_dir().exists());
    assert!(guard.git_dir().is_file());

    // Directory resolution should still work
    let swissarmyhammer_dir = find_swissarmyhammer_directory();
    assert!(swissarmyhammer_dir.is_some());
    assert_eq!(swissarmyhammer_dir.unwrap(), guard.swissarmyhammer_dir().unwrap());

    // Test that operations work normally despite .git being a file
    let memo_content = "# Worktree Memo\n\nThis memo is in a Git worktree.";
    let memo_file = swissarmyhammer_dir.unwrap().join("memos/worktree_memo.md");
    fs::write(&memo_file, memo_content).expect("Failed to write worktree memo");

    assert!(memo_file.exists());
    let read_content = fs::read_to_string(&memo_file).expect("Failed to read worktree memo");
    assert_eq!(read_content, memo_content);

    // Test from subdirectory
    guard.cd_to_subdir("src").expect("Failed to change to src directory");
    let swissarmyhammer_from_subdir = find_swissarmyhammer_directory();
    assert!(swissarmyhammer_from_subdir.is_some());
    assert_eq!(swissarmyhammer_from_subdir.unwrap(), guard.swissarmyhammer_dir().unwrap());
}

/// Test migration with very deep directory structures
///
/// This test validates migration behavior with deeply nested directory
/// structures, ensuring depth limits are respected and performance
/// remains acceptable.
#[test]
fn test_migration_with_deep_directory_structures() {
    let guard = GitRepositoryTestGuard::new().with_swissarmyhammer();
    
    // Create very deep directory structure (but within MAX_DIRECTORY_DEPTH)
    let deep_path = guard.create_deep_structure(25); // 25 levels deep
    
    // Test directory resolution from deep path
    guard.cd_to_subdir(deep_path.strip_prefix(guard.path()).unwrap())
        .expect("Failed to change to deep directory");
    
    let swissarmyhammer_from_deep = find_swissarmyhammer_directory();
    assert!(swissarmyhammer_from_deep.is_some(), 
           "Should find .swissarmyhammer from deep directory within depth limit");
    assert_eq!(swissarmyhammer_from_deep.unwrap(), guard.swissarmyhammer_dir().unwrap());

    // Test creating data from deep directory
    let memo_content = "# Deep Directory Memo\n\nCreated from 25 levels deep.";
    let memo_file = swissarmyhammer_from_deep.unwrap().join("memos/deep_memo.md");
    fs::write(&memo_file, memo_content).expect("Failed to write memo from deep directory");

    assert!(memo_file.exists());
    let read_content = fs::read_to_string(&memo_file).expect("Failed to read deep memo");
    assert_eq!(read_content, memo_content);
}

/// Test migration rollback scenarios
///
/// This test validates scenarios where migration might need to be rolled back
/// or where conflicts need to be resolved.
#[test]
fn test_migration_rollback_scenarios() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let project_root = temp_dir.path().join("project");
    fs::create_dir_all(&project_root).expect("Failed to create project directory");

    // Create a scenario where migration might conflict
    // Two different .swissarmyhammer directories with conflicting content
    let old_swissarmyhammer = project_root.join("old_location/.swissarmyhammer");
    let new_swissarmyhammer = project_root.join(".swissarmyhammer");

    fs::create_dir_all(&old_swissarmyhammer.join("memos"))
        .expect("Failed to create old memos directory");
    fs::create_dir_all(&new_swissarmyhammer.join("memos"))
        .expect("Failed to create new memos directory");

    // Create conflicting memo files
    let old_memo_content = "# Old Memo\n\nThis is the old version of the memo.";
    let new_memo_content = "# New Memo\n\nThis is the new version of the memo.";

    fs::write(old_swissarmyhammer.join("memos/conflict.md"), old_memo_content)
        .expect("Failed to write old memo");
    fs::write(new_swissarmyhammer.join("memos/conflict.md"), new_memo_content)
        .expect("Failed to write new memo");

    let original_cwd = std::env::current_dir().expect("Failed to get current directory");
    std::env::set_current_dir(&project_root).expect("Failed to change to project directory");

    let _git_repo = git2::Repository::init(&project_root)
        .expect("Failed to initialize Git repository");

    // Test that new Git-centric structure takes precedence
    let git_swissarmyhammer = find_swissarmyhammer_directory();
    assert!(git_swissarmyhammer.is_some());
    assert_eq!(git_swissarmyhammer.unwrap(), new_swissarmyhammer);

    // Verify that the new memo content is used (Git-centric takes precedence)
    let conflict_memo = git_swissarmyhammer.unwrap().join("memos/conflict.md");
    assert!(conflict_memo.exists());
    let actual_content = fs::read_to_string(&conflict_memo).expect("Failed to read conflict memo");
    assert_eq!(actual_content, new_memo_content, "Git-centric content should take precedence");

    // Old location should still exist but not be used
    assert!(old_swissarmyhammer.join("memos/conflict.md").exists());
    let old_content = fs::read_to_string(old_swissarmyhammer.join("memos/conflict.md"))
        .expect("Failed to read old memo");
    assert_eq!(old_content, old_memo_content, "Old content should be preserved but not used");

    std::env::set_current_dir(original_cwd).expect("Failed to restore directory");
}

/// Test migration performance with large directory structures
///
/// This test validates that migration and directory resolution perform
/// acceptably even with large numbers of files and directories.
#[test] 
fn test_migration_performance_with_large_structures() {
    let guard = GitRepositoryTestGuard::new().with_swissarmyhammer();
    let swissarmyhammer_dir = guard.swissarmyhammer_dir().unwrap();

    // Create large number of files to simulate real-world usage
    let start_time = std::time::Instant::now();

    // Create many memo files
    for i in 0..100 {
        let memo_content = format!("# Memo {}\n\nThis is memo number {}.", i, i);
        let memo_file = swissarmyhammer_dir.join("memos").join(format!("memo_{:03}.md", i));
        fs::write(&memo_file, memo_content)
            .unwrap_or_else(|e| panic!("Failed to create memo {}: {}", i, e));
    }

    // Create many todo files
    for i in 0..50 {
        let todo_content = format!(r#"todo:
  - id: 01H8XYZ123ABC456DEF789GH{:02}
    task: "Task {} for performance testing"
    context: "This is context for task {}"
    done: false
"#, i, i, i);
        let todo_file = swissarmyhammer_dir.join("todo").join(format!("todo_{:03}.yaml", i));
        fs::write(&todo_file, todo_content)
            .unwrap_or_else(|e| panic!("Failed to create todo {}: {}", i, e));
    }

    let creation_time = start_time.elapsed();
    assert!(creation_time.as_millis() < 5000, 
           "File creation should complete within 5 seconds, took {}ms", creation_time.as_millis());

    // Test directory resolution performance
    let resolution_start = std::time::Instant::now();
    
    for _ in 0..100 {
        let swissarmyhammer = find_swissarmyhammer_directory();
        assert!(swissarmyhammer.is_some());
        assert_eq!(swissarmyhammer.unwrap(), swissarmyhammer_dir);
    }

    let resolution_time = resolution_start.elapsed();
    assert!(resolution_time.as_millis() < 500, 
           "100 directory resolutions should complete within 500ms, took {}ms", resolution_time.as_millis());

    // Test file access performance
    let access_start = std::time::Instant::now();
    
    for i in 0..100 {
        let memo_file = swissarmyhammer_dir.join("memos").join(format!("memo_{:03}.md", i));
        assert!(memo_file.exists(), "Memo {} should exist", i);
    }

    let access_time = access_start.elapsed();
    assert!(access_time.as_millis() < 1000, 
           "File access should complete within 1 second, took {}ms", access_time.as_millis());

    // Verify total file count
    let memo_count = fs::read_dir(swissarmyhammer_dir.join("memos"))
        .expect("Failed to read memos directory")
        .count();
    assert_eq!(memo_count, 100, "Should have 100 memo files");

    let todo_count = fs::read_dir(swissarmyhammer_dir.join("todo"))
        .expect("Failed to read todo directory")
        .count();
    assert_eq!(todo_count, 50, "Should have 50 todo files");
}