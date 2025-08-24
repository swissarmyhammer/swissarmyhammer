//! Comprehensive integration tests for file command CLI interface
//!
//! This module tests end-to-end CLI workflows for all file tools,
//! including command parsing, execution, output formatting, and error handling.
//!
//! NOTE: File commands have been migrated to dynamic CLI generation.
//! These tests are disabled because the test framework (in_process_test_utils)
//! only works with static CLI parsing and doesn't support dynamic CLI testing.
//! The file commands are no longer part of the static CLI.

#![cfg(feature = "file-cli-tests-disabled")]

use anyhow::Result;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

mod in_process_test_utils;
use in_process_test_utils::run_sah_command_in_process;

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

#[tokio::test]
async fn test_file_read_basic_functionality() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("test.txt");
    let test_content = "Hello, World!\nThis is a test file.\nWith multiple lines.";

    create_test_file(&test_file, test_content)?;

    let result = run_sah_command_in_process(&[
        "file",
        "read",
        "--absolute_path",
        test_file.to_str().unwrap(),
    ])
    .await?;

    assert_eq!(result.exit_code, 0, "Command should succeed");
    assert!(
        result.stdout.contains(test_content),
        "Output should contain file content"
    );
    assert!(
        result.stderr.is_empty() || !result.stderr.contains("error"),
        "Should not have errors"
    );

    Ok(())
}

#[tokio::test]
async fn test_file_read_with_offset_and_limit() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("test.txt");
    let lines: Vec<String> = (1..=20).map(|i| format!("Line {}", i)).collect();
    let test_content = lines.join("\n");

    create_test_file(&test_file, &test_content)?;

    // Test with offset - offset 6 means start from line 6
    let result = run_sah_command_in_process(&[
        "file",
        "read",
        "--absolute_path",
        test_file.to_str().unwrap(),
        "--offset",
        "6",
    ])
    .await?;

    assert_eq!(result.exit_code, 0, "Command should succeed");
    assert!(
        !result.stdout.contains("Line 1\n") && !result.stdout.starts_with("Line 1"),
        "Should not contain line 1 when starting from line 6"
    );
    assert!(result.stdout.contains("Line 6"), "Should start from line 6");

    // Test with limit
    let result = run_sah_command_in_process(&[
        "file",
        "read",
        "--absolute_path",
        test_file.to_str().unwrap(),
        "--limit",
        "3",
    ])
    .await?;

    assert_eq!(result.exit_code, 0, "Command should succeed");
    assert!(
        result.stdout.contains("Line 1"),
        "Should contain first line"
    );
    assert!(
        result.stdout.contains("Line 3"),
        "Should contain third line"
    );
    assert!(
        !result.stdout.contains("Line 4"),
        "Should not contain fourth line"
    );

    Ok(())
}

#[tokio::test]
async fn test_file_read_nonexistent_file() -> Result<()> {
    let result = run_sah_command_in_process(&[
        "file",
        "read",
        "--absolute_path",
        "/tmp/nonexistent_file_that_should_not_exist.txt",
    ])
    .await?;

    assert_ne!(result.exit_code, 0, "Command should fail");
    assert!(
        result.stderr.contains("No such file")
            || result.stderr.contains("not found")
            || result.stdout.contains("error"),
        "Should indicate file not found"
    );

    Ok(())
}

// ============================================================================
// File Write Command Tests
// ============================================================================

#[tokio::test]
async fn test_file_write_basic_functionality() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("write_test.txt");
    let test_content = "This is new content\nWritten via CLI";

    let result = run_sah_command_in_process(&[
        "file",
        "write",
        "--file_path",
        test_file.to_str().unwrap(),
        "--content",
        test_content,
    ])
    .await?;

    assert_eq!(result.exit_code, 0, "Command should succeed");
    assert!(test_file.exists(), "File should be created");

    let written_content = fs::read_to_string(&test_file)?;
    assert_eq!(
        written_content, test_content,
        "Written content should match"
    );

    Ok(())
}

