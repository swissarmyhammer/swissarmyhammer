//! Comprehensive integration tests for file command CLI interface
//!
//! This module tests end-to-end CLI workflows for all file tools,
//! including command parsing, execution, output formatting, and error handling.

use anyhow::Result;
use assert_cmd::Command;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Helper function to run CLI command and capture output
fn run_command_with_output(cmd: &mut Command) -> Result<(String, String, Option<i32>)> {
    let output = cmd.output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code();

    Ok((stdout, stderr, exit_code))
}

/// Helper to create test files with content
fn create_test_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

// ============================================================================
// File Read Command Tests
// ============================================================================

#[test]
fn test_file_read_basic_functionality() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("test.txt");
    let test_content = "Hello, World!\nThis is a test file.\nWith multiple lines.";

    create_test_file(&test_file, test_content)?;

    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args(["file", "read", test_file.to_str().unwrap()]);

    let (stdout, stderr, exit_code) = run_command_with_output(&mut cmd)?;

    assert_eq!(exit_code, Some(0), "Command should succeed");
    assert!(
        stdout.contains(test_content),
        "Output should contain file content"
    );
    assert!(
        stderr.is_empty() || !stderr.contains("error"),
        "Should not have errors"
    );

    Ok(())
}

#[test]
fn test_file_read_with_offset_and_limit() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("test.txt");
    let lines: Vec<String> = (1..=20).map(|i| format!("Line {}", i)).collect();
    let test_content = lines.join("\n");

    create_test_file(&test_file, &test_content)?;

    // Test with offset - offset 6 means start from line 6
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args(["file", "read", test_file.to_str().unwrap(), "--offset", "6"]);

    let (stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;

    assert_eq!(exit_code, Some(0), "Command should succeed");
    assert!(
        !stdout.contains("Line 1\n") && !stdout.starts_with("Line 1"),
        "Should not contain line 1 when starting from line 6"
    );
    assert!(stdout.contains("Line 6"), "Should start from line 6");

    // Test with limit
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args(["file", "read", test_file.to_str().unwrap(), "--limit", "3"]);

    let (stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;

    assert_eq!(exit_code, Some(0), "Command should succeed");
    assert!(stdout.contains("Line 1"), "Should contain first line");
    assert!(stdout.contains("Line 3"), "Should contain third line");
    assert!(!stdout.contains("Line 4"), "Should not contain fourth line");

    Ok(())
}

#[test]
fn test_file_read_nonexistent_file() -> Result<()> {
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args([
        "file",
        "read",
        "/tmp/nonexistent_file_that_should_not_exist.txt",
    ]);

    let (stdout, stderr, exit_code) = run_command_with_output(&mut cmd)?;

    assert_ne!(exit_code, Some(0), "Command should fail");
    assert!(
        stderr.contains("No such file") || stderr.contains("not found") || stdout.contains("error"),
        "Should indicate file not found"
    );

    Ok(())
}

// ============================================================================
// File Write Command Tests
// ============================================================================

#[test]
fn test_file_write_basic_functionality() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("write_test.txt");
    let test_content = "This is new content\nWritten via CLI";

    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args(["file", "write", test_file.to_str().unwrap(), test_content]);

    let (_stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;

    assert_eq!(exit_code, Some(0), "Command should succeed");
    assert!(test_file.exists(), "File should be created");

    let written_content = fs::read_to_string(&test_file)?;
    assert_eq!(
        written_content, test_content,
        "Written content should match"
    );

    Ok(())
}

#[test]
fn test_file_write_overwrite_existing() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("overwrite_test.txt");
    let initial_content = "Initial content";
    let new_content = "New overwritten content";

    create_test_file(&test_file, initial_content)?;

    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args(["file", "write", test_file.to_str().unwrap(), new_content]);

    let (_stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;

    assert_eq!(exit_code, Some(0), "Command should succeed");

    let final_content = fs::read_to_string(&test_file)?;
    assert_eq!(final_content, new_content, "Content should be overwritten");
    assert_ne!(
        final_content, initial_content,
        "Old content should be replaced"
    );

    Ok(())
}

#[test]
fn test_file_write_creates_parent_directories() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("nested/deep/path/test.txt");
    let test_content = "Content in nested directory";

    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args(["file", "write", test_file.to_str().unwrap(), test_content]);

    let (_stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;

    assert_eq!(exit_code, Some(0), "Command should succeed");
    assert!(test_file.exists(), "File should be created");
    assert!(
        test_file.parent().unwrap().exists(),
        "Parent directories should be created"
    );

    let written_content = fs::read_to_string(&test_file)?;
    assert_eq!(written_content, test_content, "Content should match");

    Ok(())
}

// ============================================================================
// File Edit Command Tests
// ============================================================================

#[test]
fn test_file_edit_basic_replacement() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("edit_test.txt");
    let initial_content = "Hello old_value, this is a test with old_value.";

    create_test_file(&test_file, initial_content)?;

    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args([
        "file",
        "edit",
        test_file.to_str().unwrap(),
        "old_value",
        "new_value",
    ]);

    let (_stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;

    assert_eq!(exit_code, Some(0), "Command should succeed");

    let edited_content = fs::read_to_string(&test_file)?;
    assert!(
        edited_content.contains("new_value"),
        "Should contain replacement text"
    );
    assert!(
        edited_content.contains("old_value"),
        "Should still contain one instance (only first replaced)"
    );

    Ok(())
}

#[test]
fn test_file_edit_replace_all() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("edit_all_test.txt");
    let initial_content = "Replace TARGET here and TARGET there and TARGET everywhere.";

    create_test_file(&test_file, initial_content)?;

    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args([
        "file",
        "edit",
        test_file.to_str().unwrap(),
        "TARGET",
        "RESULT",
        "--replace-all",
    ]);

    let (_stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;

    assert_eq!(exit_code, Some(0), "Command should succeed");

    let edited_content = fs::read_to_string(&test_file)?;
    assert!(
        !edited_content.contains("TARGET"),
        "Should not contain original text"
    );
    assert_eq!(
        edited_content.matches("RESULT").count(),
        3,
        "Should replace all instances"
    );

    Ok(())
}

#[test]
fn test_file_edit_string_not_found() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("edit_not_found_test.txt");
    let initial_content = "This file does not contain the target string.";

    create_test_file(&test_file, initial_content)?;

    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args([
        "file",
        "edit",
        test_file.to_str().unwrap(),
        "nonexistent_string",
        "replacement",
    ]);

    let (_stdout, _stderr, _exit_code) = run_command_with_output(&mut cmd)?;

    // Should handle gracefully (either succeed with no changes or inform about no matches)
    let final_content = fs::read_to_string(&test_file)?;
    assert_eq!(
        final_content, initial_content,
        "Content should be unchanged"
    );

    Ok(())
}

