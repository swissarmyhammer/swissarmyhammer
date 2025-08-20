//! Comprehensive CLI integration tests for memoranda functionality
//!
//! Tests all CLI memo commands including:
//! - Creating, reading, updating, deleting memos via CLI
//! - Listing and searching memos via CLI
//! - Getting context from all memos via CLI
//! - Stdin/stdout handling with different input formats
//! - Error exit codes validation
//! - Command completion and help text
//! - Unicode and special character handling
//! - Large content handling via CLI

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::io::Write;
use tempfile::{NamedTempFile, TempDir};

/// Helper to create a CLI command with environment setup
fn memo_cmd() -> Command {
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.env("SWISSARMYHAMMER_TEST_MODE", "1"); // Enable dynamic CLI for E2E tests
    cmd
}

/// Helper to create a memo command with custom memos directory
fn memo_cmd_with_dir(temp_dir: &TempDir) -> Command {
    let mut cmd = memo_cmd();
    cmd.env("SWISSARMYHAMMER_MEMOS_DIR", temp_dir.path().join("memos"));
    cmd
}

/// Extract memo ID from CLI output
fn extract_memo_id(output: &str) -> String {
    if let Some(start) = output.find("🆔 ID: ") {
        let id_start = start + "🆔 ID: ".len();
        if let Some(end) = output[id_start..].find('\n') {
            return output[id_start..id_start + end].trim().to_string();
        }
    }
    panic!("Could not extract memo ID from output: {output}");
}

#[test]
fn test_cli_memo_create_basic() {
    let temp_dir = TempDir::new().unwrap();

    memo_cmd_with_dir(&temp_dir)
        .args([
            "memo",
            "create",
            "--title",
            "Test Memo Title",
            "This is test content for the memo",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("✅ Created memo: Test Memo Title"))
        .stdout(predicate::str::contains("🆔 ID:"))
        .stdout(predicate::str::contains("📅 Created:"));
}

#[test]
fn test_cli_memo_create_without_content() {
    let temp_dir = TempDir::new().unwrap();

    // Create memo without --content flag, should prompt for stdin but fail in test environment
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "create", "--title", "No Content Memo", "-"])
        .write_stdin("")
        .assert()
        .success()
        .stdout(predicate::str::contains("✅ Created memo: No Content Memo"));
}

#[test]
fn test_cli_memo_create_with_stdin() {
    let temp_dir = TempDir::new().unwrap();

    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "create", "--title", "Stdin Memo", "-"])
        .write_stdin("Content from stdin\nMultiple lines\nOf text")
        .assert()
        .success()
        .stdout(predicate::str::contains("✅ Created memo: Stdin Memo"));
}

#[test]
fn test_cli_memo_create_unicode() {
    let temp_dir = TempDir::new().unwrap();

    memo_cmd_with_dir(&temp_dir)
        .args([
            "memo",
            "create",
            "--title",
            "🚀 Unicode Test with 中文",
            "Content with émojis 🎉 and unicode chars: ñáéíóú, 中文测试",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "✅ Created memo: 🚀 Unicode Test with 中文",
        ));
}

#[test]
fn test_cli_memo_create_empty_title() {
    let temp_dir = TempDir::new().unwrap();

    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "create", "--title", "", "Content with empty title"])
        .assert()
        .success()
        .stdout(predicate::str::contains("✅ Created memo:"));
}

#[test]
fn test_cli_memo_create_large_content() {
    let temp_dir = TempDir::new().unwrap();
    let large_content = "x".repeat(50_000); // 50KB content

    memo_cmd_with_dir(&temp_dir)
        .args([
            "memo",
            "create",
            "--title",
            "Large Content Memo",
            &large_content,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "✅ Created memo: Large Content Memo",
        ));
}

#[test]
fn test_cli_memo_list_empty() {
    let temp_dir = TempDir::new().unwrap();

    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ℹ️ No memos found"));
}

#[test]
fn test_cli_memo_list_with_memos() {
    let temp_dir = TempDir::new().unwrap();

    // Create a few memos first
    let memo_titles = ["First Memo", "Second Memo", "Third Memo"];
    for title in &memo_titles {
        memo_cmd_with_dir(&temp_dir)
            .args([
                "memo",
                "create",
                "--title",
                title,
                &format!("Content for {title}"),
            ])
            .assert()
            .success();
    }

    // Now list them
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("📝 Found 3 memos"))
        .stdout(predicate::str::contains("First Memo"))
        .stdout(predicate::str::contains("Second Memo"))
        .stdout(predicate::str::contains("Third Memo"))
        .stdout(predicate::str::contains("🆔"))
        .stdout(predicate::str::contains("📄"));
}

