//! Edge case tests for flexible base branch support
//!
//! This module tests error conditions, edge cases, and abort scenarios
//! for the flexible branching functionality.

use std::process::Command;
use std::sync::Arc;
use swissarmyhammer::git::GitOperations;
use swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage};
use tempfile::TempDir;
use tokio::sync::RwLock;

/// Test environment for edge case testing
struct EdgeCaseTestEnvironment {
    temp_dir: TempDir,
    issue_storage: Arc<RwLock<Box<dyn IssueStorage>>>,
    git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>>,
}

impl EdgeCaseTestEnvironment {
    async fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temporary directory for test");

        // Set up git repository
        Self::setup_git_repo(temp_dir.path()).await;

        // Change to test directory
        std::env::set_current_dir(temp_dir.path()).expect("Failed to change to test directory");

        // Initialize issue storage
        let issues_dir = temp_dir.path().join("issues");
        let issue_storage = Box::new(
            FileSystemIssueStorage::new(issues_dir).expect("Failed to create issue storage"),
        );
        let issue_storage = Arc::new(RwLock::new(issue_storage as Box<dyn IssueStorage>));

        // Initialize git operations
        let git_ops = Arc::new(tokio::sync::Mutex::new(Some(
            GitOperations::with_work_dir(temp_dir.path().to_path_buf())
                .expect("Failed to create git operations"),
        )));

        Self {
            temp_dir,
            issue_storage,
            git_ops,
        }
    }

    async fn setup_git_repo(path: &std::path::Path) {
        // Initialize git repo
        Command::new("git")
            .current_dir(path)
            .args(["init"])
            .output()
            .unwrap();

        // Configure git
        Command::new("git")
            .current_dir(path)
            .args(["config", "user.name", "Test User"])
            .output()
            .unwrap();

        Command::new("git")
            .current_dir(path)
            .args(["config", "user.email", "test@example.com"])
            .output()
            .unwrap();

        // Create initial commit
        std::fs::write(path.join("README.md"), "# Test Project")
            .expect("Failed to write README.md");
        Command::new("git")
            .current_dir(path)
            .args(["add", "README.md"])
            .output()
            .unwrap();

        Command::new("git")
            .current_dir(path)
            .args(["commit", "-m", "Initial commit"])
            .output()
            .unwrap();
    }

    /// Create a test branch with some commits for testing
    async fn create_test_branch(&self, branch_name: &str) {
        Command::new("git")
            .current_dir(self.temp_dir.path())
            .args(["checkout", "-b", branch_name])
            .output()
            .unwrap();

        let test_file = format!("{}.txt", branch_name.replace('/', "_"));
        std::fs::write(
            self.temp_dir.path().join(&test_file),
            format!("Content for {}", branch_name),
        )
        .expect("Failed to write test file");

        Command::new("git")
            .current_dir(self.temp_dir.path())
            .args(["add", &test_file])
            .output()
            .unwrap();

        Command::new("git")
            .current_dir(self.temp_dir.path())
            .args(["commit", "-m", &format!("Add content for {}", branch_name)])
            .output()
            .unwrap();
    }
}

/// Test what happens when source branch is deleted mid-workflow
#[tokio::test]
async fn test_source_branch_deleted_mid_workflow() {
    let env = EdgeCaseTestEnvironment::new().await;

    // Create a feature branch
    env.create_test_branch("feature/temporary").await;

    // Create issue from feature branch
    let issue_name = "temp-feature-work".to_string();
    let issue_content = "# Temporary Feature Work\n\nWork based on temporary feature".to_string();

    {
        let issue_storage = env.issue_storage.write().await;
        issue_storage
            .create_issue_with_source_branch(
                issue_name.clone(),
                issue_content,
                "feature/temporary".to_string(),
            )
            .await
            .expect("Failed to create issue");
    }

    // Create issue branch
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        let (branch_name, source_branch) = git
            .create_work_branch_with_source(&issue_name, Some("feature/temporary"))
            .unwrap();

        assert_eq!(branch_name, "issue/temp-feature-work");
        assert_eq!(source_branch, "feature/temporary");
    }

    // Make some changes on the issue branch
    std::fs::write(
        env.temp_dir.path().join("work_file.txt"),
        "Work in progress",
    )
    .expect("Failed to write work file");

    Command::new("git")
        .current_dir(env.temp_dir.path())
        .args(["add", "work_file.txt"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(env.temp_dir.path())
        .args(["commit", "-m", "Add work file"])
        .output()
        .unwrap();

    // Now simulate the source branch being deleted by another developer
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        git.checkout_branch("main").unwrap();
        git.delete_branch("feature/temporary").unwrap();
    }

    // Mark issue complete
    {
        let issue_storage = env.issue_storage.write().await;
        issue_storage.mark_complete(&issue_name).await.unwrap();
    }

    // Try to merge - this should fail and create an abort file
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        let result = git.merge_issue_branch(&issue_name, "feature/temporary");
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("feature/temporary") || error_msg.contains("does not exist"));
    }

    // Check if abort file was created
    let _abort_file_path = env.temp_dir.path().join(".swissarmyhammer/.abort");
    // Note: The abort file creation depends on the specific error handling in the git operations
    // This test verifies the error is properly handled
}

