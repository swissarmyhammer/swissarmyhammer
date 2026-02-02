//! Comprehensive Error Scenario Tests
//!
//! Tests for all major error conditions in CLI-MCP integration to ensure
//! proper error handling, user-friendly messages, and correct exit codes.

use anyhow::Result;
use swissarmyhammer::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_common::SwissarmyhammerDirectory;
use tempfile::TempDir;

use crate::in_process_test_utils::{run_sah_command_in_process_with_dir, CapturedOutput};
use crate::test_utils::setup_git_repo;

/// Setup function for error scenario testing using IsolatedTestEnvironment
fn setup_error_test_environment() -> Result<(IsolatedTestEnvironment, TempDir, std::path::PathBuf)>
{
    let home_guard = IsolatedTestEnvironment::new()?;
    let temp_dir = TempDir::new()?;
    let work_dir = temp_dir.path().to_path_buf();

    // Create basic directory structure in temporary directory
    let issues_dir = work_dir.join("issues");
    std::fs::create_dir_all(&issues_dir)?;

    // Create a sample issue for testing
    std::fs::write(
        issues_dir.join("ERROR_001_existing_issue.md"),
        r#"# Existing Issue

This issue exists for error scenario testing.
"#,
    )?;

    setup_git_repo(&work_dir)?;

    Ok((home_guard, temp_dir, work_dir))
}

/// Test invalid kanban operations
#[tokio::test]
async fn test_invalid_kanban_operations() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Test getting non-existent task
    // Use explicit working directory instead of global directory change to avoid race conditions
    let result = run_sah_command_in_process_with_dir(
        &["tool", "kanban", "task", "get", "--id", "nonexistent_id"],
        &temp_path,
    )
    .await?;
    assert_ne!(result.exit_code, 0, "Should fail for non-existent task");
    assert!(
        result.stderr.contains("Error")
            || result.stderr.contains("error")
            || result.stderr.contains("not found"),
        "Should show appropriate error message: {}",
        result.stderr
    );

    // Test completing non-existent task
    let complete_result = run_sah_command_in_process_with_dir(
        &["tool", "kanban", "task", "complete", "--id", "nonexistent_id"],
        &temp_path,
    )
    .await?;
    assert_ne!(
        complete_result.exit_code, 0,
        "Should fail for non-existent task complete"
    );
    assert!(
        complete_result.stderr.contains("Error")
            || complete_result.stderr.contains("error")
            || complete_result.stderr.contains("not found"),
        "Should show error for non-existent task completion: {}",
        complete_result.stderr
    );

    Ok(())
}

/// Test invalid command line arguments
#[tokio::test]
async fn test_invalid_command_arguments() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Test completely invalid command
    // Use explicit working directory instead of global directory change to avoid race conditions
    let invalid_cmd_result =
        run_sah_command_in_process_with_dir(&["completely", "invalid", "command"], &temp_path)
            .await?;
    assert_eq!(
        invalid_cmd_result.exit_code, 2,
        "Invalid command should return clap usage error code"
    );

    // Test invalid tool name
    let invalid_tool_result =
        run_sah_command_in_process_with_dir(&["tool", "invalid_tool_name"], &temp_path).await?;
    assert_ne!(
        invalid_tool_result.exit_code, 0,
        "Invalid tool should return error code"
    );

    // Test invalid flags for kanban tasks list
    let invalid_flag_result =
        run_sah_command_in_process_with_dir(&["tool", "kanban", "tasks", "list", "--invalid-flag"], &temp_path)
            .await?;
    assert_eq!(
        invalid_flag_result.exit_code, 2,
        "Invalid flag should return clap usage error code"
    );

    Ok(())
}

/// Test storage backend errors when .swissarmyhammer directory is read-only
#[tokio::test]
async fn test_storage_backend_permissions() -> Result<()> {
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt;

    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Create a read-only parent directory to cause storage errors
    let swissarmyhammer_dir = temp_path.join(SwissarmyhammerDirectory::dir_name());
    std::fs::remove_dir_all(&swissarmyhammer_dir).ok(); // Remove existing directory structure

    // Create the directory first
    if let Err(e) = std::fs::create_dir_all(&swissarmyhammer_dir) {
        println!("Failed to create swissarmyhammer dir: {}", e);
        return Ok(());
    }

    // Make the .swissarmyhammer directory read-only to prevent kanban directory creation
    if let Err(e) = std::fs::set_permissions(&swissarmyhammer_dir, Permissions::from_mode(0o555)) {
        println!("Failed to set permissions: {}", e);
        return Ok(());
    }

    // Test operations that require write access to .swissarmyhammer
    let result = run_sah_command_in_process_with_dir(
        &["tool", "kanban", "task", "add", "--title", "test"],
        &temp_path,
    )
    .await
    .unwrap_or_else(|e| CapturedOutput {
        stdout: String::new(),
        stderr: format!("Function error: {}", e),
        exit_code: 1,
    });

    // Restore permissions for cleanup before assertions (to avoid test cleanup issues)
    std::fs::set_permissions(&swissarmyhammer_dir, Permissions::from_mode(0o755))?;

    // Assert that the command failed with a permission or initialization error
    // Kanban operations may fail with "board not initialized" when the directory is read-only
    // because the board initialization cannot proceed
    assert_ne!(
        result.exit_code, 0,
        "Should fail when .swissarmyhammer directory is not writable. Exit code: {}, Stderr: {}",
        result.exit_code, result.stderr
    );
    assert!(
        result.stderr.contains("Permission denied")
            || result.stderr.contains("permission denied")
            || result.stderr.contains("IO error")
            || result.stderr.contains("board not initialized")
            || result.stderr.contains("not initialized"),
        "Should show permission-related or initialization error: {}",
        result.stderr
    );

    Ok(())
}

