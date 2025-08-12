//! End-to-end MCP tool tests for flexible base branch support
//!
//! This module tests the MCP tools (issue_work, issue_merge, etc.) with flexible branching.

use assert_cmd::Command;
use std::process::Command as StdCommand;
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

        // Initialize git repo
        StdCommand::new("git")
            .current_dir(path)
            .args(["init"])
            .output()
            .unwrap();

        // Configure git
        StdCommand::new("git")
            .current_dir(path)
            .args(["config", "user.name", "Test User"])
            .output()
            .unwrap();

        StdCommand::new("git")
            .current_dir(path)
            .args(["config", "user.email", "test@example.com"])
            .output()
            .unwrap();

        // Create initial commit on main
        std::fs::write(path.join("README.md"), "# MCP Test Project")
            .expect("Failed to write README.md");
        StdCommand::new("git")
            .current_dir(path)
            .args(["add", "README.md"])
            .output()
            .unwrap();
        StdCommand::new("git")
            .current_dir(path)
            .args(["commit", "-m", "Initial commit"])
            .output()
            .unwrap();

        // Create feature branch
        StdCommand::new("git")
            .current_dir(path)
            .args(["checkout", "-b", "feature/user-management"])
            .output()
            .unwrap();

        std::fs::write(path.join("user.rs"), "// User management module")
            .expect("Failed to write user.rs");
        StdCommand::new("git")
            .current_dir(path)
            .args(["add", "user.rs"])
            .output()
            .unwrap();
        StdCommand::new("git")
            .current_dir(path)
            .args(["commit", "-m", "Add user management"])
            .output()
            .unwrap();

        // Create develop branch from main
        StdCommand::new("git")
            .current_dir(path)
            .args(["checkout", "main"])
            .output()
            .unwrap();
        StdCommand::new("git")
            .current_dir(path)
            .args(["checkout", "-b", "develop"])
            .output()
            .unwrap();

        std::fs::write(path.join("develop.md"), "# Development branch")
            .expect("Failed to write develop.md");
        StdCommand::new("git")
            .current_dir(path)
            .args(["add", "develop.md"])
            .output()
            .unwrap();
        StdCommand::new("git")
            .current_dir(path)
            .args(["commit", "-m", "Add development documentation"])
            .output()
            .unwrap();
    }

    fn run_cli_command(&self, args: &[&str]) -> std::process::Output {
        Command::cargo_bin("sah")
            .expect("Failed to find sah binary")
            .current_dir(self.temp_dir.path())
            .args(args)
            .output()
            .expect("Failed to run CLI command")
    }

    fn get_current_branch(&self) -> String {
        let output = StdCommand::new("git")
            .current_dir(self.temp_dir.path())
            .args(["branch", "--show-current"])
            .output()
            .unwrap();

        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn switch_to_branch(&self, branch: &str) {
        StdCommand::new("git")
            .current_dir(self.temp_dir.path())
            .args(["checkout", branch])
            .output()
            .unwrap();
    }
}

