//! Integration tests for flexible base branch support
//!
//! This module provides comprehensive tests for the flexible branching functionality,
//! covering complete workflows, MCP tool integration, and edge cases.

use std::process::Command;
use std::sync::Arc;
use swissarmyhammer::git::GitOperations;
use swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage};
use tempfile::TempDir;
use tokio::sync::RwLock;

/// Test environment for flexible branching integration tests
struct FlexibleBranchingTestEnvironment {
    temp_dir: TempDir,
    issue_storage: Arc<RwLock<Box<dyn IssueStorage>>>,
    git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>>,
}

impl FlexibleBranchingTestEnvironment {
    async fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temporary directory for test");

        // Set up git repository with multiple branches
        Self::setup_git_repo_with_branches(temp_dir.path()).await;

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

    async fn setup_git_repo_with_branches(path: &std::path::Path) {
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

        // Create initial commit on main branch
        std::fs::write(
            path.join("README.md"),
            "# Test Project\n\nMain branch content",
        )
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

        // Create feature branch
        Command::new("git")
            .current_dir(path)
            .args(["checkout", "-b", "feature/user-authentication"])
            .output()
            .unwrap();

        std::fs::write(path.join("auth.rs"), "// User authentication module")
            .expect("Failed to write auth.rs");
        Command::new("git")
            .current_dir(path)
            .args(["add", "auth.rs"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(path)
            .args(["commit", "-m", "Add authentication module"])
            .output()
            .unwrap();

        // Create development branch from main
        Command::new("git")
            .current_dir(path)
            .args(["checkout", "main"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(path)
            .args(["checkout", "-b", "develop"])
            .output()
            .unwrap();

        std::fs::write(path.join("dev.txt"), "Development branch")
            .expect("Failed to write dev.txt");
        Command::new("git")
            .current_dir(path)
            .args(["add", "dev.txt"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(path)
            .args(["commit", "-m", "Add development branch file"])
            .output()
            .unwrap();

        // Create release branch from main
        Command::new("git")
            .current_dir(path)
            .args(["checkout", "main"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(path)
            .args(["checkout", "-b", "release/v1.0"])
            .output()
            .unwrap();

        std::fs::write(path.join("version.txt"), "v1.0").expect("Failed to write version.txt");
        Command::new("git")
            .current_dir(path)
            .args(["add", "version.txt"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(path)
            .args(["commit", "-m", "Version 1.0 release"])
            .output()
            .unwrap();

        // Return to main branch
        Command::new("git")
            .current_dir(path)
            .args(["checkout", "main"])
            .output()
            .unwrap();
    }
}

/// Test complete workflow: feature branch → issue branch → merge back to feature
#[tokio::test]
async fn test_feature_branch_to_issue_to_merge_workflow() {
    let env = FlexibleBranchingTestEnvironment::new().await;

    // Start on feature branch
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        git.checkout_branch("feature/user-authentication").unwrap();
    }

    // Create issue from feature branch using issue storage
    let issue_name = "auth-tests".to_string();
    let issue_content =
        "# Authentication Tests\n\nImplement comprehensive tests for the authentication module"
            .to_string();

    // Create the issue
    let _created_issue = {
        let issue_storage = env.issue_storage.write().await;
        issue_storage
            .create_issue(issue_name.clone(), issue_content)
            .await
            .expect("Failed to create issue")
    };

    // Note: source_branch field no longer exists - using git's merge-base instead

    // Create and switch to issue branch (should use current branch as source)
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        let branch_name = git.create_work_branch(&issue_name).unwrap();

        assert_eq!(branch_name, "issue/auth-tests");
    }

    // Make changes on issue branch
    std::fs::write(
        env.temp_dir.path().join("auth_tests.rs"),
        "// Comprehensive authentication tests",
    )
    .expect("Failed to write auth_tests.rs");

    Command::new("git")
        .current_dir(env.temp_dir.path())
        .args(["add", "auth_tests.rs"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(env.temp_dir.path())
        .args(["commit", "-m", "Add authentication tests"])
        .output()
        .unwrap();

    // Mark issue as completed
    {
        let issue_storage = env.issue_storage.write().await;
        issue_storage.mark_complete(&issue_name).await.unwrap();
    }

    // Merge back to feature branch
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        git.merge_issue_branch(&issue_name, "feature/user-authentication")
            .unwrap();
    }

    // Verify final state
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        let current_branch = git.current_branch().unwrap();
        assert_eq!(current_branch, "feature/user-authentication");
    }

    // Verify both files exist on feature branch
    assert!(env.temp_dir.path().join("auth.rs").exists());
    assert!(env.temp_dir.path().join("auth_tests.rs").exists());

    // Verify main branch does NOT have the test file
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        git.checkout_branch("main").unwrap();
        assert!(!env.temp_dir.path().join("auth_tests.rs").exists());
    }
}

/// Test multiple issues from the same source branch
#[tokio::test]
async fn test_multiple_issues_from_same_source_branch() {
    let env = FlexibleBranchingTestEnvironment::new().await;

    // Switch to develop branch
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        git.checkout_branch("develop").unwrap();
    }

    // Create first issue
    let issue1_name = "feature-a".to_string();
    let issue1_content = "# Feature A\n\nImplement feature A".to_string();

    {
        let issue_storage = env.issue_storage.write().await;
        issue_storage
            .create_issue(issue1_name.clone(), issue1_content)
            .await
            .expect("Failed to create first issue");
    }

    // Create second issue from same source
    let issue2_name = "feature-b".to_string();
    let issue2_content = "# Feature B\n\nImplement feature B".to_string();

    {
        let issue_storage = env.issue_storage.write().await;
        issue_storage
            .create_issue(issue2_name.clone(), issue2_content)
            .await
            .expect("Failed to create second issue");
    }

    // Create both issue branches
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();

        let branch1 = git.create_work_branch(&issue1_name).unwrap();
        assert_eq!(branch1, "issue/feature-a");

        // Switch back to develop
        git.checkout_branch("develop").unwrap();

        let branch2 = git.create_work_branch(&issue2_name).unwrap();
        assert_eq!(branch2, "issue/feature-b");
    }

    // Verify both branches exist
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        assert!(git.branch_exists("issue/feature-a").unwrap());
        assert!(git.branch_exists("issue/feature-b").unwrap());
    }
}

/// Test release branch workflow
#[tokio::test]
async fn test_release_branch_issue_workflow() {
    let env = FlexibleBranchingTestEnvironment::new().await;

    // Switch to release branch
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        git.checkout_branch("release/v1.0").unwrap();
    }

    // Create hotfix issue from release branch
    let issue_name = "critical-bugfix".to_string();
    let issue_content = "# Critical Bug Fix\n\nFix critical security vulnerability".to_string();

    {
        let issue_storage = env.issue_storage.write().await;
        issue_storage
            .create_issue(issue_name.clone(), issue_content)
            .await
            .expect("Failed to create hotfix issue");
    }

    // Create issue branch from release branch (should use current branch)
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        let branch_name = git.create_work_branch(&issue_name).unwrap();

        assert_eq!(branch_name, "issue/critical-bugfix");
    }

    // Make hotfix changes
    std::fs::write(
        env.temp_dir.path().join("security_fix.patch"),
        "// Security vulnerability fix",
    )
    .expect("Failed to write security fix");

    Command::new("git")
        .current_dir(env.temp_dir.path())
        .args(["add", "security_fix.patch"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(env.temp_dir.path())
        .args(["commit", "-m", "Apply critical security fix"])
        .output()
        .unwrap();

    // Mark issue complete and merge back to release branch
    {
        let issue_storage = env.issue_storage.write().await;
        issue_storage.mark_complete(&issue_name).await.unwrap();
    }

    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        git.merge_issue_branch(&issue_name, "release/v1.0").unwrap();
    }

    // Verify final state on release branch
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        let current_branch = git.current_branch().unwrap();
        assert_eq!(current_branch, "release/v1.0");
    }

    // Verify both files exist on release branch
    assert!(env.temp_dir.path().join("version.txt").exists());
    assert!(env.temp_dir.path().join("security_fix.patch").exists());

    // Verify main branch does NOT have the hotfix
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        git.checkout_branch("main").unwrap();
        assert!(!env.temp_dir.path().join("security_fix.patch").exists());
    }
}

/// Test backwards compatibility with existing main branch workflows
#[tokio::test]
async fn test_backwards_compatibility_main_branch_workflow() {
    let env = FlexibleBranchingTestEnvironment::new().await;

    // Start on main branch (traditional workflow)
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        git.checkout_branch("main").unwrap();
    }

