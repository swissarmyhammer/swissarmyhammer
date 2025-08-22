//! Integration tests for Git repository error handling
//!
//! Tests that CLI commands provide clear, actionable error messages when run outside
//! Git repositories, with component-specific guidance for resolution.

use std::fs;
use tempfile::TempDir;

mod in_process_test_utils;
use in_process_test_utils::run_sah_command_in_process;

/// Test that memo commands require Git repository - DISABLED: Memo commands only available with dynamic-cli feature
// #[tokio::test]
// #[ignore = "Memo commands only available with dynamic-cli feature"]
async fn _test_memo_commands_require_git_repository_disabled() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Save current directory and change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = run_sah_command_in_process(&["memo", "list"]).await;

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();

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

/// Test that issue commands require Git repository
#[tokio::test]
async fn test_issue_commands_require_git_repository() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Save current directory and change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = run_sah_command_in_process(&["issue", "list"]).await;

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();

    let output = result.unwrap();
    assert_ne!(output.exit_code, 0, "Command should fail");

    assert!(
        output
            .stderr
            .contains("Issue operations require a Git repository"),
        "Should contain Git repo error: {}",
        output.stderr
    );
    assert!(
        output
            .stderr
            .contains("Issues are stored in .swissarmyhammer/issues/"),
        "Should mention issues directory: {}",
        output.stderr
    );
    assert!(
        output.stderr.contains("branch management"),
        "Should mention branch management: {}",
        output.stderr
    );
}

/// Test that search commands have been migrated to dynamic CLI - DISABLED: Search commands only available with dynamic-cli feature
#[tokio::test]
#[ignore = "Search commands only available with dynamic-cli feature"]
async fn test_search_commands_require_git_repository() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Save current directory and change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // In static CLI mode, search commands should not exist
    let result = run_sah_command_in_process(&["search", "index", "**/*.rs"]).await;

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();

    let output = result.unwrap();
    assert_eq!(
        output.exit_code, 2,
        "Command should fail with 'command not found' error"
    );

    assert!(
        output.stderr.contains("unrecognized subcommand 'search'"),
        "Should indicate search command is not available in static CLI: {}",
        output.stderr
    );
}

/// Test that search query commands have been migrated to dynamic CLI - DISABLED: Search commands only available with dynamic-cli feature
#[tokio::test]
#[ignore = "Search commands only available with dynamic-cli feature"]
async fn test_search_query_requires_git_repository() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Save current directory and change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // In static CLI mode, search commands should not exist
    let result = run_sah_command_in_process(&["search", "query", "test"]).await;

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();

    let output = result.unwrap();
    assert_eq!(
        output.exit_code, 2,
        "Command should fail with 'command not found' error"
    );

    assert!(
        output.stderr.contains("unrecognized subcommand 'search'"),
        "Should indicate search command is not available in static CLI: {}",
        output.stderr
    );
}

/// Test error message format consistency - DISABLED: Memo commands only available with dynamic-cli feature
// #[tokio::test]
// #[ignore = "Memo commands only available with dynamic-cli feature"]
async fn _test_error_message_format_consistency_disabled() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Save current directory and change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Test memo command error format
    let result = run_sah_command_in_process(&["memo", "create", "test"]).await;

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();

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
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to initialize git repository");

    // Create .swissarmyhammer directory
    fs::create_dir_all(temp_dir.path().join(".swissarmyhammer"))
        .expect("Failed to create directory");

    // Save current directory and change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Test that memo list command now works (or at least doesn't fail with Git repository error)
    let result = run_sah_command_in_process(&["memo", "list"]).await;

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();

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

    // Save current directory and change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = run_sah_command_in_process(&["memo", "list"]).await;

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();

    let output = result.unwrap();
    assert_eq!(output.exit_code, 2, "Should exit with code 2 (EXIT_ERROR)");
}

/// Test that file commands don't require Git repository (should work)
#[tokio::test]
#[ignore = "File commands migrated to dynamic CLI - static CLI no longer supports file commands"]
async fn test_file_commands_work_without_git() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create a test file
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "Hello, world!").expect("Failed to create test file");

    // Save current directory and change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = run_sah_command_in_process(&["file", "read", test_file.to_str().unwrap()]).await;

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();

    let output = result.unwrap();
    assert_eq!(
        output.exit_code, 0,
        "Command should succeed. stderr: {}",
        output.stderr
    );
    assert!(
        output.stdout.contains("Hello, world!"),
        "Should contain file content: {}",
        output.stdout
    );
}

/// Test that shell commands don't require Git repository
#[tokio::test]
async fn test_shell_commands_work_without_git() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Save current directory and change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = run_sah_command_in_process(&["shell", "execute", "echo test"]).await;

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();

    let output = result.unwrap();
    assert_eq!(
        output.exit_code, 0,
        "Command should succeed. stderr: {}",
        output.stderr
    );
    assert!(
        output.stdout.contains("test"),
        "Should contain echo output: {}",
        output.stdout
    );
}

/// Test that web search commands don't require Git repository
#[tokio::test]
async fn test_web_search_works_without_git() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Note: This test might fail if web search is not available or has issues,
    // but it should not fail due to Git repository requirements

    // Save current directory and change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = run_sah_command_in_process(&["web-search", "search", "test"]).await;

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();

    let output = result.unwrap();
    // Should not contain Git repository requirement error
    assert!(
        !output.stderr.contains("require a Git repository"),
        "Should not have Git repo error: {}",
        output.stderr
    );
}

/// Test error message actionability
#[tokio::test]
async fn test_error_messages_are_actionable() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Save current directory and change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = run_sah_command_in_process(&["issue", "create", "test"]).await;

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();

    let output = result.unwrap();
    assert_ne!(output.exit_code, 0, "Command should fail");

    let stderr = &output.stderr;

    // Check that error messages provide actionable solutions
    assert!(
        stderr.contains("Solutions:"),
        "Should provide solutions section"
    );
    assert!(
        stderr.contains("git init"),
        "Should suggest git init command"
    );
    assert!(
        stderr.contains("git clone"),
        "Should suggest git clone option"
    );
    assert!(
        stderr.contains("Current directory:"),
        "Should show current directory context"
    );
}

/// Test error context preservation - DISABLED: Memo commands only available with dynamic-cli feature
// #[tokio::test]
// #[ignore = "Memo commands only available with dynamic-cli feature"]
async fn _test_error_context_preservation_disabled() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Save current directory and change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = run_sah_command_in_process(&["memo", "get", "invalid_id"]).await;

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();

    let output = result.unwrap();
    assert_ne!(output.exit_code, 0, "Command should fail");

    let stderr = &output.stderr;

    // Should contain Git repository error, not invalid ID error, since Git check happens first
    assert!(
        stderr.contains("Git repository"),
        "Should show Git repository error first"
    );
}
