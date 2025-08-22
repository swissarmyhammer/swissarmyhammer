//! Comprehensive Error Scenario Tests
//!
//! Tests for all major error conditions in CLI-MCP integration to ensure
//! proper error handling, user-friendly messages, and correct exit codes.

use anyhow::Result;
use swissarmyhammer::test_utils::IsolatedTestHome;
use tempfile::TempDir;

mod test_utils;
use test_utils::setup_git_repo;

mod in_process_test_utils;
use in_process_test_utils::run_sah_command_in_process;

/// Setup function for error scenario testing using IsolatedTestHome
fn setup_error_test_environment() -> Result<(IsolatedTestHome, TempDir, std::path::PathBuf)> {
    let home_guard = IsolatedTestHome::new();
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

/// Test invalid issue operations
#[tokio::test]
async fn test_invalid_issue_operations() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Change to temp directory for test
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(&temp_path)?;

    // Test showing non-existent issue
    let result = run_sah_command_in_process(&["issue", "show", "nonexistent_issue"]).await?;
    assert_ne!(result.exit_code, 0, "Should fail for non-existent issue");
    assert!(
        result.stderr.contains("Error")
            || result.stderr.contains("error")
            || result.stderr.contains("not found"),
        "Should show appropriate error message: {}",
        result.stderr
    );

    // Test working on non-existent issue
    let work_result = run_sah_command_in_process(&["issue", "work", "nonexistent_issue"]).await?;
    assert_ne!(
        work_result.exit_code, 0,
        "Should fail for non-existent issue work"
    );
    assert!(
        work_result.stderr.contains("Error")
            || work_result.stderr.contains("error")
            || work_result.stderr.contains("not found"),
        "Should show error for non-existent issue work: {}",
        work_result.stderr
    );

    // Test completing non-existent issue
    let complete_result =
        run_sah_command_in_process(&["issue", "complete", "nonexistent_issue"]).await?;
    assert_ne!(
        complete_result.exit_code, 0,
        "Should fail for non-existent issue complete"
    );
    assert!(
        complete_result.stderr.contains("Error")
            || complete_result.stderr.contains("error")
            || complete_result.stderr.contains("not found"),
        "Should show error for non-existent issue completion: {}",
        complete_result.stderr
    );

    // Test updating non-existent issue
    let update_result = run_sah_command_in_process(&[
        "issue",
        "update",
        "nonexistent_issue",
        "--content",
        "Updated content",
    ])
    .await?;
    assert_ne!(
        update_result.exit_code, 0,
        "Should fail for non-existent issue update"
    );
    assert!(
        update_result.stderr.contains("Error")
            || update_result.stderr.contains("error")
            || update_result.stderr.contains("not found"),
        "Should show error for non-existent issue update: {}",
        update_result.stderr
    );

    // Restore original directory
    std::env::set_current_dir(original_dir)?;

    Ok(())
}

/// Test invalid memo operations - DISABLED: Memo commands only available with dynamic-cli feature
// #[tokio::test]
// #[ignore = "Memo commands only available with dynamic-cli feature"]
async fn _test_invalid_memo_operations_disabled() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Change to temp directory for test
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(&temp_path)?;

    // Test getting memo with invalid ID
    let get_result = run_sah_command_in_process(&["memo", "get", "invalid_memo_id"]).await?;
    assert_ne!(get_result.exit_code, 0, "Should fail for invalid memo ID");
    assert!(
        get_result.stderr.contains("Error")
            || get_result.stderr.contains("error")
            || get_result.stderr.contains("invalid")
            || get_result.stderr.contains("not found"),
        "Should show error for invalid memo ID: {}",
        get_result.stderr
    );

    // Test updating memo with invalid ID
    let update_result = run_sah_command_in_process(&[
        "memo",
        "update",
        "invalid_memo_id",
        "--content",
        "Updated content",
    ])
    .await?;
    assert_ne!(
        update_result.exit_code, 0,
        "Should fail for invalid memo update"
    );
    assert!(
        update_result.stderr.contains("Error")
            || update_result.stderr.contains("error")
            || update_result.stderr.contains("invalid")
            || update_result.stderr.contains("not found"),
        "Should show error for invalid memo update: {}",
        update_result.stderr
    );

    // Test deleting memo with invalid ID
    let delete_result = run_sah_command_in_process(&["memo", "delete", "invalid_memo_id"]).await?;
    assert_ne!(
        delete_result.exit_code, 0,
        "Should fail for invalid memo deletion"
    );
    assert!(
        delete_result.stderr.contains("Error")
            || delete_result.stderr.contains("error")
            || delete_result.stderr.contains("invalid")
            || delete_result.stderr.contains("not found"),
        "Should show error for invalid memo deletion: {}",
        delete_result.stderr
    );

    // Test creating memo without title
    let create_result = run_sah_command_in_process(&["memo", "create"]).await?;
    assert_ne!(
        create_result.exit_code, 0,
        "Should fail for missing memo title"
    );
    assert!(
        create_result.stderr.contains("required")
            || create_result.stderr.contains("missing")
            || create_result.stderr.contains("title"),
        "Should show error for missing memo title: {}",
        create_result.stderr
    );

    // Restore original directory
    std::env::set_current_dir(original_dir)?;

    Ok(())
}