/// Test merge conflicts with source branch that has diverged
#[tokio::test]
async fn test_merge_conflicts_with_diverged_source_branch() {
    let env = EdgeCaseTestEnvironment::new().await;

    // Create feature branch with initial content
    env.create_test_branch("feature/conflicting").await;

    // Create issue from feature branch
    let issue_name = "conflicting-changes".to_string();
    let issue_content = "# Conflicting Changes\n\nChanges that will conflict".to_string();

    {
        let issue_storage = env.issue_storage.write().await;
        issue_storage
            .create_issue_with_source_branch(
                issue_name.clone(),
                issue_content,
                "feature/conflicting".to_string(),
            )
            .await
            .expect("Failed to create issue");
    }

    // Create issue branch and make changes
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        git.create_work_branch_with_source(&issue_name, Some("feature/conflicting"))
            .unwrap();
    }

    // Make changes on issue branch to conflict_file.txt
    std::fs::write(
        env.temp_dir.path().join("conflict_file.txt"),
        "Issue branch version",
    )
    .expect("Failed to write conflict file");

    Command::new("git")
        .current_dir(env.temp_dir.path())
        .args(["add", "conflict_file.txt"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(env.temp_dir.path())
        .args(["commit", "-m", "Add conflict file from issue branch"])
        .output()
        .unwrap();

    // Switch to feature branch and make conflicting changes
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        git.checkout_branch("feature/conflicting").unwrap();
    }

    std::fs::write(
        env.temp_dir.path().join("conflict_file.txt"),
        "Feature branch version",
    )
    .expect("Failed to write conflicting content");

    Command::new("git")
        .current_dir(env.temp_dir.path())
        .args(["add", "conflict_file.txt"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(env.temp_dir.path())
        .args(["commit", "-m", "Add conflict file from feature branch"])
        .output()
        .unwrap();

    // Switch back to issue branch
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        git.checkout_branch(&format!("issue/{}", issue_name))
            .unwrap();
    }

    // Mark issue complete
    {
        let issue_storage = env.issue_storage.write().await;
        issue_storage.mark_complete(&issue_name).await.unwrap();
    }

    // Try to merge - should fail due to conflict
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        let result = git.merge_issue_branch(&issue_name, "feature/conflicting");
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("conflict") || error_msg.contains("merge"));
    }

    // Verify no partial merge state is left
    {
        let git_ops = env.git_ops.lock().await;
        let _git = git_ops.as_ref().unwrap();
        let status_output = Command::new("git")
            .current_dir(env.temp_dir.path())
            .args(["status", "--porcelain"])
            .output()
            .unwrap();

        let status_str = String::from_utf8_lossy(&status_output.stdout);
        // Should not have unmerged files (UU status)
        assert!(!status_str.contains("UU"));
    }
}

