//! Integration tests for Git repository error handling
//!
//! Tests that CLI commands provide clear, actionable error messages when run outside
//! Git repositories, with component-specific guidance for resolution.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Test that memo commands require Git repository
#[test]
fn test_memo_commands_require_git_repository() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    let mut cmd = Command::cargo_bin("sah").unwrap_or_else(|_| {
        // Fallback: try to find the binary by path
        Command::new(env!("CARGO_BIN_EXE_sah"))
    });
    cmd.current_dir(temp_dir.path())
        .args(["memo", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Memo operations require a Git repository"))
        .stderr(predicate::str::contains("Memos are stored in .swissarmyhammer/memos/"))
        .stderr(predicate::str::contains("git init"));
}

/// Test that issue commands require Git repository
#[test]
fn test_issue_commands_require_git_repository() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    let mut cmd = Command::cargo_bin("sah").unwrap_or_else(|_| {
        // Fallback: try to find the binary by path
        Command::new(env!("CARGO_BIN_EXE_sah"))
    });
    cmd.current_dir(temp_dir.path())
        .args(["issue", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Issue operations require a Git repository"))
        .stderr(predicate::str::contains("Issues are stored in .swissarmyhammer/issues/"))
        .stderr(predicate::str::contains("branch management"));
}

/// Test that search commands require Git repository
#[test]
fn test_search_commands_require_git_repository() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    let mut cmd = Command::cargo_bin("sah").unwrap_or_else(|_| {
        // Fallback: try to find the binary by path
        Command::new(env!("CARGO_BIN_EXE_sah"))
    });
    cmd.current_dir(temp_dir.path())
        .args(["search", "index", "**/*.rs"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Search indexing require a Git repository"))
        .stderr(predicate::str::contains("Search index is stored in .swissarmyhammer/semantic.db"));
}

/// Test that search query commands require Git repository
#[test]
fn test_search_query_requires_git_repository() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    let mut cmd = Command::cargo_bin("sah").unwrap_or_else(|_| {
        // Fallback: try to find the binary by path
        Command::new(env!("CARGO_BIN_EXE_sah"))
    });
    cmd.current_dir(temp_dir.path())
        .args(["search", "query", "test"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Search operations require a Git repository"))
        .stderr(predicate::str::contains("Search index is stored in .swissarmyhammer/semantic.db"));
}

/// Test error message format consistency
#[test]
fn test_error_message_format_consistency() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    // Test memo command error format
    let mut cmd = Command::cargo_bin("sah").unwrap_or_else(|_| {
        // Fallback: try to find the binary by path
        Command::new(env!("CARGO_BIN_EXE_sah"))
    });
    let output = cmd.current_dir(temp_dir.path())
        .args(["memo", "create", "test"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    
    let stderr = String::from_utf8_lossy(&output);
    
    // Check for consistent error format elements
    assert!(stderr.contains("❌"), "Error should start with ❌ icon");
    assert!(stderr.contains("Solutions:"), "Error should include Solutions section");
    assert!(stderr.contains("git init"), "Error should suggest git init");
    assert!(stderr.contains("Current directory:"), "Error should show current directory");
}

/// Test that commands work correctly within Git repository
#[test]
fn test_commands_work_in_git_repository() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    // Initialize git repository
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to initialize git repository");
    
    // Create .swissarmyhammer directory
    fs::create_dir_all(temp_dir.path().join(".swissarmyhammer")).expect("Failed to create directory");
    
    // Test that memo list command now works (or at least doesn't fail with Git repository error)
    let mut cmd = Command::cargo_bin("sah").unwrap_or_else(|_| {
        // Fallback: try to find the binary by path
        Command::new(env!("CARGO_BIN_EXE_sah"))
    });
    let result = cmd.current_dir(temp_dir.path())
        .args(["memo", "list"])
        .assert();
    
    // Should not contain Git repository requirement error
    result.stderr(predicate::str::contains("require a Git repository").not());
}

/// Test exit codes for Git repository errors
#[test]
fn test_git_repository_error_exit_codes() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    let mut cmd = Command::cargo_bin("sah").unwrap_or_else(|_| {
        // Fallback: try to find the binary by path
        Command::new(env!("CARGO_BIN_EXE_sah"))
    });
    cmd.current_dir(temp_dir.path())
        .args(["memo", "list"])
        .assert()
        .code(2); // EXIT_ERROR
}

/// Test that file commands don't require Git repository (should work)
#[test]
fn test_file_commands_work_without_git() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    // Create a test file
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "Hello, world!").expect("Failed to create test file");
    
    let mut cmd = Command::cargo_bin("sah").unwrap_or_else(|_| {
        // Fallback: try to find the binary by path
        Command::new(env!("CARGO_BIN_EXE_sah"))
    });
    cmd.current_dir(temp_dir.path())
        .args(["file", "read", test_file.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello, world!"));
}

/// Test that shell commands don't require Git repository
#[test]
fn test_shell_commands_work_without_git() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    let mut cmd = Command::cargo_bin("sah").unwrap_or_else(|_| {
        // Fallback: try to find the binary by path
        Command::new(env!("CARGO_BIN_EXE_sah"))
    });
    cmd.current_dir(temp_dir.path())
        .args(["shell", "execute", "echo test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("test"));
}

/// Test that web search commands don't require Git repository
#[test]
fn test_web_search_works_without_git() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    // Note: This test might fail if web search is not available or has issues,
    // but it should not fail due to Git repository requirements
    let mut cmd = Command::cargo_bin("sah").unwrap_or_else(|_| {
        // Fallback: try to find the binary by path
        Command::new(env!("CARGO_BIN_EXE_sah"))
    });
    let result = cmd.current_dir(temp_dir.path())
        .args(["web-search", "search", "test"])
        .assert();
    
    // Should not contain Git repository requirement error
    result.stderr(predicate::str::contains("require a Git repository").not());
}

/// Test error message actionability 
#[test]
fn test_error_messages_are_actionable() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    let mut cmd = Command::cargo_bin("sah").unwrap_or_else(|_| {
        // Fallback: try to find the binary by path
        Command::new(env!("CARGO_BIN_EXE_sah"))
    });
    let output = cmd.current_dir(temp_dir.path())
        .args(["issue", "create", "test"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    
    let stderr = String::from_utf8_lossy(&output);
    
    // Check that error messages provide actionable solutions
    assert!(stderr.contains("Solutions:"), "Should provide solutions section");
    assert!(stderr.contains("git init"), "Should suggest git init command");
    assert!(stderr.contains("git clone"), "Should suggest git clone option");
    assert!(stderr.contains("Current directory:"), "Should show current directory context");
}

/// Test error context preservation
#[test]
fn test_error_context_preservation() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    let mut cmd = Command::cargo_bin("sah").unwrap_or_else(|_| {
        // Fallback: try to find the binary by path
        Command::new(env!("CARGO_BIN_EXE_sah"))
    });
    let output = cmd.current_dir(temp_dir.path())
        .args(["memo", "get", "invalid_id"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    
    let stderr = String::from_utf8_lossy(&output);
    
    // Should contain Git repository error, not invalid ID error, since Git check happens first
    assert!(stderr.contains("Git repository"), "Should show Git repository error first");
}