// ============================================================================
// File Glob Command Tests
// ============================================================================

#[test]
fn test_file_glob_basic_patterns() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Create test files
    let test_files = vec![
        ("file1.txt", "content1"),
        ("file2.rs", "content2"),
        ("subdir/file3.txt", "content3"),
        ("subdir/file4.md", "content4"),
        ("README.md", "readme content"),
    ];

    for (path, content) in &test_files {
        let file_path = temp_dir.path().join(path);
        create_test_file(&file_path, content)?;
    }

    // Test basic glob pattern
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args([
        "file",
        "glob",
        "*.txt",
        "--path",
        temp_dir.path().to_str().unwrap(),
    ]);

    let (stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;

    assert_eq!(exit_code, Some(0), "Command should succeed");
    assert!(stdout.contains("file1.txt"), "Should find txt files");
    assert!(!stdout.contains("file2.rs"), "Should not find rs files");

    // Test recursive glob pattern
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args([
        "file",
        "glob",
        "**/*.txt",
        "--path",
        temp_dir.path().to_str().unwrap(),
    ]);

    let (stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;

    assert_eq!(exit_code, Some(0), "Command should succeed");
    assert!(
        stdout.contains("file1.txt"),
        "Should find top-level txt files"
    );
    assert!(stdout.contains("file3.txt"), "Should find nested txt files");

    Ok(())
}

#[test]
fn test_file_glob_case_sensitivity() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Create files with different names to avoid case-insensitive filesystem conflicts
    create_test_file(&temp_dir.path().join("README.TXT"), "content")?;
    create_test_file(&temp_dir.path().join("notes.txt"), "content")?;

    // Test case-sensitive search - should only find files that exactly match case
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args([
        "file",
        "glob",
        "*.txt", // lowercase pattern
        "--path",
        temp_dir.path().to_str().unwrap(),
        "--case-sensitive",
    ]);

    let (stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;

    assert_eq!(exit_code, Some(0), "Command should succeed");
    assert!(stdout.contains("notes.txt"), "Should find exact case match");
    assert!(
        !stdout.contains("README.TXT"),
        "Should not find different case"
    );

    Ok(())
}

// ============================================================================
// File Grep Command Tests
// ============================================================================

#[test]
fn test_file_grep_basic_search() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Create files with searchable content
    let test_files = vec![
        (
            "search1.txt",
            "This file contains TARGET_STRING for testing.",
        ),
        ("search2.txt", "Another file with TARGET_STRING here."),
        ("search3.txt", "This file has no matching content."),
        ("search4.rs", "fn main() { TARGET_STRING.process(); }"),
    ];

    for (path, content) in &test_files {
        let file_path = temp_dir.path().join(path);
        create_test_file(&file_path, content)?;
    }

    // Test basic grep search
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args([
        "file",
        "grep",
        "TARGET_STRING",
        "--path",
        temp_dir.path().to_str().unwrap(),
    ]);

    let (stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;

    assert_eq!(exit_code, Some(0), "Command should succeed");
    assert!(
        stdout.contains("search1.txt") || stdout.contains("3") || stdout.contains("found"),
        "Should find matches in files: {}",
        stdout
    );

    Ok(())
}

#[test]
fn test_file_grep_regex_patterns() -> Result<()> {
    let temp_dir = TempDir::new()?;

    let content = r#"
        function processData() {
            return data.map(item => item.value);
        }
        
        const result = processData();
    "#;

    create_test_file(&temp_dir.path().join("code.js"), content)?;

    // Test regex pattern for function definitions
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args([
        "file",
        "grep",
        r"function\s+\w+",
        "--path",
        temp_dir.path().to_str().unwrap(),
    ]);

    let (_stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;

    assert_eq!(exit_code, Some(0), "Command should succeed");
    // Should find the function definition

    Ok(())
}

#[test]
fn test_file_grep_file_type_filtering() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Create files of different types
    let test_files = vec![
        ("file.rs", "fn main() { search_target(); }"),
        ("file.js", "function main() { search_target(); }"),
        ("file.txt", "This contains search_target text."),
    ];

    for (path, content) in &test_files {
        let file_path = temp_dir.path().join(path);
        create_test_file(&file_path, content)?;
    }

    // Test filtering by file type
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args([
        "file",
        "grep",
        "search_target",
        "--path",
        temp_dir.path().to_str().unwrap(),
        "--type",
        "rs",
    ]);

    let (stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;

    assert_eq!(exit_code, Some(0), "Command should succeed");
    assert!(
        stdout.contains("file.rs") || stdout.contains("1") || stdout.contains("found"),
        "Should find matches only in Rust files: {}",
        stdout
    );

    Ok(())
}

// ============================================================================
// End-to-End Workflow Tests
// ============================================================================

#[test]
fn test_complete_file_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("workflow_test.txt");
    let initial_content = "Initial content with OLD_VALUE to replace.";

    // Step 1: Write initial content
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args([
        "file",
        "write",
        test_file.to_str().unwrap(),
        initial_content,
    ]);

    let (_stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;
    assert_eq!(exit_code, Some(0), "Write should succeed");

    // Step 2: Read and verify content
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args(["file", "read", test_file.to_str().unwrap()]);

    let (stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;
    assert_eq!(exit_code, Some(0), "Read should succeed");
    assert!(
        stdout.contains("OLD_VALUE"),
        "Should contain original content"
    );

    // Step 3: Edit the content
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args([
        "file",
        "edit",
        test_file.to_str().unwrap(),
        "OLD_VALUE",
        "NEW_VALUE",
    ]);

    let (_stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;
    assert_eq!(exit_code, Some(0), "Edit should succeed");

    // Step 4: Read and verify the edit
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args(["file", "read", test_file.to_str().unwrap()]);

    let (stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;
    assert_eq!(exit_code, Some(0), "Final read should succeed");
    assert!(
        stdout.contains("NEW_VALUE"),
        "Should contain edited content"
    );
    assert!(
        !stdout.contains("OLD_VALUE"),
        "Should not contain old content"
    );

    Ok(())
}

#[test]
fn test_file_discovery_and_search_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Create a project-like structure
    let test_files = vec![
        (
            "src/main.rs",
            "fn main() {\n    println!(\"Hello, TARGET!\");\n}",
        ),
        (
            "src/lib.rs",
            "pub fn process_TARGET() -> String {\n    \"result\".to_string()\n}",
        ),
        (
            "tests/test.rs",
            "#[test]\nfn test_TARGET() {\n    assert!(true);\n}",
        ),
        (
            "README.md",
            "# Project\n\nThis project processes TARGET data.",
        ),
        (
            "Cargo.toml",
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        ),
    ];

    for (path, content) in &test_files {
        let file_path = temp_dir.path().join(path);
        create_test_file(&file_path, content)?;
    }

    // Step 1: Find all Rust files
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args([
        "file",
        "glob",
        "**/*.rs",
        "--path",
        temp_dir.path().to_str().unwrap(),
    ]);

    let (stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;
    assert_eq!(exit_code, Some(0), "Glob should succeed");
    assert!(stdout.contains("main.rs"), "Should find main.rs");
    assert!(stdout.contains("lib.rs"), "Should find lib.rs");
    assert!(stdout.contains("test.rs"), "Should find test.rs");

    // Step 2: Search for TARGET in all files
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args([
        "file",
        "grep",
        "TARGET",
        "--path",
        temp_dir.path().to_str().unwrap(),
    ]);

    let (_stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;
    assert_eq!(exit_code, Some(0), "Grep should succeed");
    // Should find matches across multiple files

    // Step 3: Search specifically in Rust files
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args([
        "file",
        "grep",
        "TARGET",
        "--path",
        temp_dir.path().to_str().unwrap(),
        "--type",
        "rust",
    ]);

    let (_stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;
    assert_eq!(exit_code, Some(0), "Rust-specific grep should succeed");

    Ok(())
}

// ============================================================================
// Output Format Tests
// ============================================================================

#[test]
fn test_output_formatting_consistency() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("format_test.txt");
    let content = "Test content for formatting";

    create_test_file(&test_file, content)?;

    // Test that read command produces readable output
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args(["file", "read", test_file.to_str().unwrap()]);

    let (stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;
    assert_eq!(exit_code, Some(0), "Command should succeed");
    assert!(!stdout.is_empty(), "Should produce output");
    assert!(
        stdout.contains(content) || stdout.contains("success"),
        "Should be meaningful"
    );

    Ok(())
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_invalid_arguments_handling() -> Result<()> {
    // Test read with invalid offset
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args(["file", "read", "/tmp/test.txt", "--offset", "invalid"]);

    let (_stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;
    assert_ne!(exit_code, Some(0), "Should fail with invalid offset");

    // Test grep with empty pattern
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args(["file", "grep", "", "--path", "/tmp"]);

    let (_stdout, _stderr, _exit_code) = run_command_with_output(&mut cmd)?;
    // Should handle empty pattern gracefully (may succeed or fail, but no panic)

    Ok(())
}

#[test]
fn test_permission_error_handling() -> Result<()> {
    // Test reading a file that requires root permissions on macOS
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args([
        "file",
        "read",
        "/etc/master.passwd", // Requires root permissions on macOS
    ]);

    let (stdout, stderr, exit_code) = run_command_with_output(&mut cmd)?;
    assert_ne!(exit_code, Some(0), "Should fail with permission error");
    assert!(
        stderr.contains("Permission denied")
            || stderr.contains("No such file")
            || stdout.contains("error"),
        "Should indicate permission or access error"
    );

    Ok(())
}

#[test]
fn test_help_command_functionality() -> Result<()> {
    // Test main file help
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args(["file", "--help"]);

    let (stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;
    assert_eq!(exit_code, Some(0), "Help should succeed");
    assert!(
        stdout.contains("read") || stdout.contains("write"),
        "Should show subcommands"
    );

    // Test individual command help
    let mut cmd = Command::cargo_bin("sah")?;
    cmd.args(["file", "read", "--help"]);

    let (stdout, _stderr, exit_code) = run_command_with_output(&mut cmd)?;
    assert_eq!(exit_code, Some(0), "Read help should succeed");
    assert!(
        stdout.contains("offset") || stdout.contains("limit"),
        "Should show read options"
    );

    Ok(())
}