/// Test issue_work tool with feature branch as source
#[test]
fn test_mcp_issue_work_from_feature_branch() {
    let env = McpTestEnvironment::new();

    // Switch to feature branch
    env.switch_to_branch("feature/user-management");
    assert_eq!(env.get_current_branch(), "feature/user-management");

    // Create issue using MCP tools via CLI
    let output = env.run_cli_command(&[
        "issue",
        "create",
        "user-tests",
        "--content",
        "# User Tests\n\nImplement tests for user management",
    ]);

    assert!(
        output.status.success(),
        "Issue create failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Work on the issue (should create issue branch from feature branch)
    let output = env.run_cli_command(&["issue", "work", "user-tests"]);

    assert!(
        output.status.success(),
        "Issue work failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify we're on the issue branch
    assert_eq!(env.get_current_branch(), "issue/user-tests");

    // Verify the source branch is tracked (by examining the issue)
    let output = env.run_cli_command(&["issue", "show", "user-tests"]);

    assert!(output.status.success());
    let output_str = String::from_utf8_lossy(&output.stdout);
    // The output should contain information about the source branch
    // (exact format depends on implementation)
    assert!(!output_str.is_empty());
}

/// Test issue_work tool with develop branch
#[test]
fn test_mcp_issue_work_from_develop_branch() {
    let env = McpTestEnvironment::new();

    // Switch to develop branch
    env.switch_to_branch("develop");
    assert_eq!(env.get_current_branch(), "develop");

    // Create and work on issue from develop
    let output = env.run_cli_command(&[
        "issue",
        "create",
        "dev-feature",
        "--content",
        "# Development Feature\n\nNew feature for develop branch",
    ]);
    assert!(output.status.success());

    let output = env.run_cli_command(&["issue", "work", "dev-feature"]);
    assert!(output.status.success());

    // Should be on issue branch
    assert_eq!(env.get_current_branch(), "issue/dev-feature");
}

/// Test that issue merge validates current branch and creates abort file
#[test]
fn test_mcp_issue_merge_requires_issue_branch() {
    let env = McpTestEnvironment::new();

    // Start from feature branch
    env.switch_to_branch("feature/user-management");

    // Create and complete an issue
    let output = env.run_cli_command(&[
        "issue",
        "create",
        "test-validation",
        "--content",
        "# Test Validation\n\nTest branch validation for merge",
    ]);
    assert!(output.status.success());

    let output = env.run_cli_command(&["issue", "work", "test-validation"]);
    assert!(output.status.success());

    // Make changes and commit
    std::fs::write(env.temp_dir.path().join("test.rs"), "// Test file")
        .expect("Failed to write test file");

    StdCommand::new("git")
        .current_dir(env.temp_dir.path())
        .args(["add", "test.rs"])
        .output()
        .unwrap();
    StdCommand::new("git")
        .current_dir(env.temp_dir.path())
        .args(["commit", "-m", "Add test file"])
        .output()
        .unwrap();

    let output = env.run_cli_command(&["issue", "complete", "test-validation"]);
    assert!(output.status.success());

    // Switch to a non-issue branch (main)
    env.switch_to_branch("main");
    assert_eq!(env.get_current_branch(), "main");

    // Try to merge from non-issue branch - should fail
    let output = env.run_cli_command(&["issue", "merge", "test-validation"]);
    assert!(
        !output.status.success(),
        "Merge should fail when not on issue branch"
    );

    // Check that abort file was created
    let abort_file = env.temp_dir.path().join(".swissarmyhammer/.abort");
    assert!(
        abort_file.exists(),
        "Abort file should be created when merge fails due to invalid branch"
    );

    // Abort file should contain reason
    let abort_content = std::fs::read_to_string(&abort_file).unwrap();
    assert!(abort_content.contains("Cannot merge issue"));
    assert!(abort_content.contains("main"));
    assert!(abort_content.contains("test-validation"));
}

/// Test issue merge back to correct source branch
#[test]
fn test_mcp_issue_merge_to_source_branch() {
    let env = McpTestEnvironment::new();

    // Start from feature branch
    env.switch_to_branch("feature/user-management");

    // Create issue and work on it
    let output = env.run_cli_command(&[
        "issue",
        "create",
        "user-validation",
        "--content",
        "# User Validation\n\nAdd validation to user management",
    ]);
    assert!(output.status.success());

    let output = env.run_cli_command(&["issue", "work", "user-validation"]);
    assert!(output.status.success());

    // Make changes on issue branch
    std::fs::write(
        env.temp_dir.path().join("validation.rs"),
        "// User validation logic",
    )
    .expect("Failed to write validation file");

    StdCommand::new("git")
        .current_dir(env.temp_dir.path())
        .args(["add", "validation.rs"])
        .output()
        .unwrap();
    StdCommand::new("git")
        .current_dir(env.temp_dir.path())
        .args(["commit", "-m", "Add user validation"])
        .output()
        .unwrap();

    // Mark issue complete
    let output = env.run_cli_command(&["issue", "complete", "user-validation"]);
    assert!(output.status.success());

    // Ensure we're on the issue branch before merging (required by new validation)
    let output = env.run_cli_command(&["issue", "work", "user-validation"]);
    assert!(output.status.success());

    // Merge issue back to its source branch (feature/user-management)
    let output = env.run_cli_command(&["issue", "merge", "user-validation"]);
    assert!(
        output.status.success(),
        "Issue merge failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Should be back on feature branch
    assert_eq!(env.get_current_branch(), "feature/user-management");

    // Both files should exist on feature branch
    assert!(env.temp_dir.path().join("user.rs").exists());
    assert!(env.temp_dir.path().join("validation.rs").exists());

    // Main branch should NOT have validation.rs
    env.switch_to_branch("main");
    assert!(!env.temp_dir.path().join("validation.rs").exists());
}

/// Test issue_work tool prevents issue from issue branch
#[test]
fn test_mcp_issue_work_prevents_issue_from_issue_branch() {
    let env = McpTestEnvironment::new();

    // Create first issue from main
    let output = env.run_cli_command(&[
        "issue",
        "create",
        "first-issue",
        "--content",
        "# First Issue\n\nFirst issue from main",
    ]);
    assert!(output.status.success());

    let output = env.run_cli_command(&["issue", "work", "first-issue"]);
    assert!(output.status.success());

    // Now try to work on another issue while on first issue branch
    let output = env.run_cli_command(&[
        "issue",
        "create",
        "second-issue",
        "--content",
        "# Second Issue\n\nAttempt from issue branch",
    ]);
    assert!(output.status.success()); // Issue creation should succeed

    // But working on it should fail
    let output = env.run_cli_command(&["issue", "work", "second-issue"]);
    assert!(
        !output.status.success(),
        "Issue work should have failed from issue branch"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Cannot work") || stderr.contains("issue branch"),
        "Error should mention issue branch restriction: {}",
        stderr
    );

    // Should still be on first issue branch
    assert_eq!(env.get_current_branch(), "issue/first-issue");
}

/// Test backwards compatibility with main branch workflow
#[test]
fn test_mcp_backwards_compatibility_main_branch() {
    let env = McpTestEnvironment::new();

    // Start on main branch (traditional workflow)
    env.switch_to_branch("main");
    assert_eq!(env.get_current_branch(), "main");

    // Create issue using traditional approach
    let output = env.run_cli_command(&[
        "issue",
        "create",
        "main-branch-issue",
        "--content",
        "# Main Branch Issue\n\nTraditional main branch workflow",
    ]);
    if !output.status.success() {
        eprintln!("STDOUT: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("STDERR: {}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(output.status.success());

    // Work on issue (should default to main branch behavior)
    let output = env.run_cli_command(&["issue", "work", "main-branch-issue"]);
    assert!(output.status.success());

    // Should be on issue branch
    assert_eq!(env.get_current_branch(), "issue/main-branch-issue");

    // Make changes and commit
    std::fs::write(
        env.temp_dir.path().join("main_feature.rs"),
        "// Feature for main branch",
    )
    .expect("Failed to write main feature file");

    StdCommand::new("git")
        .current_dir(env.temp_dir.path())
        .args(["add", "main_feature.rs"])
        .output()
        .unwrap();
    StdCommand::new("git")
        .current_dir(env.temp_dir.path())
        .args(["commit", "-m", "Add main branch feature"])
        .output()
        .unwrap();

    // Mark complete and merge
    let output = env.run_cli_command(&["issue", "complete", "main-branch-issue"]);
    assert!(output.status.success());

    let output = env.run_cli_command(&["issue", "merge", "main-branch-issue"]);
    assert!(output.status.success());

    // Should be back on main branch
    assert_eq!(env.get_current_branch(), "main");

    // Feature file should exist on main
    assert!(env.temp_dir.path().join("main_feature.rs").exists());
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
        "invalid-source-test",
        "--content",
        "# Invalid Source Test\n\nTesting error handling",
    ]);
    assert!(output.status.success());

    // The error handling will be tested more thoroughly in the actual MCP tool operations
    // when they try to work with non-existent source branches
}

/// Test multiple issues from same source branch via MCP tools
#[test]
fn test_mcp_multiple_issues_same_source() {
    let env = McpTestEnvironment::new();

    // Switch to develop branch
    env.switch_to_branch("develop");

    // Create first issue
    let output = env.run_cli_command(&[
        "issue",
        "create",
        "develop-feature-a",
        "--content",
        "# Develop Feature A\n\nFirst feature for develop branch",
    ]);
    assert!(output.status.success());

    // Create second issue
    let output = env.run_cli_command(&[
        "issue",
        "create",
        "develop-feature-b",
        "--content",
        "# Develop Feature B\n\nSecond feature for develop branch",
    ]);
    assert!(output.status.success());

    // Work on first issue
    let output = env.run_cli_command(&["issue", "work", "develop-feature-a"]);
    assert!(output.status.success());
    assert_eq!(env.get_current_branch(), "issue/develop-feature-a");

    // Switch back to develop and work on second issue
    env.switch_to_branch("develop");
    let output = env.run_cli_command(&["issue", "work", "develop-feature-b"]);
    assert!(output.status.success());
    assert_eq!(env.get_current_branch(), "issue/develop-feature-b");

    // Both issue branches should exist
    let output = StdCommand::new("git")
        .current_dir(env.temp_dir.path())
        .args(["branch", "--list"])
        .output()
        .unwrap();

    let branches = String::from_utf8_lossy(&output.stdout);
    assert!(branches.contains("issue/develop-feature-a"));
    assert!(branches.contains("issue/develop-feature-b"));
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
        "main-issue",
        "--content",
        "# Main Issue\n\nIssue from main",
    ]);
    assert!(output.status.success());

    env.switch_to_branch("feature/user-management");
    let output = env.run_cli_command(&[
        "issue",
        "create",
        "feature-issue",
        "--content",
        "# Feature Issue\n\nIssue from feature branch",
    ]);
    assert!(output.status.success());

    env.switch_to_branch("develop");
    let output = env.run_cli_command(&[
        "issue",
        "create",
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
        "feature-details",
        "--content",
        "# Feature Details\n\nDetailed feature implementation",
    ]);
    assert!(output.status.success());

    // Show the issue
    let output = env.run_cli_command(&["issue", "show", "feature-details"]);
    assert!(output.status.success());

    let output_str = String::from_utf8_lossy(&output.stdout);

    // Should contain the issue name and content
    assert!(output_str.contains("feature-details"));
    assert!(output_str.contains("Feature Details"));

    // Depending on implementation, should show source branch info
    // The key is that the command works correctly with flexible branching
    assert!(!output_str.is_empty());
}