#[test]
fn test_cli_memo_get_basic() {
    let temp_dir = TempDir::new().unwrap();

    // Create a memo first
    let create_output = memo_cmd_with_dir(&temp_dir)
        .args([
            "memo",
            "create",
            "--title",
            "Get Test Memo",
            "Content for get test",
        ])
        .output()
        .unwrap();

    let create_stdout = String::from_utf8(create_output.stdout).unwrap();
    let memo_id = extract_memo_id(&create_stdout);

    // Now get the memo
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "get", &memo_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Get Test Memo"))
        .stdout(predicate::str::contains("Content for get test"))
        .stdout(predicate::str::contains(format!("🆔 ID: {memo_id}")))
        .stdout(predicate::str::contains("📅 Created:"))
        .stdout(predicate::str::contains("🔄 Updated:"));
}

#[test]
fn test_cli_memo_get_invalid_id() {
    let temp_dir = TempDir::new().unwrap();

    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "get", "invalid/memo/id"]) // Use forward slashes which are forbidden
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid memo ID format"));
}

#[test]
fn test_cli_memo_get_nonexistent() {
    let temp_dir = TempDir::new().unwrap();

    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "get", "01ARZ3NDEKTSV4RRFFQ69G5FAV"]) // Valid ULID format but doesn't exist
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_cli_memo_update_basic() {
    let temp_dir = TempDir::new().unwrap();

    // Create a memo first
    let create_output = memo_cmd_with_dir(&temp_dir)
        .args([
            "memo",
            "create",
            "--title",
            "Update Test Memo",
            "Original content",
        ])
        .output()
        .unwrap();

    let memo_id = extract_memo_id(&String::from_utf8(create_output.stdout).unwrap());

    // Update the memo
    memo_cmd_with_dir(&temp_dir)
        .args([
            "memo",
            "update",
            "--id",
            &memo_id,
            "Updated content via CLI",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Successfully updated memo:"))
        .stdout(predicate::str::contains("Update Test Memo"))
        .stdout(predicate::str::contains("Updated content via CLI"));
}

#[test]
fn test_cli_memo_update_with_stdin() {
    let temp_dir = TempDir::new().unwrap();

    // Create a memo first
    let create_output = memo_cmd_with_dir(&temp_dir)
        .args([
            "memo",
            "create",
            "--title",
            "Stdin Update Test",
            "Original content",
        ])
        .output()
        .unwrap();

    let memo_id = extract_memo_id(&String::from_utf8(create_output.stdout).unwrap());

    // Update with stdin
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "update", "--id", &memo_id, "-"])
        .write_stdin("Updated content from stdin\nWith multiple lines")
        .assert()
        .success()
        .stdout(predicate::str::contains("Successfully updated memo:"));
}

#[test]
fn test_cli_memo_update_invalid_id() {
    let temp_dir = TempDir::new().unwrap();

    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "update", "--id", "invalid*id", "New content"]) // Use asterisk which is forbidden
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid memo ID format"));
}

#[test]
fn test_cli_memo_update_nonexistent() {
    let temp_dir = TempDir::new().unwrap();

    memo_cmd_with_dir(&temp_dir)
        .args([
            "memo",
            "update",
            "--id",
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "New content",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_cli_memo_delete_basic() {
    let temp_dir = TempDir::new().unwrap();

    // Create a memo first
    let create_output = memo_cmd_with_dir(&temp_dir)
        .args([
            "memo",
            "create",
            "--title",
            "Delete Test Memo",
            "To be deleted",
        ])
        .output()
        .unwrap();

    let memo_id = extract_memo_id(&String::from_utf8(create_output.stdout).unwrap());

    // Delete the memo
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "delete", &memo_id])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Successfully deleted memo with ID",
        ))
        .stdout(predicate::str::contains(&memo_id));

    // Verify it's deleted by trying to get it
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "get", &memo_id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_cli_memo_delete_invalid_id() {
    let temp_dir = TempDir::new().unwrap();

    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "delete", "invalid?id"]) // Use question mark which is forbidden
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid memo ID format"));
}