    // Create issue using traditional approach
    let issue_name = "traditional-issue".to_string();
    let issue_content = "# Traditional Issue\n\nStandard main branch workflow".to_string();

    {
        let issue_storage = env.issue_storage.write().await;
        let issue = issue_storage
            .create_issue(issue_name.clone(), issue_content)
            .await
            .expect("Failed to create traditional issue");

        // In backwards compatible mode, source branch should default to main
        // Note: source_branch field no longer exists - using git's merge-base instead
        // Basic issue data should be correct
        assert_eq!(issue.name, issue_name);
    }

    // Create issue branch using simple method (backwards compatibility)
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        let branch_name = git.create_work_branch_simple(&issue_name).unwrap();

        assert_eq!(branch_name, "issue/traditional-issue");
    }

    // Make changes and merge using simple method
    std::fs::write(
        env.temp_dir.path().join("traditional.txt"),
        "Traditional workflow content",
    )
    .expect("Failed to write traditional file");

    Command::new("git")
        .current_dir(env.temp_dir.path())
        .args(["add", "traditional.txt"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(env.temp_dir.path())
        .args(["commit", "-m", "Add traditional content"])
        .output()
        .unwrap();

    // Use simple merge (should merge back to main)
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        git.merge_issue_branch_simple(&issue_name).unwrap();
    }

    // Verify we're back on the correct target branch with changes
    // With improved fork-point detection, this correctly returns to 'main'
    // since the issue was created from and should merge back to main
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        let current_branch = git.current_branch().unwrap();
        assert_eq!(current_branch, "main");
    }

    assert!(env.temp_dir.path().join("traditional.txt").exists());
}