#[tokio::test]
async fn test_file_write_overwrite_existing() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("overwrite_test.txt");
    let initial_content = "Initial content";
    let new_content = "New overwritten content";

    create_test_file(&test_file, initial_content)?;

    let result = run_sah_command_in_process(&[
        "file",
        "write",
        "--file_path",
        test_file.to_str().unwrap(),
        "--content",
        new_content,
    ])
    .await?;

    assert_eq!(result.exit_code, 0, "Command should succeed");

    let final_content = fs::read_to_string(&test_file)?;
    assert_eq!(final_content, new_content, "Content should be overwritten");
    assert_ne!(
        final_content, initial_content,
        "Old content should be replaced"
    );

    Ok(())
}

#[tokio::test]
async fn test_file_write_creates_parent_directories() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("nested/deep/path/test.txt");
    let test_content = "Content in nested directory";

    let result = run_sah_command_in_process(&[
        "file",
        "write",
        "--file_path",
        test_file.to_str().unwrap(),
        "--content",
        test_content,
    ])
    .await?;

    assert_eq!(result.exit_code, 0, "Command should succeed");
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

#[tokio::test]
async fn test_file_edit_basic_replacement() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("edit_test.txt");
    let initial_content = "Hello old_value, this is a test with old_value.";

    create_test_file(&test_file, initial_content)?;

    let result = run_sah_command_in_process(&[
        "file",
        "edit",
        "--file_path",
        test_file.to_str().unwrap(),
        "--old_string",
        "old_value",
        "--new_string",
        "new_value",
    ])
    .await?;

    assert_eq!(result.exit_code, 0, "Command should succeed");

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

#[tokio::test]
async fn test_file_edit_replace_all() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("edit_all_test.txt");
    let initial_content = "Replace TARGET here and TARGET there and TARGET everywhere.";

    create_test_file(&test_file, initial_content)?;

    let result = run_sah_command_in_process(&[
        "file",
        "edit",
        "--file_path",
        test_file.to_str().unwrap(),
        "--old_string",
        "TARGET",
        "--new_string",
        "RESULT",
        "--replace_all",
    ])
    .await?;

    assert_eq!(result.exit_code, 0, "Command should succeed");

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

#[tokio::test]
async fn test_file_edit_string_not_found() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("edit_not_found_test.txt");
    let initial_content = "This file does not contain the target string.";

    create_test_file(&test_file, initial_content)?;

    let _result = run_sah_command_in_process(&[
        "file",
        "edit",
        "--file_path",
        test_file.to_str().unwrap(),
        "--old_string",
        "nonexistent_string",
        "--new_string",
        "replacement",
    ])
    .await?;

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

#[tokio::test]
async fn test_file_glob_basic_patterns() -> Result<()> {
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
    let result = run_sah_command_in_process(&[
        "file",
        "glob",
        "--pattern",
        "*.txt",
        "--path",
        temp_dir.path().to_str().unwrap(),
    ])
    .await?;

    assert_eq!(result.exit_code, 0, "Command should succeed");
    assert!(result.stdout.contains("file1.txt"), "Should find txt files");
    assert!(
        !result.stdout.contains("file2.rs"),
        "Should not find rs files"
    );

    // Test recursive glob pattern
    let result = run_sah_command_in_process(&[
        "file",
        "glob",
        "--pattern",
        "**/*.txt",
        "--path",
        temp_dir.path().to_str().unwrap(),
    ])
    .await?;

    assert_eq!(result.exit_code, 0, "Command should succeed");
    assert!(
        result.stdout.contains("file1.txt"),
        "Should find top-level txt files"
    );
    assert!(
        result.stdout.contains("file3.txt"),
        "Should find nested txt files"
    );

    Ok(())
}

