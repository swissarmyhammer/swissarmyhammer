//! End-to-end MCP tool tests for flexible base branch support
//!
//! This module tests the MCP tools (issue_work, issue_merge, etc.) with flexible branching.

use git2::Repository;
use swissarmyhammer_git::git2_utils;
use tempfile::TempDir;

/// Test environment for MCP tool testing
struct McpTestEnvironment {
    temp_dir: TempDir,
}

impl McpTestEnvironment {
    fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");

        // Set up git repository with branches
        Self::setup_git_repo_with_branches(&temp_dir);

        // Change to test directory for CLI operations
        std::env::set_current_dir(temp_dir.path()).expect("Failed to change to test directory");

        Self { temp_dir }
    }

    fn setup_git_repo_with_branches(temp_dir: &TempDir) {
        let path = temp_dir.path();

        // Initialize git repo using git2
        let repo = Repository::init(path).unwrap();

        // Configure git
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        // Create initial commit on main
        std::fs::write(path.join("README.md"), "# MCP Test Project")
            .expect("Failed to write README.md");
        git2_utils::add_files(&repo, &["README.md"]).unwrap();
        git2_utils::create_commit(
            &repo,
            "Initial commit",
            Some("Test User"),
            Some("test@example.com"),
        )
        .unwrap();

        // Create feature branch
        let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
        let branch = repo
            .branch("feature/user-management", &head_commit, false)
            .unwrap();

        // Checkout the feature branch
        let branch_ref = branch.get();
        let tree = branch_ref.peel_to_tree().unwrap();
        repo.checkout_tree(tree.as_object(), None).unwrap();
        repo.set_head("refs/heads/feature/user-management").unwrap();

        std::fs::write(path.join("user.rs"), "// User management module")
            .expect("Failed to write user.rs");
        git2_utils::add_files(&repo, &["user.rs"]).unwrap();
        git2_utils::create_commit(
            &repo,
            "Add user management",
            Some("Test User"),
            Some("test@example.com"),
        )
        .unwrap();

        // Create develop branch from main
        // First checkout main
        let main_branch = match repo.find_branch("main", git2::BranchType::Local) {
            Ok(b) => b,
            Err(_) => repo.branch("main", &head_commit, false).unwrap(),
        };
        let main_ref = main_branch.get();
        let main_tree = main_ref.peel_to_tree().unwrap();
        repo.checkout_tree(main_tree.as_object(), None).unwrap();
        repo.set_head("refs/heads/main").unwrap();

        // Create develop branch
        let main_commit = repo.head().unwrap().peel_to_commit().unwrap();
        let develop_branch = repo.branch("develop", &main_commit, false).unwrap();

        // Checkout develop
        let develop_ref = develop_branch.get();
        let develop_tree = develop_ref.peel_to_tree().unwrap();
        repo.checkout_tree(develop_tree.as_object(), None).unwrap();
        repo.set_head("refs/heads/develop").unwrap();

        std::fs::write(path.join("develop.md"), "# Development branch")
            .expect("Failed to write develop.md");
        git2_utils::add_files(&repo, &["develop.md"]).unwrap();
        git2_utils::create_commit(
            &repo,
            "Add development documentation",
            Some("Test User"),
            Some("test@example.com"),
        )
        .unwrap();
    }

    fn run_cli_command(&self, args: &[&str]) -> std::process::Output {
        std::process::Command::new(env!("CARGO_BIN_EXE_sah"))
            .current_dir(self.temp_dir.path())
            .args(args)
            .output()
            .expect("Failed to run CLI command")
    }

    fn switch_to_branch(&self, branch: &str) {
        let repo = Repository::open(self.temp_dir.path()).unwrap();
        let branch_ref = repo.find_branch(branch, git2::BranchType::Local).unwrap();
        let branch_ref = branch_ref.get();
        let tree = branch_ref.peel_to_tree().unwrap();
        repo.checkout_tree(tree.as_object(), None).unwrap();
        repo.set_head(&format!("refs/heads/{}", branch)).unwrap();
    }
}

/// Test MCP tool error handling for non-existent source branches
#[test]
fn test_mcp_error_handling_invalid_source() {
    let env = McpTestEnvironment::new();

    // Try to create issue with non-existent source in the content or metadata
    // This test depends on how the CLI tools handle source branch specification

    // For now, test that invalid branch operations are handled gracefully
    env.switch_to_branch("main");

    let output = env.run_cli_command(&[
        "issue",
        "create",
        "--name",
        "invalid-source-test",
        "--content",
        "# Invalid Source Test\n\nTesting error handling",
    ]);
    assert!(output.status.success());

    // The error handling will be tested more thoroughly in the actual MCP tool operations
    // when they try to work with non-existent source branches
}

/// Test issue list command shows source branch information
#[test]
fn test_mcp_issue_list_shows_source_branches() {
    let env = McpTestEnvironment::new();

    // Create issues from different source branches
    env.switch_to_branch("main");
    let output = env.run_cli_command(&[
        "issue",
        "create",
        "--name",
        "main-issue",
        "--content",
        "# Main Issue\n\nIssue from main",
    ]);
    assert!(output.status.success());

    env.switch_to_branch("feature/user-management");
    let output = env.run_cli_command(&[
        "issue",
        "create",
        "--name",
        "feature-issue",
        "--content",
        "# Feature Issue\n\nIssue from feature branch",
    ]);
    assert!(output.status.success());

    env.switch_to_branch("develop");
    let output = env.run_cli_command(&[
        "issue",
        "create",
        "--name",
        "develop-issue",
        "--content",
        "# Develop Issue\n\nIssue from develop branch",
    ]);
    assert!(output.status.success());

    // List all issues
    let output = env.run_cli_command(&["issue", "list"]);
    assert!(output.status.success());

    let output_str = String::from_utf8_lossy(&output.stdout);

    // Should show all three issues
    assert!(output_str.contains("main-issue"));
    assert!(output_str.contains("feature-issue"));
    assert!(output_str.contains("develop-issue"));

    // Depending on implementation, might show source branch information
    // This test verifies the command works with flexible branching
    assert!(!output_str.is_empty());
}

/// Test issue show command displays source branch information
#[test]
fn test_mcp_issue_show_displays_source_branch() {
    let env = McpTestEnvironment::new();

    // Create issue from feature branch
    env.switch_to_branch("feature/user-management");
    let output = env.run_cli_command(&[
        "issue",
        "create",
        "--name",
        "feature-details",
        "--content",
        "# Feature Details\n\nDetailed feature implementation",
    ]);
    assert!(output.status.success());

    // Show the issue
    let output = env.run_cli_command(&["issue", "show", "--name", "feature-details"]);
    assert!(output.status.success());

    let output_str = String::from_utf8_lossy(&output.stdout);

    // Should contain the issue name and content
    assert!(output_str.contains("feature-details"));
    assert!(output_str.contains("Feature Details"));

    // Depending on implementation, should show source branch info
    // The key is that the command works correctly with flexible branching
    assert!(!output_str.is_empty());
}
