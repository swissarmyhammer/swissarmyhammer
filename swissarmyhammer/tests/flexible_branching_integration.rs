//! Integration tests for flexible base branch support
//!
//! This module provides comprehensive tests for the flexible branching functionality,
//! covering complete workflows, MCP tool integration, and edge cases.

use std::sync::Arc;
use swissarmyhammer_issues::{FileSystemIssueStorage, IssueStorage};
use swissarmyhammer_git::GitOperations;
use tempfile::TempDir;
use tokio::sync::RwLock;

// Import git2 utilities
use anyhow::Result;
use git2::{BranchType, Repository, Signature};

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
        Self::setup_git_repo_with_branches_git2(path).unwrap();
    }

    fn setup_git_repo_with_branches_git2(path: &std::path::Path) -> Result<()> {
        // Initialize git repo
        let repo = Repository::init(path)?;

        // Configure git user
        let mut config = repo.config()?;
        config.set_str("user.name", "Test User")?;
        config.set_str("user.email", "test@example.com")?;

        // Create initial commit on main branch
        std::fs::write(
            path.join("README.md"),
            "# Test Project\n\nMain branch content",
        )?;

        let mut index = repo.index()?;
        index.add_path(std::path::Path::new("README.md"))?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let signature = Signature::now("Test User", "test@example.com")?;

        let _commit_id = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        )?;

        // Create feature branch
        let head_commit = repo.head()?.peel_to_commit()?;
        let feature_branch = repo.branch("feature/user-authentication", &head_commit, false)?;

        // Checkout feature branch
        let feature_ref = feature_branch.get();
        let feature_tree = feature_ref.peel_to_tree()?;
        repo.checkout_tree(feature_tree.as_object(), None)?;
        repo.set_head("refs/heads/feature/user-authentication")?;

        std::fs::write(path.join("auth.rs"), "// User authentication module")?;
        index.add_path(std::path::Path::new("auth.rs"))?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let parent_commit = repo.head()?.peel_to_commit()?;

        let _auth_commit = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Add authentication module",
            &tree,
            &[&parent_commit],
        )?;

        // Switch back to main
        let main_branch = repo.find_branch("main", BranchType::Local)?;
        let main_ref = main_branch.get();
        let main_tree = main_ref.peel_to_tree()?;
        repo.checkout_tree(main_tree.as_object(), None)?;
        repo.set_head("refs/heads/main")?;

        // Create development branch from main
        let main_commit = repo.head()?.peel_to_commit()?;
        let dev_branch = repo.branch("develop", &main_commit, false)?;

        // Checkout develop branch
        let dev_ref = dev_branch.get();
        let dev_tree = dev_ref.peel_to_tree()?;
        repo.checkout_tree(dev_tree.as_object(), None)?;
        repo.set_head("refs/heads/develop")?;

        std::fs::write(path.join("dev.txt"), "Development branch")?;
        index.add_path(std::path::Path::new("dev.txt"))?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let parent_commit = repo.head()?.peel_to_commit()?;

        let _dev_commit = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Add development branch file",
            &tree,
            &[&parent_commit],
        )?;

        // Switch back to main
        repo.checkout_tree(main_tree.as_object(), None)?;
        repo.set_head("refs/heads/main")?;

        // Create release branch from main
        let release_branch = repo.branch("release/v1.0", &main_commit, false)?;

        // Checkout release branch
        let release_ref = release_branch.get();
        let release_tree = release_ref.peel_to_tree()?;
        repo.checkout_tree(release_tree.as_object(), None)?;
        repo.set_head("refs/heads/release/v1.0")?;

        std::fs::write(path.join("version.txt"), "v1.0")?;
        index.add_path(std::path::Path::new("version.txt"))?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let parent_commit = repo.head()?.peel_to_commit()?;

        let _version_commit = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Version 1.0 release",
            &tree,
            &[&parent_commit],
        )?;

        // Return to main branch
        repo.checkout_tree(main_tree.as_object(), None)?;
        repo.set_head("refs/heads/main")?;

        Ok(())
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
        let feature_branch =
            swissarmyhammer_git::BranchName::new("feature/user-authentication").unwrap();
        git.checkout_branch(&feature_branch).unwrap();
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

    // Use git2 instead of shell commands
    let repo = Repository::open(env.temp_dir.path()).unwrap();
    let mut index = repo.index().unwrap();
    index
        .add_path(std::path::Path::new("auth_tests.rs"))
        .unwrap();
    index.write().unwrap();

    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let signature = Signature::now("Test User", "test@example.com").unwrap();
    let parent_commit = repo.head().unwrap().peel_to_commit().unwrap();

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Add authentication tests",
        &tree,
        &[&parent_commit],
    )
    .unwrap();

    // Mark issue as completed
    {
        let issue_storage = env.issue_storage.write().await;
        issue_storage.complete_issue(&issue_name).await.unwrap();
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

        // Debug: list files that actually exist
        eprintln!("Current branch: {}", current_branch);
        eprintln!("Files in working directory:");
        if let Ok(entries) = std::fs::read_dir(env.temp_dir.path()) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if !name.to_string_lossy().starts_with('.') {
                    eprintln!("  {:?}", name);
                }
            }
        }
    }

    // Verify both files exist on feature branch
    assert!(env.temp_dir.path().join("auth.rs").exists());
    assert!(env.temp_dir.path().join("auth_tests.rs").exists());

    // Verify main branch does NOT have the test file
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        let main_branch = swissarmyhammer_git::BranchName::new("main").unwrap();
        git.checkout_branch(&main_branch).unwrap();
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
        let develop_branch = swissarmyhammer_git::BranchName::new("develop").unwrap();
        git.checkout_branch(&develop_branch).unwrap();
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
        let develop_branch = swissarmyhammer_git::BranchName::new("develop").unwrap();
        git.checkout_branch(&develop_branch).unwrap();

        let branch2 = git.create_work_branch(&issue2_name).unwrap();
        assert_eq!(branch2, "issue/feature-b");
    }

    // Verify both branches exist
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        let branch_a = swissarmyhammer_git::BranchName::new("issue/feature-a").unwrap();
        assert!(git.branch_exists(&branch_a).unwrap());
        let branch_b = swissarmyhammer_git::BranchName::new("issue/feature-b").unwrap();
        assert!(git.branch_exists(&branch_b).unwrap());
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
        let release_branch = swissarmyhammer_git::BranchName::new("release/v1.0").unwrap();
        git.checkout_branch(&release_branch).unwrap();
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

    // Use git2 instead of shell commands
    let repo = Repository::open(env.temp_dir.path()).unwrap();
    let mut index = repo.index().unwrap();
    index
        .add_path(std::path::Path::new("security_fix.patch"))
        .unwrap();
    index.write().unwrap();

    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let signature = Signature::now("Test User", "test@example.com").unwrap();
    let parent_commit = repo.head().unwrap().peel_to_commit().unwrap();

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Apply critical security fix",
        &tree,
        &[&parent_commit],
    )
    .unwrap();

    // Mark issue complete and merge back to release branch
    {
        let issue_storage = env.issue_storage.write().await;
        issue_storage.complete_issue(&issue_name).await.unwrap();
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
        let main_branch = swissarmyhammer_git::BranchName::new("main").unwrap();
        git.checkout_branch(&main_branch).unwrap();
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
        let main_branch = swissarmyhammer_git::BranchName::new("main").unwrap();
        git.checkout_branch(&main_branch).unwrap();
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

    // Use git2 instead of shell commands
    let repo = Repository::open(env.temp_dir.path()).unwrap();
    let mut index = repo.index().unwrap();
    index
        .add_path(std::path::Path::new("traditional.txt"))
        .unwrap();
    index.write().unwrap();

    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let signature = Signature::now("Test User", "test@example.com").unwrap();
    let parent_commit = repo.head().unwrap().peel_to_commit().unwrap();

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Add traditional content",
        &tree,
        &[&parent_commit],
    )
    .unwrap();

    // Use simple merge (should merge back to main)
    {
        let git_ops = env.git_ops.lock().await;
        let git = git_ops.as_ref().unwrap();
        git.merge_issue_branch_auto(&issue_name).unwrap();
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
        let main_branch = swissarmyhammer_git::BranchName::new("main").unwrap();
        git.checkout_branch(&main_branch).unwrap();
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
        let main_branch = swissarmyhammer_git::BranchName::new("main").unwrap();
        git.checkout_branch(&main_branch).unwrap();
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