#[tokio::test]
async fn test_file_glob_case_sensitivity() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Create files with different names to avoid case-insensitive filesystem conflicts
    create_test_file(&temp_dir.path().join("README.TXT"), "content")?;
    create_test_file(&temp_dir.path().join("notes.txt"), "content")?;

    // Test case-sensitive search - should only find files that exactly match case
    let result = run_sah_command_in_process(&[
        "file",
        "glob",
        "--pattern",
        "*.txt", // lowercase pattern
        "--path",
        temp_dir.path().to_str().unwrap(),
        "--case_sensitive",
    ])
    .await?;

    assert_eq!(result.exit_code, 0, "Command should succeed");
    assert!(
        result.stdout.contains("notes.txt"),
        "Should find exact case match"
    );
    assert!(
        !result.stdout.contains("README.TXT"),
        "Should not find different case"
    );

    Ok(())
}

// ============================================================================
// File Grep Command Tests
// ============================================================================

#[tokio::test]
async fn test_file_grep_basic_search() -> Result<()> {
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
    let result = run_sah_command_in_process(&[
        "file",
        "grep",
        "--pattern",
        "TARGET_STRING",
        "--path",
        temp_dir.path().to_str().unwrap(),
    ])
    .await?;

    assert_eq!(result.exit_code, 0, "Command should succeed");
    assert!(
        result.stdout.contains("search1.txt")
            || result.stdout.contains("3")
            || result.stdout.contains("found"),
        "Should find matches in files: {}",
        result.stdout
    );

    Ok(())
}

#[tokio::test]
async fn test_file_grep_regex_patterns() -> Result<()> {
    let temp_dir = TempDir::new()?;

    let content = r#"
        function processData() {
            return data.map(item => item.value);
        }
        
        const result = processData();
    "#;

    create_test_file(&temp_dir.path().join("code.js"), content)?;

    // Test regex pattern for function definitions
    let result = run_sah_command_in_process(&[
        "file",
        "grep",
        "--pattern",
        r"function\s+\w+",
        "--path",
        temp_dir.path().to_str().unwrap(),
    ])
    .await?;

    assert_eq!(result.exit_code, 0, "Command should succeed");
    // Should find the function definition

    Ok(())
}

#[tokio::test]
async fn test_file_grep_file_type_filtering() -> Result<()> {
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
    let result = run_sah_command_in_process(&[
        "file",
        "grep",
        "--pattern",
        "search_target",
        "--path",
        temp_dir.path().to_str().unwrap(),
        "--type",
        "rs",
    ])
    .await?;

    assert_eq!(result.exit_code, 0, "Command should succeed");
    assert!(
        result.stdout.contains("file.rs")
            || result.stdout.contains("1")
            || result.stdout.contains("found"),
        "Should find matches only in Rust files: {}",
        result.stdout
    );

    Ok(())
}

// ============================================================================
// End-to-End Workflow Tests
// ============================================================================

#[tokio::test]
async fn test_complete_file_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("workflow_test.txt");
    let initial_content = "Initial content with OLD_VALUE to replace.";

    // Step 1: Write initial content
    let result = run_sah_command_in_process(&[
        "file",
        "write",
        "--file_path",
        test_file.to_str().unwrap(),
        "--content",
        initial_content,
    ])
    .await?;
    assert_eq!(result.exit_code, 0, "Write should succeed");

    // Step 2: Read and verify content
    let result = run_sah_command_in_process(&[
        "file",
        "read",
        "--absolute_path",
        test_file.to_str().unwrap(),
    ])
    .await?;
    assert_eq!(result.exit_code, 0, "Read should succeed");
    assert!(
        result.stdout.contains("OLD_VALUE"),
        "Should contain original content"
    );

    // Step 3: Edit the content
    let result = run_sah_command_in_process(&[
        "file",
        "edit",
        "--file_path",
        test_file.to_str().unwrap(),
        "--old_string",
        "OLD_VALUE",
        "--new_string",
        "NEW_VALUE",
    ])
    .await?;
    assert_eq!(result.exit_code, 0, "Edit should succeed");

    // Step 4: Read and verify the edit
    let result = run_sah_command_in_process(&[
        "file",
        "read",
        "--absolute_path",
        test_file.to_str().unwrap(),
    ])
    .await?;
    assert_eq!(result.exit_code, 0, "Final read should succeed");
    assert!(
        result.stdout.contains("NEW_VALUE"),
        "Should contain edited content"
    );
    assert!(
        !result.stdout.contains("OLD_VALUE"),
        "Should not contain old content"
    );

    Ok(())
}

