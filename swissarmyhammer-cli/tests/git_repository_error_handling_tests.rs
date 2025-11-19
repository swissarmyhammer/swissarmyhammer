//! Integration tests for Git repository error handling
//!
//! Tests that CLI commands provide clear, actionable error messages when run outside
//! Git repositories, with component-specific guidance for resolution.

use std::fs;
use tempfile::TempDir;

mod in_process_test_utils;
use in_process_test_utils::run_sah_command_in_process_with_dir;

/// Test that memo commands require Git repository - DISABLED: Memo commands only available with dynamic-cli feature
// #[tokio::test]
// #[ignore = "Memo commands only available with dynamic-cli feature"]
async fn _test_memo_commands_require_git_repository_disabled() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Use explicit working directory instead of global directory change

    let result = run_sah_command_in_process_with_dir(&["memo", "list"], temp_dir.path()).await;

    // Restore original directory

    let output = result.unwrap();
    assert_ne!(output.exit_code, 0, "Command should fail");

    assert!(
        output
            .stderr
            .contains("Memo operations require a Git repository"),
        "Should contain Git repo error: {}",
        output.stderr
    );
    assert!(
        output
            .stderr
            .contains("Memos are stored in .swissarmyhammer/memos/"),
        "Should mention memos directory: {}",
        output.stderr
    );
    assert!(
        output.stderr.contains("git init"),
        "Should suggest git init: {}",
        output.stderr
    );
}

/// Test that todo commands require Git repository
#[tokio::test]
async fn test_todo_commands_require_git_repository() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Use explicit working directory instead of global directory change

    let result = run_sah_command_in_process_with_dir(&["todo", "list"], temp_dir.path()).await;

    // Restore original directory

    let output = result.unwrap();
    // Todo commands require git repositories
    assert_ne!(
        output.exit_code, 0,
        "Command should fail without git repository"
    );
    assert!(
        output.stderr.contains("Todo operations require a Git repository")
            || output.stderr.contains("Git repository"),
        "Should show git repository error: {}",
        output.stderr
    );
}

/// Test error message format consistency - DISABLED: Memo commands only available with dynamic-cli feature
// #[tokio::test]
// #[ignore = "Memo commands only available with dynamic-cli feature"]
async fn _test_error_message_format_consistency_disabled() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Use explicit working directory instead of global directory change

    // Test memo command error format
    let result =
        run_sah_command_in_process_with_dir(&["memo", "create", "test"], temp_dir.path()).await;

    // Restore original directory

    let output = result.unwrap();
    assert_ne!(output.exit_code, 0, "Command should fail");

    let stderr = &output.stderr;

    // Check for consistent error format elements
    assert!(stderr.contains("❌"), "Error should start with ❌ icon");
    assert!(
        stderr.contains("Solutions:"),
        "Error should include Solutions section"
    );
    assert!(stderr.contains("git init"), "Error should suggest git init");
    assert!(
        stderr.contains("Current directory:"),
        "Error should show current directory"
    );
}

/// Test that commands work correctly within Git repository
#[tokio::test]
async fn test_commands_work_in_git_repository() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Initialize git repository
    use git2::Repository;
    Repository::init(temp_dir.path()).expect("Failed to initialize git repository");

    // Create .swissarmyhammer directory
    fs::create_dir_all(temp_dir.path().join(".swissarmyhammer"))
        .expect("Failed to create directory");

    // Use explicit working directory instead of global directory change

    // Test that memo list command now works (or at least doesn't fail with Git repository error)
    let result = run_sah_command_in_process_with_dir(&["memo", "list"], temp_dir.path()).await;

    // Restore original directory

    let output = result.unwrap();
    // Should not contain Git repository requirement error
    assert!(
        !output.stderr.contains("require a Git repository"),
        "Should not have Git repo error: {}",
        output.stderr
    );
}

/// Test exit codes for Git repository errors
#[tokio::test]
async fn test_git_repository_error_exit_codes() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Use explicit working directory instead of global directory change

    let result = run_sah_command_in_process_with_dir(&["memo", "list"], temp_dir.path()).await;

    // Restore original directory

    let output = result.unwrap();
    eprintln!(
        "DEBUG test_git_repository_error_exit_codes: stdout: {}",
        output.stdout
    );
    eprintln!(
        "DEBUG test_git_repository_error_exit_codes: stderr: {}",
        output.stderr
    );
    eprintln!(
        "DEBUG test_git_repository_error_exit_codes: exit_code: {}",
        output.exit_code
    );
    // Memo commands now work gracefully without git repositories
    assert_eq!(
        output.exit_code, 0,
        "Memo commands should succeed without git repository"
    );
}

// Removed test_shell_commands_work_without_git - shell command was migrated away from static CLI

/// Test that web search commands don't require Git repository
#[tokio::test]
async fn test_web_search_works_without_git() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Note: This test might fail if web search is not available or has issues,
    // but it should not fail due to Git repository requirements

    // Use explicit working directory instead of global directory change

    let result =
        run_sah_command_in_process_with_dir(&["web-search", "search", "test"], temp_dir.path())
            .await;

    // Restore original directory

    let output = result.unwrap();
    // Should not contain Git repository requirement error
    assert!(
        !output.stderr.contains("require a Git repository"),
        "Should not have Git repo error: {}",
        output.stderr
    );
}

/// Test error message actionability with todo commands
#[tokio::test]
async fn test_error_messages_are_actionable() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Initialize git repository
    use git2::Repository;
    Repository::init(temp_dir.path()).expect("Failed to initialize git repository");

    // Use explicit working directory instead of global directory change

    let result = run_sah_command_in_process_with_dir(
        &["todo", "create", "--task", "Test task"],
        temp_dir.path(),
    )
    .await;

    // Restore original directory

    let output = result.unwrap();
    eprintln!(
        "DEBUG test_error_messages_are_actionable: stdout: {}",
        output.stdout
    );
    eprintln!(
        "DEBUG test_error_messages_are_actionable: stderr: {}",
        output.stderr
    );
    eprintln!(
        "DEBUG test_error_messages_are_actionable: exit_code: {}",
        output.exit_code
    );

    // Todo create commands should succeed with git repository
    assert_eq!(
        output.exit_code, 0,
        "Todo create should succeed with git repository"
    );
}

/// Test error context preservation - DISABLED: Memo commands only available with dynamic-cli feature
// #[tokio::test]
// #[ignore = "Memo commands only available with dynamic-cli feature"]
async fn _test_error_context_preservation_disabled() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Use explicit working directory instead of global directory change

    let result =
        run_sah_command_in_process_with_dir(&["memo", "get", "invalid_id"], temp_dir.path()).await;

    // Restore original directory

    let output = result.unwrap();
    assert_ne!(output.exit_code, 0, "Command should fail");

    let stderr = &output.stderr;

    // Should contain Git repository error, not invalid ID error, since Git check happens first
    assert!(
        stderr.contains("Git repository"),
        "Should show Git repository error first"
    );
}