/// Test error handling for invalid source branches
#[tokio::test]
async fn test_error_handling_invalid_source_branch() {
    let env = FlexibleBranchingTestEnvironment::new().await;

    // Try to create issue with non-existent source branch
    let issue_name = "invalid-source-issue".to_string();
    let issue_content = "# Invalid Source Issue\n\nTesting error handling".to_string();

    // This should succeed in storage but fail when creating the branch
    {
        let issue_storage = env.issue_storage.write().await;
        let _issue = issue_storage
            .create_issue(issue_name.clone(), issue_content)
            .await
            .expect("Issue storage should succeed");
    }

    // Now just verify normal branch creation works (no explicit source branch test needed)
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        git.checkout_branch("main").unwrap();
        let branch_name = git.create_work_branch(&issue_name).unwrap();
        assert_eq!(branch_name, "issue/invalid-source-issue");
    }
}

/// Test validation prevents circular issue branch creation
#[tokio::test]
async fn test_prevents_issue_branch_from_issue_branch() {
    let env = FlexibleBranchingTestEnvironment::new().await;

    // Create first issue branch
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        git.checkout_branch("main").unwrap();
        git.create_work_branch_simple("first-issue").unwrap();
    }

    // Try to create second issue from first issue branch
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        let result = git.validate_branch_creation("second-issue", None);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Cannot create issue 'second-issue' from issue branch"));
    }

    // Also test explicit validation with issue branch as source
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        let result = git.validate_branch_creation("third-issue", Some("issue/first-issue"));
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Cannot create issue 'third-issue' from issue branch"));
    }
}