#[tokio::test]
async fn test_file_discovery_and_search_workflow() -> Result<()> {
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
    let result = run_sah_command_in_process(&[
        "file",
        "glob",
        "--pattern",
        "**/*.rs",
        "--path",
        temp_dir.path().to_str().unwrap(),
    ])
    .await?;
    assert_eq!(result.exit_code, 0, "Glob should succeed");
    assert!(result.stdout.contains("main.rs"), "Should find main.rs");
    assert!(result.stdout.contains("lib.rs"), "Should find lib.rs");
    assert!(result.stdout.contains("test.rs"), "Should find test.rs");

    // Step 2: Search for TARGET in all files
    let result = run_sah_command_in_process(&[
        "file",
        "grep",
        "--pattern",
        "TARGET",
        "--path",
        temp_dir.path().to_str().unwrap(),
    ])
    .await?;
    assert_eq!(result.exit_code, 0, "Grep should succeed");
    // Should find matches across multiple files

    // Step 3: Search specifically in Rust files
    let result = run_sah_command_in_process(&[
        "file",
        "grep",
        "--pattern",
        "TARGET",
        "--path",
        temp_dir.path().to_str().unwrap(),
        "--type",
        "rust",
    ])
    .await?;
    assert_eq!(result.exit_code, 0, "Rust-specific grep should succeed");

    Ok(())
}

// ============================================================================
// Output Format Tests
// ============================================================================

#[tokio::test]
async fn test_output_formatting_consistency() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("format_test.txt");
    let content = "Test content for formatting";

    create_test_file(&test_file, content)?;

    // Test that read command produces readable output
    let result = run_sah_command_in_process(&[
        "file",
        "read",
        "--absolute_path",
        test_file.to_str().unwrap(),
    ])
    .await?;
    assert_eq!(result.exit_code, 0, "Command should succeed");
    assert!(!result.stdout.is_empty(), "Should produce output");
    assert!(
        result.stdout.contains(content) || result.stdout.contains("success"),
        "Should be meaningful"
    );

    Ok(())
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_invalid_arguments_handling() -> Result<()> {
    // Test read with invalid offset
    let result = run_sah_command_in_process(&[
        "file",
        "read",
        "--absolute_path",
        "/tmp/test.txt",
        "--offset",
        "invalid",
    ])
    .await?;
    assert_ne!(result.exit_code, 0, "Should fail with invalid offset");

    // Test grep with empty pattern
    let _result =
        run_sah_command_in_process(&["file", "grep", "--pattern", "", "--path", "/tmp"]).await?;
    // Should handle empty pattern gracefully (may succeed or fail, but no panic)

    Ok(())
}

#[tokio::test]
async fn test_permission_error_handling() -> Result<()> {
    // Test reading a file that requires root permissions on macOS
    let result = run_sah_command_in_process(&[
        "file",
        "read",
        "--absolute_path",
        "/etc/master.passwd", // Requires root permissions on macOS
    ])
    .await?;
    assert_ne!(result.exit_code, 0, "Should fail with permission error");
    assert!(
        result.stderr.contains("Permission denied")
            || result.stderr.contains("No such file")
            || result.stdout.contains("error"),
        "Should indicate permission or access error"
    );

    Ok(())
}

#[tokio::test]
async fn test_help_command_functionality() -> Result<()> {
    // Test main file help
    let result = run_sah_command_in_process(&["file", "--help"]).await?;
    assert_eq!(result.exit_code, 0, "Help should succeed");
    assert!(
        result.stdout.contains("read") || result.stdout.contains("write"),
        "Should show subcommands"
    );

    // Test individual command help
    let result = run_sah_command_in_process(&["file", "read", "--help"]).await?;
    assert_eq!(result.exit_code, 0, "Read help should succeed");
    assert!(
        result.stdout.contains("offset") || result.stdout.contains("limit"),
        "Should show read options"
    );

    Ok(())
}