/// Test search command migration - search commands moved to dynamic CLI
#[tokio::test]
async fn test_search_error_conditions() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Change to temp directory for test
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(&temp_path)?;

    // With static CLI (default), search commands should not be available
    // This tests the successful migration of search commands to dynamic CLI
    let help_result = run_sah_command_in_process(&["search", "--help"]).await?;
    assert_eq!(
        help_result.exit_code, 2,
        "Search command should not exist in static CLI mode (exit code 2 = command not found)"
    );
    assert!(
        help_result.stderr.contains("unrecognized subcommand")
            || help_result.stderr.contains("invalid")
            || help_result
                .stderr
                .contains("error: unrecognized subcommand 'search'"),
        "Error message should indicate search command is not available: {}",
        help_result.stderr
    );

    // Test that main help doesn't contain standalone "search" command in static mode
    // Check that there's no line that starts with "  search " (the exact format for commands)
    let main_help_result = run_sah_command_in_process(&["--help"]).await?;
    assert_eq!(main_help_result.exit_code, 0, "Main help should succeed");
    assert!(
        !main_help_result
            .stdout
            .lines()
            .any(|line| line.trim().starts_with("search ")),
        "Main help should not contain search commands in static CLI mode (web-search is OK): {}",
        main_help_result.stdout
    );

    // Verify other commands still work (sanity check that CLI isn't completely broken)
    let validate_help_result = run_sah_command_in_process(&["validate", "--help"]).await?;
    assert_eq!(
        validate_help_result.exit_code, 0,
        "Validate help should still work"
    );

    // Restore original directory
    std::env::set_current_dir(original_dir)?;

    Ok(())
}

/// Test invalid command line arguments
#[tokio::test]
async fn test_invalid_command_arguments() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Change to temp directory for test
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(&temp_path)?;

    // Test completely invalid command
    let invalid_cmd_result =
        run_sah_command_in_process(&["completely", "invalid", "command"]).await?;
    assert_eq!(
        invalid_cmd_result.exit_code, 2,
        "Invalid command should return clap usage error code"
    );

    // Test invalid subcommand
    let invalid_sub_result = run_sah_command_in_process(&["issue", "invalid_subcommand"]).await?;
    assert_eq!(
        invalid_sub_result.exit_code, 2,
        "Invalid subcommand should return clap usage error code"
    );

    // Test invalid flags
    let invalid_flag_result =
        run_sah_command_in_process(&["issue", "list", "--invalid-flag"]).await?;
    assert_eq!(
        invalid_flag_result.exit_code, 2,
        "Invalid flag should return clap usage error code"
    );

    // Test invalid format option - this should succeed since MCP doesn't validate format at CLI level
    let invalid_format_result =
        run_sah_command_in_process(&["issue", "list", "--format", "invalid_format"]).await?;
    assert_eq!(
        invalid_format_result.exit_code, 2,
        "Invalid format should return clap usage error code"
    );
    // Should show clap usage error for invalid enum value
    assert!(
        invalid_format_result.stderr.contains("invalid value")
            || invalid_format_result.stderr.contains("possible values"),
        "Should show enum validation error: {}",
        invalid_format_result.stderr
    );

    // Restore original directory
    std::env::set_current_dir(original_dir)?;

    Ok(())
}