#[test]
fn test_cli_memo_delete_nonexistent() {
    let temp_dir = TempDir::new().unwrap();

    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "delete", "01ARZ3NDEKTSV4RRFFQ69G5FAV"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_cli_memo_search_basic() {
    let temp_dir = TempDir::new().unwrap();

    // Create test memos
    let test_data = [
        ("Rust Programming", "Learning Rust language"),
        ("Python Guide", "Python programming tutorial"),
        ("JavaScript Basics", "Introduction to JavaScript"),
        ("Rust Advanced", "Advanced Rust concepts"),
    ];

    for (title, content) in &test_data {
        memo_cmd_with_dir(&temp_dir)
            .args(["memo", "create", "--title", title, content])
            .assert()
            .success();
    }

    // Search for "Rust" - should find 2 memos
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "search", "Rust"])
        .assert()
        .success()
        .stdout(predicate::str::contains("🔍 Found 2 memos matching 'Rust'"))
        .stdout(predicate::str::contains("Rust Programming"))
        .stdout(predicate::str::contains("Rust Advanced"));
}

#[test]
fn test_cli_memo_search_case_insensitive() {
    let temp_dir = TempDir::new().unwrap();

    // Create a memo with mixed case
    memo_cmd_with_dir(&temp_dir)
        .args([
            "memo",
            "create",
            "--title",
            "CamelCase Title",
            "Content with MixedCase words",
        ])
        .assert()
        .success();

    // Search with different cases
    let search_terms = ["camelcase", "MIXEDCASE", "MiXeDcAsE"];

    for term in &search_terms {
        memo_cmd_with_dir(&temp_dir)
            .args(["memo", "search", term])
            .assert()
            .success()
            .stdout(predicate::str::contains(format!(
                "🔍 Found 1 memo matching '{term}'"
            )));
    }
}

#[test]
fn test_cli_memo_search_no_results() {
    let temp_dir = TempDir::new().unwrap();

    // Create a memo
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "create", "--title", "Test Memo", "Test content"])
        .assert()
        .success();

    // Search for non-existent content
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "search", "nonexistent"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No memos found matching query: 'nonexistent'",
        ));
}

#[test]
fn test_cli_memo_search_empty_query() {
    let temp_dir = TempDir::new().unwrap();

    // Create a memo
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "create", "--title", "Test Memo", "Test content"])
        .assert()
        .success();

    // Search with empty query should fail
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "search", ""])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Search query cannot be empty"));
}

#[test]
fn test_cli_memo_search_special_characters() {
    let temp_dir = TempDir::new().unwrap();

    // Create memos with different types of special content
    memo_cmd_with_dir(&temp_dir)
        .args([
            "memo",
            "create",
            "--title",
            "Email Content",
            "Contact support at help@example.com",
        ])
        .assert()
        .success();

    memo_cmd_with_dir(&temp_dir)
        .args([
            "memo",
            "create",
            "--title",
            "Programming Notes",
            "Use C++ for performance-critical code",
        ])
        .assert()
        .success();

    memo_cmd_with_dir(&temp_dir)
        .args([
            "memo",
            "create",
            "--title",
            "File Path",
            "Config file located at /usr/local/bin/config",
        ])
        .assert()
        .success();

    // Search for email address
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "search", "help@example.com"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "🔍 Found 1 memo matching 'help@example.com'",
        ));

    // Search for C++ (with special characters)
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "search", "C++"])
        .assert()
        .success()
        .stdout(predicate::str::contains("🔍 Found 1 memo matching 'C++'"));

    // Search for file path with forward slashes
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "search", "/usr/local"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "🔍 Found 1 memo matching '/usr/local'",
        ));
}

#[test]
fn test_cli_memo_context_empty() {
    let temp_dir = TempDir::new().unwrap();

    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "get-all-context"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No memos available"));
}