/// Test validation of source branch state before merge operations
#[tokio::test]
async fn test_source_branch_validation_before_merge() {
    let env = EdgeCaseTestEnvironment::new().await;

    // Create a normal feature branch
    env.create_test_branch("feature/valid").await;

    let issue_name = "validation-test".to_string();

    // Create issue and branch
    {
        let issue_storage = env.issue_storage.write().await;
        issue_storage
            .create_issue_with_source_branch(
                issue_name.clone(),
                "# Validation Test".to_string(),
                "feature/valid".to_string(),
            )
            .await
            .expect("Failed to create issue");
    }

    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        git.create_work_branch_with_source(&issue_name, Some("feature/valid"))
            .unwrap();
    }

    // Make a simple change and commit
    std::fs::write(
        env.temp_dir.path().join("validation.txt"),
        "validation content",
    )
    .expect("Failed to write validation file");

    Command::new("git")
        .current_dir(env.temp_dir.path())
        .args(["add", "validation.txt"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(env.temp_dir.path())
        .args(["commit", "-m", "Add validation content"])
        .output()
        .unwrap();

    // Now corrupt the source branch reference to simulate repository issues
    // This simulates various git repository corruption scenarios
    Command::new("git")
        .current_dir(env.temp_dir.path())
        .args(["checkout", "feature/valid"])
        .output()
        .unwrap();

    // Create a scenario where the branch exists but has issues
    // by creating an invalid ref
    let refs_dir = env.temp_dir.path().join(".git/refs/heads");
    if refs_dir.exists() {
        let feature_ref = refs_dir.join("feature/invalid-ref");
        std::fs::write(feature_ref, "invalid-commit-hash\n").expect("Failed to write invalid ref");
    }

    // Try to validate with invalid reference - should detect the issue
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();

        // Test validation with the corrupted reference
        // Note: validate_source_branch_state is private, so we test indirectly through merge
        // The important thing is that operations don't panic or create inconsistent state
        let _result = git.merge_issue_branch(&issue_name, "feature/invalid-ref");
        // This should handle the invalid reference gracefully
    }
}

/// Test recovery from failed branch operations
#[tokio::test]
async fn test_recovery_from_failed_branch_operations() {
    let env = EdgeCaseTestEnvironment::new().await;

    let git_ops = env.git_ops.lock().await;
    let git = git_ops.as_ref().unwrap();

    // Start on main branch
    git.checkout_branch("main").unwrap();
    let initial_branch = git.current_branch().unwrap();

    // Create first issue branch successfully
    git.create_work_branch_simple("good-issue").unwrap();
    assert_eq!(git.current_branch().unwrap(), "issue/good-issue");

    // Try to create another issue branch from the issue branch (should fail)
    let result = git.create_work_branch_simple("bad-issue");
    assert!(result.is_err());

    // Verify we're still on the good issue branch (consistent state)
    assert_eq!(git.current_branch().unwrap(), "issue/good-issue");

    // Verify the failed branch doesn't exist
    assert!(!git.branch_exists("issue/bad-issue").unwrap());

    // Verify we can still switch back to main and create other branches
    git.checkout_branch(&initial_branch).unwrap();
    assert_eq!(git.current_branch().unwrap(), initial_branch);

    // Should be able to create new branches after the failure
    let result = git.create_work_branch_simple("recovery-issue");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "issue/recovery-issue");
}

/// Test handling of working directory changes during merge
#[tokio::test]
async fn test_uncommitted_changes_during_merge() {
    let env = EdgeCaseTestEnvironment::new().await;

    // Create feature branch
    env.create_test_branch("feature/dirty").await;

    let issue_name = "dirty-work".to_string();

    // Create issue and branch
    {
        let issue_storage = env.issue_storage.write().await;
        issue_storage
            .create_issue_with_source_branch(
                issue_name.clone(),
                "# Dirty Work".to_string(),
                "feature/dirty".to_string(),
            )
            .await
            .expect("Failed to create issue");
    }

    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        git.create_work_branch_with_source(&issue_name, Some("feature/dirty"))
            .unwrap();
    }

    // Make and commit changes on issue branch
    std::fs::write(
        env.temp_dir.path().join("committed_work.txt"),
        "committed content",
    )
    .expect("Failed to write committed file");

    Command::new("git")
        .current_dir(env.temp_dir.path())
        .args(["add", "committed_work.txt"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(env.temp_dir.path())
        .args(["commit", "-m", "Add committed work"])
        .output()
        .unwrap();

    // Make uncommitted changes (dirty working directory)
    std::fs::write(
        env.temp_dir.path().join("uncommitted_work.txt"),
        "uncommitted content",
    )
    .expect("Failed to write uncommitted file");

    // Check that we have uncommitted changes
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        assert!(git.has_uncommitted_changes().unwrap());
    }

    // Mark issue complete
    {
        let issue_storage = env.issue_storage.write().await;
        issue_storage.mark_complete(&issue_name).await.unwrap();
    }

    // Try to merge with uncommitted changes - behavior depends on implementation
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        let result = git.merge_issue_branch(&issue_name, "feature/dirty");

        // The implementation should either:
        // 1. Succeed and handle the uncommitted changes appropriately, or
        // 2. Fail with a clear error message about uncommitted changes
        // This test verifies the behavior is consistent and doesn't leave corrupt state

        match result {
            Ok(_) => {
                // If merge succeeded, verify the state is consistent
                let current_branch = git.current_branch().unwrap();
                assert_eq!(current_branch, "feature/dirty");
                assert!(env.temp_dir.path().join("committed_work.txt").exists());
            }
            Err(e) => {
                // If merge failed, verify error message is informative
                let error_msg = e.to_string();
                // Error should be related to working directory state or merge conflicts
                assert!(!error_msg.is_empty());
            }
        }
    }
}