/// Test storage backend errors
#[tokio::test]
async fn test_storage_backend_errors() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Change to temp directory for test
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(&temp_path)?;

    // Create a file where issues directory should be to cause storage errors
    let issues_path = temp_path.join("issues");
    std::fs::remove_dir_all(&issues_path).ok(); // Remove existing directory
    std::fs::write(&issues_path, "This is a file, not a directory")?;

    // Test operations that require issues directory
    let result = run_sah_command_in_process(&["issue", "list"]).await?;
    assert_ne!(
        result.exit_code, 0,
        "Should fail when issues directory is not accessible"
    );
    assert!(
        result.stderr.contains("Error")
            || result.stderr.contains("error")
            || result.stderr.contains("directory")
            || result.stderr.contains("storage"),
        "Should show storage-related error: {}",
        result.stderr
    );

    // Restore original directory
    std::env::set_current_dir(original_dir)?;

    Ok(())
}

/// Test git-related errors
#[tokio::test]
async fn test_git_related_errors() -> Result<()> {
    // Create a separate temporary directory without git for this test
    let temp_dir = tempfile::TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();

    // Change to temp directory for test
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(&temp_path)?;

    // Create directory structure without git repository
    let issues_dir = temp_path.join("issues");
    std::fs::create_dir_all(&issues_dir)?;

    std::fs::write(
        issues_dir.join("GIT_001_test_issue.md"),
        "# Test Issue\n\nFor git error testing.",
    )?;

    // Test operations that might require git without git repository
    let result = run_sah_command_in_process(&["issue", "work", "GIT_001_test_issue"]).await?;
    assert_ne!(
        result.exit_code, 0,
        "Should fail when git repository is not available"
    );
    assert!(
        result.stderr.contains("Error")
            || result.stderr.contains("error")
            || result.stderr.contains("git")
            || result.stderr.contains("repository"),
        "Should show git-related error: {}",
        result.stderr
    );

    // Restore original directory
    std::env::set_current_dir(original_dir)?;

    Ok(())
}
/// Test resource exhaustion scenarios
#[tokio::test]
async fn test_resource_exhaustion() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Change to temp directory for test
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(&temp_path)?;

    // Test with very large content (potential memory issues)
    let large_content = "A".repeat(1_000_000); // 1MB of content
    let result = run_sah_command_in_process(&[
        "issue",
        "create",
        "large_content_test",
        "--content",
        &large_content,
    ])
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

    // Restore original directory
    std::env::set_current_dir(original_dir)?;

    Ok(())
}

/// Test malformed input handling
#[tokio::test]
async fn test_malformed_input_handling() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Change to temp directory for test
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(&temp_path)?;

    // Test with special characters in issue names
    let special_names = vec![
        "issue/with/slashes",
        "issue\\with\\backslashes",
        "issue with spaces",
        "issue..with..dots",
        "issue|with|pipes",
        "issue\"with\"quotes",
        "issue'with'apostrophes",
        "issue<with>brackets",
        "issue{with}braces",
        "issue[with]square",
    ];

    for name in special_names {
        let result = run_sah_command_in_process(&[
            "issue",
            "create",
            name,
            "--content",
            "Test content with special name",
        ])
        .await?;

        // Should either succeed (if name is sanitized) or fail gracefully
        if result.exit_code != 0 {
            assert!(
                result.stderr.contains("Error")
                    || result.stderr.contains("error")
                    || result.stderr.contains("invalid")
                    || result.stderr.contains("name"),
                "Invalid names should be handled gracefully: {}",
                result.stderr
            );
        }
    }

    // Restore original directory
    std::env::set_current_dir(original_dir)?;

    Ok(())
}

/// Test exit code consistency
#[tokio::test]
async fn test_exit_code_consistency() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Change to temp directory for test
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(&temp_path)?;

    // Test that similar error conditions produce consistent exit codes
    let error_commands = vec![
        vec!["issue", "show", "nonexistent1"],
        vec!["issue", "show", "nonexistent2"],
        vec!["issue", "show", "nonexistent3"],
    ];

    let mut exit_codes = vec![];
    for cmd in error_commands {
        let result = run_sah_command_in_process(&cmd).await?;
        assert_ne!(result.exit_code, 0, "Should fail for non-existent issue");
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

    // Restore original directory
    std::env::set_current_dir(original_dir)?;

    Ok(())
}

/// Test error message internationalization/localization readiness
#[tokio::test]
async fn test_error_message_consistency() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Change to temp directory for test
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(&temp_path)?;

    // Test that error messages are consistent and informative
    let result =
        run_sah_command_in_process(&["issue", "show", "definitely_nonexistent_issue"]).await?;
    assert_ne!(result.exit_code, 0, "Should fail for non-existent issue");

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

    // Restore original directory
    std::env::set_current_dir(original_dir)?;

    Ok(())
}