#[test]
fn test_cli_memo_context_with_memos() {
    let temp_dir = TempDir::new().unwrap();

    // Create some memos
    for i in 1..=3 {
        memo_cmd_with_dir(&temp_dir)
            .args([
                "memo",
                "create",
                "--title",
                &format!("Context Memo {i}"),
                &format!("Context content for memo {i}"),
            ])
            .assert()
            .success();
    }

    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "get-all-context"])
        .assert()
        .success()
        .stdout(predicate::str::contains("All memo context (3 memos):"))
        .stdout(predicate::str::contains("Context Memo 1"))
        .stdout(predicate::str::contains("Context Memo 2"))
        .stdout(predicate::str::contains("Context Memo 3"))
        .stdout(predicate::str::contains("===")); // Context separators
}

#[test]
fn test_cli_memo_context_ordering() {
    let temp_dir = TempDir::new().unwrap();

    // Create memos with delays to ensure different timestamps
    for i in 1..=3 {
        memo_cmd_with_dir(&temp_dir)
            .args([
                "memo",
                "create",
                "--title",
                &format!("Ordered Memo {i}"),
                &format!("Content {i}"),
            ])
            .assert()
            .success();

        // Small delay to ensure different creation times
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    let output = memo_cmd_with_dir(&temp_dir)
        .args(["memo", "get-all-context"])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();

    // Newest memo should appear first in context
    let memo3_pos = stdout.find("Ordered Memo 3").unwrap();
    let memo2_pos = stdout.find("Ordered Memo 2").unwrap();
    let memo1_pos = stdout.find("Ordered Memo 1").unwrap();

    assert!(memo3_pos < memo2_pos);
    assert!(memo2_pos < memo1_pos);
}

#[test]
fn test_cli_memo_help() {
    memo_cmd()
        .args(["memo", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Commands:"))
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("get"))
        .stdout(predicate::str::contains("update"))
        .stdout(predicate::str::contains("delete"))
        .stdout(predicate::str::contains("search"))
        .stdout(predicate::str::contains("context"));
}

#[test]
fn test_cli_memo_create_help() {
    let temp_dir = TempDir::new().unwrap();
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "create", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Create a new memo"))
        .stdout(predicate::str::contains("<content>"));
}

#[test]
fn test_cli_memo_invalid_command() {
    memo_cmd()
        .args(["memo", "invalid-command"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("error:"));
}

#[test]
fn test_cli_memo_create_missing_title() {
    let temp_dir = TempDir::new().unwrap();
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "create"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_cli_memo_get_missing_id() {
    memo_cmd()
        .args(["memo", "get"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_cli_memo_update_missing_id() {
    memo_cmd()
        .args(["memo", "update"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_cli_memo_delete_missing_id() {
    memo_cmd()
        .args(["memo", "delete"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_cli_memo_search_missing_query() {
    memo_cmd()
        .args(["memo", "search"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_cli_memo_workflow() {
    let temp_dir = TempDir::new().unwrap();

    // Test complete workflow: create -> list -> get -> update -> search -> delete -> list

    // 1. Create a memo
    let create_output = memo_cmd_with_dir(&temp_dir)
        .args([
            "memo",
            "create",
            "--title",
            "Workflow Test",
            "Original workflow content",
        ])
        .output()
        .unwrap();

    assert!(create_output.status.success());
    let memo_id = extract_memo_id(&String::from_utf8(create_output.stdout).unwrap());

    // 2. List memos (should have 1)
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("📝 Found 1 memo"));

    // 3. Get the memo
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "get", &memo_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Workflow Test"))
        .stdout(predicate::str::contains("Original workflow content"));

    // 4. Update the memo
    memo_cmd_with_dir(&temp_dir)
        .args([
            "memo",
            "update",
            "--id",
            &memo_id,
            "Updated workflow content",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Successfully updated memo:"));

    // 5. Search for the memo
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "search", "workflow"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "🔍 Found 1 memo matching 'workflow'",
        ));

    // 6. Delete the memo
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "delete", &memo_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Successfully deleted memo"));

    // 7. List memos again (should be empty)
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ℹ️ No memos found"));
}

#[test]
fn test_cli_memo_content_with_newlines() {
    let temp_dir = TempDir::new().unwrap();

    let multiline_content = "Line 1\nLine 2\nLine 3\n\nLine 5 with empty line above";

    // Create memo with multiline content
    let create_output = memo_cmd_with_dir(&temp_dir)
        .args([
            "memo",
            "create",
            "--title",
            "Multiline Test",
            multiline_content,
        ])
        .output()
        .unwrap();

    assert!(create_output.status.success());
    let memo_id = extract_memo_id(&String::from_utf8(create_output.stdout).unwrap());

    // Retrieve and verify content
    let get_output = memo_cmd_with_dir(&temp_dir)
        .args(["memo", "get", &memo_id])
        .output()
        .unwrap();

    assert!(get_output.status.success());
    let get_stdout = String::from_utf8(get_output.stdout).unwrap();
    assert!(get_stdout.contains("Line 1"));
    assert!(get_stdout.contains("Line 2"));
    assert!(get_stdout.contains("Line 5 with empty line above"));
}

#[test]
fn test_cli_memo_special_title_characters() {
    let temp_dir = TempDir::new().unwrap();

    let special_titles = [
        "Title with \"quotes\"",
        "Title with 'apostrophes'",
        "Title with /forward/slashes",
        "Title with \\backslashes",
        "Title with <brackets>",
    ];

    for title in &special_titles {
        memo_cmd_with_dir(&temp_dir)
            .args(["memo", "create", "--title", title, "Special content"])
            .assert()
            .success()
            .stdout(predicate::str::contains(format!(
                "✅ Created memo: {title}"
            )));
    }

    // List all and verify they're all there
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Found 5 memos:"));
}

#[test]
fn test_cli_memo_concurrent_operations() {
    let temp_dir = TempDir::new().unwrap();

    // Create multiple memos rapidly
    let handles: Vec<_> = (1..=5)
        .map(|i| {
            let temp_dir_path = temp_dir.path().to_path_buf();
            std::thread::spawn(move || {
                let mut cmd = Command::cargo_bin("sah").unwrap();
                cmd.env("SWISSARMYHAMMER_TEST_MODE", "1"); // Enable dynamic CLI for E2E tests
                cmd.env("SWISSARMYHAMMER_MEMOS_DIR", temp_dir_path.join("memos"));
                cmd.args([
                    "memo",
                    "create",
                    "--title",
                    &format!("Concurrent Memo {i}"),
                    &format!("Content for concurrent memo {i}"),
                ])
                .assert()
                .success();
            })
        })
        .collect();

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all memos were created
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Found 5 memos:"));
}

#[test]
fn test_cli_memo_exit_codes() {
    let temp_dir = TempDir::new().unwrap();

    // Success cases should return 0
    memo_cmd_with_dir(&temp_dir)
        .args([
            "memo",
            "create",
            "--title",
            "Exit Code Test",
            "Test content",
        ])
        .assert()
        .code(0);

    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "list"])
        .assert()
        .code(0);

    // Error cases should return non-zero
    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "get", "invalid-id"])
        .assert()
        .code(predicate::ne(0));

    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "delete", "01ARZ3NDEKTSV4RRFFQ69G5FAV"])
        .assert()
        .code(predicate::ne(0));
}

#[test]
fn test_cli_memo_file_from_temp() {
    let temp_dir = TempDir::new().unwrap();
    let mut temp_file = NamedTempFile::new().unwrap();

    // Write content to temp file
    writeln!(temp_file, "Content from temporary file").unwrap();
    writeln!(temp_file, "With multiple lines").unwrap();
    writeln!(temp_file, "And special chars: éñüñ").unwrap();
    temp_file.flush().unwrap();

    // Create memo with content from file via stdin redirect
    let file_content = fs::read_to_string(temp_file.path()).unwrap();

    memo_cmd_with_dir(&temp_dir)
        .args(["memo", "create", "--title", "File Content Test", "-"])
        .write_stdin(file_content)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "✅ Created memo: File Content Test",
        ));
}

#[cfg(test)]
mod stress_tests {
    use super::*;

    /// Performance test: Create multiple memos rapidly via CLI
    #[test]
    fn test_cli_memo_create_many() {
        let temp_dir = TempDir::new().unwrap();
        let num_memos = 45;

        for i in 1..=num_memos {
            memo_cmd_with_dir(&temp_dir)
                .args([
                    "memo",
                    "create",
                    "--title",
                    &format!("Stress Test Memo {i}"),
                    &format!("Content for stress test memo {i} with additional text"),
                ])
                .assert()
                .success();
        }

        // Verify all were created
        memo_cmd_with_dir(&temp_dir)
            .args(["memo", "list"])
            .assert()
            .success()
            .stdout(predicate::str::contains(format!(
                "Found {num_memos} memos:"
            )));
    }
}
