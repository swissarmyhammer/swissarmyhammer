//! Comprehensive Error Scenario Tests
//!
//! Tests for all major error conditions in CLI-MCP integration to ensure
//! proper error handling, user-friendly messages, and correct exit codes.

use anyhow::Result;
use assert_cmd::Command;
use swissarmyhammer::test_utils::IsolatedTestHome;
use tempfile::TempDir;

mod test_utils;
use test_utils::setup_git_repo;

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
#[test]
fn test_invalid_issue_operations() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Test showing non-existent issue
    let output = Command::cargo_bin("sah")?
        .args(["issue", "show", "nonexistent_issue"])
        .current_dir(&temp_path)
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(
        stderr.contains("Error") || stderr.contains("error") || stderr.contains("not found"),
        "Should show appropriate error message: {stderr}"
    );

    // Test working on non-existent issue
    let output = Command::cargo_bin("sah")?
        .args(["issue", "work", "nonexistent_issue"])
        .current_dir(&temp_path)
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(
        stderr.contains("Error") || stderr.contains("error") || stderr.contains("not found"),
        "Should show error for non-existent issue work: {stderr}"
    );

    // Test completing non-existent issue
    let output = Command::cargo_bin("sah")?
        .args(["issue", "complete", "nonexistent_issue"])
        .current_dir(&temp_path)
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(
        stderr.contains("Error") || stderr.contains("error") || stderr.contains("not found"),
        "Should show error for non-existent issue completion: {stderr}"
    );

    // Test updating non-existent issue
    let output = Command::cargo_bin("sah")?
        .args([
            "issue",
            "update",
            "nonexistent_issue",
            "--content",
            "Updated content",
        ])
        .current_dir(&temp_path)
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(
        stderr.contains("Error") || stderr.contains("error") || stderr.contains("not found"),
        "Should show error for non-existent issue update: {stderr}"
    );

    Ok(())
}

/// Test invalid memo operations
#[test]
fn test_invalid_memo_operations() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Test getting memo with invalid ID
    let output = Command::cargo_bin("sah")?
        .args(["memo", "get", "invalid_memo_id"])
        .current_dir(&temp_path)
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(
        stderr.contains("Error")
            || stderr.contains("error")
            || stderr.contains("invalid")
            || stderr.contains("not found"),
        "Should show error for invalid memo ID: {stderr}"
    );

    // Test updating memo with invalid ID
    let output = Command::cargo_bin("sah")?
        .args([
            "memo",
            "update",
            "invalid_memo_id",
            "--content",
            "Updated content",
        ])
        .current_dir(&temp_path)
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(
        stderr.contains("Error")
            || stderr.contains("error")
            || stderr.contains("invalid")
            || stderr.contains("not found"),
        "Should show error for invalid memo update: {stderr}"
    );

    // Test deleting memo with invalid ID
    let output = Command::cargo_bin("sah")?
        .args(["memo", "delete", "invalid_memo_id"])
        .current_dir(&temp_path)
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(
        stderr.contains("Error")
            || stderr.contains("error")
            || stderr.contains("invalid")
            || stderr.contains("not found"),
        "Should show error for invalid memo deletion: {stderr}"
    );

    // Test creating memo without title
    let output = Command::cargo_bin("sah")?
        .args(["memo", "create"])
        .current_dir(&temp_path)
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(
        stderr.contains("required") || stderr.contains("missing") || stderr.contains("title"),
        "Should show error for missing memo title: {stderr}"
    );

    Ok(())
}