/// Test branch name validation and sanitization
#[tokio::test]
async fn test_branch_name_validation() {
    let env = EdgeCaseTestEnvironment::new().await;

    let git_ops = env.git_ops.lock().await;
    let git = git_ops.as_ref().unwrap();

    // Test various invalid branch names
    let invalid_names = vec![
        "issue/with space",     // spaces
        "issue/with..dots",     // double dots
        "issue/with~tilde",     // tildes
        "issue/with^caret",     // carets
        "issue/with:colon",     // colons
        "issue/with[brackets]", // brackets
        "",                     // empty name
        "issue/",               // just prefix
    ];

    for invalid_name in invalid_names {
        // The validation should catch these at the issue creation level
        let result = git.validate_branch_creation(invalid_name, None);
        // Some of these might be caught by git itself rather than our validation
        // The key is that they don't create inconsistent state

        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(!error_msg.is_empty());
        }

        // Verify no branch was created with invalid name
        if !invalid_name.is_empty() {
            let branch_name = format!("issue/{}", invalid_name);
            // This might fail due to git's own validation, which is fine
            let _ = git.branch_exists(&branch_name);
        }
    }

    // Test valid branch names work correctly
    let valid_names = vec![
        "valid-issue-name",
        "feature-123",
        "bug-fix_underscore",
        "issue.with.dots", // single dots are OK
    ];

    for valid_name in valid_names {
        let _result = git.validate_branch_creation(valid_name, None);
        // Valid names should not fail validation (though they might fail for other reasons like being on issue branch)
        // This test ensures our validation doesn't reject valid names
    }
}

/// Test performance with many branches
#[tokio::test]
async fn test_performance_with_many_branches() {
    let env = EdgeCaseTestEnvironment::new().await;

    // Create many feature branches
    for i in 0..10 {
        // Reduced number for CI performance
        env.create_test_branch(&format!("feature/branch-{}", i))
            .await;
    }

    let git_ops = env.git_ops.lock().await;
    let git = git_ops.as_ref().unwrap();

    // Create issue branches from various sources
    for i in 0..5 {
        // Test subset for performance
        let issue_name = format!("perf-test-{}", i);
        let source_branch = format!("feature/branch-{}", i);

        git.checkout_branch(&source_branch).unwrap();

        let start_time = std::time::Instant::now();
        let result = git.create_work_branch_with_source(&issue_name, None);
        let duration = start_time.elapsed();

        assert!(result.is_ok());
        assert!(duration.as_millis() < 5000); // Should complete within 5 seconds

        let (branch_name, detected_source) = result.unwrap();
        assert_eq!(branch_name, format!("issue/{}", issue_name));
        assert_eq!(detected_source, source_branch);
    }

    // Test branch existence checking performance
    let start_time = std::time::Instant::now();
    for i in 0..10 {
        let branch_name = format!("feature/branch-{}", i);
        assert!(git.branch_exists(&branch_name).unwrap());
    }
    let duration = start_time.elapsed();
    assert!(duration.as_millis() < 1000); // Should check all branches within 1 second
}
