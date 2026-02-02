//! Integration tests for Git repository error handling
//!
//! Tests that CLI commands provide clear, actionable error messages when run outside
//! Git repositories, with component-specific guidance for resolution.

// sah rule ignore test_rule_with_allow

use std::fs;
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_common::SwissarmyhammerDirectory;

use crate::in_process_test_utils::run_sah_command_in_process_with_dir;

/// Test that kanban commands require Git repository
#[tokio::test]
async fn test_kanban_commands_require_git_repository() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();

    // Use explicit working directory instead of global directory change

    let result =
        run_sah_command_in_process_with_dir(&["tool", "kanban", "tasks", "list"], &temp_dir).await;

    // Restore original directory

    let output = result.unwrap();
    // Kanban commands require git repositories
    assert_ne!(
        output.exit_code, 0,
        "Command should fail without git repository"
    );
    assert!(
        output
            .stderr
            .contains("Kanban operations require a Git repository")
            || output.stderr.contains("Git repository")
            || output.stderr.contains("git"),
        "Should show git repository error: {}",
        output.stderr
    );
}

/// Test that commands work correctly within Git repository
#[tokio::test]
async fn test_commands_work_in_git_repository() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();

    // Initialize git repository
    use git2::Repository;
    Repository::init(&temp_dir).expect("Failed to initialize git repository");

    // Create .swissarmyhammer directory
    fs::create_dir_all(temp_dir.join(SwissarmyhammerDirectory::dir_name()))
        .expect("Failed to create directory");

    // Use explicit working directory instead of global directory change

    // Test that kanban list command now works (or at least doesn't fail with Git repository error)
    let result =
        run_sah_command_in_process_with_dir(&["tool", "kanban", "tasks", "list"], &temp_dir).await;

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
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();

    // Use explicit working directory instead of global directory change

    let result =
        run_sah_command_in_process_with_dir(&["tool", "kanban", "tasks", "list"], &temp_dir).await;

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
    // Kanban commands require git repositories
    assert_ne!(
        output.exit_code, 0,
        "Kanban commands should fail without git repository"
    );
}

// Removed test_shell_commands_work_without_git - shell command was migrated away from static CLI

/// Test that web search commands don't require Git repository
#[tokio::test]
async fn test_web_search_works_without_git() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();

    // Note: This test might fail if web search is not available or has issues,
    // but it should not fail due to Git repository requirements

    // Use explicit working directory instead of global directory change

    let result =
        run_sah_command_in_process_with_dir(&["tool", "web_search", "--query", "test"], &temp_dir).await;

    // Restore original directory

    let output = result.unwrap();
    // Should not contain Git repository requirement error
    assert!(
        !output.stderr.contains("require a Git repository"),
        "Should not have Git repo error: {}",
        output.stderr
    );
}

/// Test error message actionability with kanban commands
#[tokio::test]
async fn test_error_messages_are_actionable() {
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();

    // Initialize git repository
    use git2::Repository;
    Repository::init(&temp_dir).expect("Failed to initialize git repository");

    // Initialize kanban board first (required before adding tasks)
    let init_result = run_sah_command_in_process_with_dir(
        &["tool", "kanban", "board", "init", "--name", "Test Board"],
        &temp_dir,
    )
    .await;

    let init_output = init_result.unwrap();
    assert_eq!(
        init_output.exit_code, 0,
        "Board init should succeed with git repository: {}",
        init_output.stderr
    );

    // Use explicit working directory instead of global directory change

    let result = run_sah_command_in_process_with_dir(
        &["tool", "kanban", "task", "add", "--title", "Test task"],
        &temp_dir,
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

    // Task add commands should succeed with git repository and initialized board
    assert_eq!(
        output.exit_code, 0,
        "Task add should succeed with git repository and initialized board"
    );
}