/// Test search error conditions (fast version - no ML model operations)
#[test]
fn test_search_error_conditions() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Test help command works for search - this is fast and doesn't trigger ML model downloads
    let help_output = Command::cargo_bin("sah")?
        .args(["search", "--help"])
        .current_dir(&temp_path)
        .env("RUST_LOG", "warn")
        .assert()
        .success();

    let help_stdout = String::from_utf8_lossy(&help_output.get_output().stdout);
    assert!(
        help_stdout.contains("search") && help_stdout.contains("index"),
        "Search help should contain subcommands: {help_stdout}"
    );

    // Test search index help - also fast
    let index_help_output = Command::cargo_bin("sah")?
        .args(["search", "index", "--help"])
        .current_dir(&temp_path)
        .env("RUST_LOG", "warn")
        .assert()
        .success();

    let index_help_stdout = String::from_utf8_lossy(&index_help_output.get_output().stdout);
    assert!(
        index_help_stdout.contains("patterns") && index_help_stdout.contains("force"),
        "Search index help should contain expected options: {index_help_stdout}"
    );

    // Test search query help - also fast
    let query_help_output = Command::cargo_bin("sah")?
        .args(["search", "query", "--help"])
        .current_dir(&temp_path)
        .env("RUST_LOG", "warn")
        .assert()
        .success();

    let query_help_stdout = String::from_utf8_lossy(&query_help_output.get_output().stdout);
    assert!(
        query_help_stdout.contains("query") && query_help_stdout.contains("limit"),
        "Search query help should contain expected options: {query_help_stdout}"
    );

    // Test invalid search command - should fail with proper error
    let invalid_output = Command::cargo_bin("sah")?
        .args(["search", "invalid_subcommand"])
        .current_dir(&temp_path)
        .assert()
        .failure()
        .code(2); // clap usage error

    let invalid_stderr = String::from_utf8_lossy(&invalid_output.get_output().stderr);
    assert!(
        invalid_stderr.contains("unrecognized subcommand") || invalid_stderr.contains("invalid"),
        "Invalid search subcommand should show proper error: {invalid_stderr}"
    );

    Ok(())
}

/// Test invalid command line arguments
#[test]
fn test_invalid_command_arguments() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Test completely invalid command
    Command::cargo_bin("sah")?
        .args(["completely", "invalid", "command"])
        .assert()
        .failure()
        .code(2); // clap returns 2 for usage errors

    // Test invalid subcommand
    Command::cargo_bin("sah")?
        .args(["issue", "invalid_subcommand"])
        .assert()
        .failure()
        .code(2);

    // Test invalid flags
    Command::cargo_bin("sah")?
        .args(["issue", "list", "--invalid-flag"])
        .assert()
        .failure()
        .code(2);

    // Test invalid format option - this should succeed since MCP doesn't validate format at CLI level
    let output = Command::cargo_bin("sah")?
        .args(["issue", "list", "--format", "invalid_format"])
        .current_dir(&temp_path)
        .assert()
        .failure()
        .code(2);

    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    // Should show clap usage error for invalid enum value
    assert!(
        stderr.contains("invalid value") || stderr.contains("possible values"),
        "Should show enum validation error: {stderr}"
    );

    Ok(())
}

/// Test storage backend errors
#[test]
fn test_storage_backend_errors() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Create a file where issues directory should be to cause storage errors
    let issues_path = temp_path.join("issues");
    std::fs::remove_dir_all(&issues_path).ok(); // Remove existing directory
    std::fs::write(&issues_path, "This is a file, not a directory")?;

    // Test operations that require issues directory
    let output = Command::cargo_bin("sah")?
        .args(["issue", "list"])
        .current_dir(&temp_path)
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(
        stderr.contains("Error")
            || stderr.contains("error")
            || stderr.contains("directory")
            || stderr.contains("storage"),
        "Should show storage-related error: {stderr}"
    );

    Ok(())
}

/// Test git-related errors
#[test]
fn test_git_related_errors() -> Result<()> {
    // Create a separate temporary directory without git for this test
    let temp_dir = tempfile::TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();

    // Create directory structure without git repository
    let issues_dir = temp_path.join("issues");
    std::fs::create_dir_all(&issues_dir)?;

    std::fs::write(
        issues_dir.join("GIT_001_test_issue.md"),
        "# Test Issue\n\nFor git error testing.",
    )?;

    // Test operations that might require git without git repository
    let output = Command::cargo_bin("sah")?
        .args(["issue", "work", "GIT_001_test_issue"])
        .current_dir(&temp_path)
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(
        stderr.contains("Error")
            || stderr.contains("error")
            || stderr.contains("git")
            || stderr.contains("repository"),
        "Should show git-related error: {stderr}"
    );

    Ok(())
}

/// Test concurrent operation errors
#[test]
fn test_concurrent_operation_errors() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // This is a basic test - true concurrency errors are hard to reproduce reliably
    // Test multiple rapid operations on the same resource
    let mut handles = vec![];

    for i in 0..3 {
        let temp_path_clone = temp_path.clone();
        let handle = std::thread::spawn(move || {
            Command::cargo_bin("sah")
                .unwrap()
                .args([
                    "issue",
                    "create",
                    &format!("concurrent_test_{i}"),
                    "--content",
                    &format!("Concurrent test issue {i}"),
                ])
                .current_dir(&temp_path_clone)
                .output()
        });
        handles.push(handle);
    }

    // Collect results - all should either succeed or fail gracefully
    for handle in handles {
        let result = handle.join().expect("Thread should complete");
        let output = result.expect("Command should execute");

        // Either succeeds or fails with appropriate error message
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("Error") || stderr.contains("error"),
                "Failed operations should have error messages: {stderr}"
            );
        }
    }

    Ok(())
}