/// Test git-related functionality - verify kanban commands require git
#[tokio::test]
async fn test_commands_require_git() -> Result<()> {
    // Create a separate temporary directory without git for this test
    let temp_dir = tempfile::TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();

    // Test that kanban commands require git repository
    // Use explicit working directory instead of global directory change to avoid race conditions
    let result =
        run_sah_command_in_process_with_dir(&["tool", "kanban", "tasks", "list"], &temp_path)
            .await?;
    assert_ne!(
        result.exit_code, 0,
        "Kanban commands should fail without git repository"
    );
    assert!(
        result
            .stderr
            .contains("Kanban operations require a Git repository")
            || result.stderr.contains("Git repository")
            || result.stderr.contains("git"),
        "Should show git repository error: {}",
        result.stderr
    );

    Ok(())
}
/// Test resource exhaustion scenarios
#[tokio::test]
async fn test_resource_exhaustion() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Test with very large content (potential memory issues)
    let large_content = "A".repeat(1_000_000); // 1MB of content
                                               // Use explicit working directory instead of global directory change to avoid race conditions
    let result = run_sah_command_in_process_with_dir(
        &["tool", "kanban", "task", "add", "--title", &large_content],
        &temp_path,
    )
    .await?;

    // Should either succeed or fail gracefully (not crash)
    if result.exit_code != 0 {
        assert!(
            result.stderr.contains("Error")
                || result.stderr.contains("error")
                || result.stderr.contains("too large")
                || result.stderr.contains("memory"),
            "Large content errors should be handled gracefully: {}",
            result.stderr
        );
    }

    Ok(())
}

/// Test exit code consistency
#[tokio::test]
async fn test_exit_code_consistency() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Test that similar error conditions produce consistent exit codes
    let error_commands = vec![
        vec!["tool", "kanban", "task", "get", "--id", "nonexistent1"],
        vec!["tool", "kanban", "task", "get", "--id", "nonexistent2"],
        vec!["tool", "kanban", "task", "get", "--id", "nonexistent3"],
    ];

    let mut exit_codes = vec![];
    for cmd in error_commands {
        // Use explicit working directory instead of global directory change to avoid race conditions
        let result = run_sah_command_in_process_with_dir(&cmd, &temp_path).await?;
        assert_ne!(result.exit_code, 0, "Should fail for non-existent task");
        exit_codes.push(result.exit_code);
    }

    // All similar errors should have the same exit code
    let first_code = exit_codes[0];
    for code in &exit_codes {
        assert_eq!(
            *code, first_code,
            "Similar errors should have consistent exit codes"
        );
    }

    Ok(())
}

/// Test error message internationalization/localization readiness
#[tokio::test]
async fn test_error_message_consistency() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Test that error messages are consistent and informative
    // Use explicit working directory instead of global directory change to avoid race conditions
    let result = run_sah_command_in_process_with_dir(
        &[
            "tool",
            "kanban",
            "task",
            "get",
            "--id",
            "definitely_nonexistent_task",
        ],
        &temp_path,
    )
    .await?;
    assert_ne!(result.exit_code, 0, "Should fail for non-existent task");

    // Error messages should be:
    // 1. Informative (tell user what went wrong)
    // 2. Actionable (suggest what to do)
    // 3. Consistent in format
    assert!(
        result.stderr.len() > 10, // Should have substantial error message
        "Error messages should be informative: {}",
        result.stderr
    );

    assert!(
        result.stderr.contains("Error") || result.stderr.contains("error"),
        "Error messages should be clearly marked as errors: {}",
        result.stderr
    );

    // Should not contain technical jargon that users won't understand
    assert!(
        !result.stderr.contains("MCP")
            && !result.stderr.contains("toolContext")
            && !result.stderr.contains("NullPointer"),
        "Error messages should be user-friendly, not technical: {}",
        result.stderr
    );

    Ok(())
}