/// Test resource exhaustion scenarios
#[test]
fn test_resource_exhaustion() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Test with very large content (potential memory issues)
    let large_content = "A".repeat(1_000_000); // 1MB of content
    let output = Command::cargo_bin("sah")?
        .args([
            "issue",
            "create",
            "large_content_test",
            "--content",
            &large_content,
        ])
        .current_dir(&temp_path)
        .assert();

    // Should either succeed or fail gracefully (not crash)
    if !output.get_output().status.success() {
        let stderr = String::from_utf8_lossy(&output.get_output().stderr);
        assert!(
            stderr.contains("Error")
                || stderr.contains("error")
                || stderr.contains("too large")
                || stderr.contains("memory"),
            "Large content errors should be handled gracefully: {stderr}"
        );
    }

    Ok(())
}

/// Test malformed input handling
#[test]
fn test_malformed_input_handling() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

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
        let output = Command::cargo_bin("sah")?
            .args([
                "issue",
                "create",
                name,
                "--content",
                "Test content with special name",
            ])
            .current_dir(&temp_path)
            .assert();

        // Should either succeed (if name is sanitized) or fail gracefully
        if !output.get_output().status.success() {
            let stderr = String::from_utf8_lossy(&output.get_output().stderr);
            assert!(
                stderr.contains("Error")
                    || stderr.contains("error")
                    || stderr.contains("invalid")
                    || stderr.contains("name"),
                "Invalid names should be handled gracefully: {stderr}"
            );
        }
    }

    Ok(())
}

/// Test timeout scenarios
#[test]
fn test_timeout_scenarios() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Test operations with very short timeouts
    // Note: This is primarily for operations that might hang

    let output = Command::cargo_bin("sah")?
        .args(["issue", "list"])
        .current_dir(&temp_path)
        .timeout(std::time::Duration::from_millis(100)) // Very short timeout
        .assert();

    // Should either complete quickly or timeout gracefully
    match output.get_output().status.code() {
        Some(code) => {
            // Normal completion or error
            if code != 0 {
                let stderr = String::from_utf8_lossy(&output.get_output().stderr);
                assert!(
                    stderr.contains("Error") || stderr.contains("error"),
                    "Errors should have appropriate messages: {stderr}"
                );
            }
        }
        None => {
            // Process was terminated (timeout) - this is acceptable
        }
    }

    Ok(())
}

/// Test exit code consistency
#[test]
fn test_exit_code_consistency() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Test that similar error conditions produce consistent exit codes
    let error_commands = vec![
        vec!["issue", "show", "nonexistent1"],
        vec!["issue", "show", "nonexistent2"],
        vec!["issue", "show", "nonexistent3"],
    ];

    let mut exit_codes = vec![];
    for cmd in error_commands {
        let output = Command::cargo_bin("sah")?
            .args(cmd)
            .current_dir(&temp_path)
            .assert()
            .failure();
        exit_codes.push(output.get_output().status.code());
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
#[test]
fn test_error_message_consistency() -> Result<()> {
    let (_home_guard, _temp_dir, temp_path) = setup_error_test_environment()?;

    // Test that error messages are consistent and informative
    let output = Command::cargo_bin("sah")?
        .args(["issue", "show", "definitely_nonexistent_issue"])
        .current_dir(&temp_path)
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&output.get_output().stderr);

    // Error messages should be:
    // 1. Informative (tell user what went wrong)
    // 2. Actionable (suggest what to do)
    // 3. Consistent in format
    assert!(
        stderr.len() > 10, // Should have substantial error message
        "Error messages should be informative: {stderr}"
    );

    assert!(
        stderr.contains("Error") || stderr.contains("error"),
        "Error messages should be clearly marked as errors: {stderr}"
    );

    // Should not contain technical jargon that users won't understand
    assert!(
        !stderr.contains("MCP")
            && !stderr.contains("toolContext")
            && !stderr.contains("NullPointer"),
        "Error messages should be user-friendly, not technical: {stderr}"
    );

    Ok(())
}
