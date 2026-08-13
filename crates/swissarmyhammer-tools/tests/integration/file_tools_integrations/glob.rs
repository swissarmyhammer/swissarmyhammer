//! The glob tool.
//!
//! Basic and recursive patterns, gitignore integration, pattern
//! validation, case sensitivity, modification-time order, and no matches.

use super::*;

// ============================================================================
// Glob Tool Tests
// ============================================================================

#[tokio::test]
async fn test_glob_tool_discovery_and_registration() {
    let registry = create_test_registry().await;
    verify_tool_registration(
        &registry,
        "files",
        &["file"],
        &["op"],
        &["pattern", "path", "case_sensitive", "respect_git_ignore"],
    );
}

#[tokio::test]
async fn test_glob_tool_basic_pattern_matching() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Create test directory structure
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_files = vec![
        "test1.txt",
        "test2.js",
        "subdir/test3.txt",
        "subdir/test4.py",
        "README.md",
    ];

    for file_path in test_files {
        let full_path = &temp_dir.join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, format!("Content of {}", file_path)).unwrap();
    }

    // Test basic glob pattern
    let mut arguments = glob_args("*.txt");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Basic glob should succeed: {:?}", result);

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Extract response text
    let response_text = extract_response_text(&call_result);

    assert!(response_text.contains("test1.txt"));
    assert!(!response_text.contains("test2.js"));
    assert!(!response_text.contains("README.md"));
}

#[tokio::test]
async fn test_glob_tool_advanced_gitignore_integration() {
    // This test verifies .gitignore patterns are properly respected.
    // Since **/* is rejected as too broad, we use directory-scoped patterns
    // to test the gitignore functionality.
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Create test directory with .gitignore and git repo
    let (_env, temp_dir) = create_test_dir_with_git();

    // Write .gitignore file
    let gitignore_content = "*.log\n/build/\ntemp_*\n!important.log\n";
    fs::write(temp_dir.join(".gitignore"), gitignore_content).unwrap();

    // Create a src directory structure for scoped pattern testing
    let test_files = vec![
        "src/main.rs",
        "src/lib.rs",
        "src/debug.log",    // Should be ignored by *.log
        "important.log",    // Explicitly not ignored by !important.log
        "debug.log",        // Should be ignored by *.log
        "build/output.txt", // Should be ignored by /build/
        "temp_file.txt",    // Should be ignored by temp_*
        "normal.txt",       // Should be included
    ];

    for file_path in test_files {
        let full_path = &temp_dir.join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, format!("Content of {}", file_path)).unwrap();
    }

    // Test 1: Scoped pattern for src directory with gitignore
    let mut arguments = glob_args("src/**/*.rs");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    arguments.insert("respect_git_ignore".to_string(), json!(true));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Scoped gitignore glob should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should find .rs files in src/
    assert!(response_text.contains("main.rs"), "Should find main.rs");
    assert!(response_text.contains("lib.rs"), "Should find lib.rs");
    // Should NOT find log files even in src/ (gitignore applies)
    assert!(
        !response_text.contains("debug.log"),
        "Should not find src/debug.log"
    );

    // Test 2: Root-level txt files pattern to verify temp_* is ignored
    let mut arguments = glob_args("*.txt");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    arguments.insert("respect_git_ignore".to_string(), json!(true));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Root txt pattern should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should find normal.txt
    assert!(
        response_text.contains("normal.txt"),
        "Should find normal.txt"
    );
    // Should NOT find temp_file.txt (ignored by temp_*)
    assert!(
        !response_text.contains("temp_file.txt"),
        "Should not find temp_file.txt"
    );

    // Test 3: Log files pattern to verify !important.log negation
    let mut arguments = glob_args("*.log");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    arguments.insert("respect_git_ignore".to_string(), json!(true));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Root log pattern should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should find important.log (negated in .gitignore with !important.log)
    assert!(
        response_text.contains("important.log"),
        "Should find important.log (negated ignore)"
    );
    // Should NOT find debug.log (ignored by *.log)
    assert!(
        !response_text.contains("debug.log"),
        "Should not find debug.log"
    );
}

#[tokio::test]
async fn test_glob_tool_pattern_validation() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Test empty pattern
    let arguments = glob_args("");

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Empty pattern should fail");

    // Test overly long pattern
    let long_pattern = "a".repeat(1001);
    let arguments = glob_args(&long_pattern);

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Overly long pattern should fail");

    // Test invalid glob pattern
    let arguments = glob_args("[invalid[pattern");

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Invalid glob pattern should fail");
}

#[tokio::test]
async fn test_glob_tool_case_sensitivity() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Create test files with mixed case
    let (_env, temp_dir) = create_test_dir_with_git();

    // Use different filenames to avoid filesystem case issues
    let test_files = vec!["Test.TXT", "other.txt", "README.md", "readme.MD"];

    for file_path in test_files {
        let full_path = &temp_dir.join(file_path);
        fs::write(full_path, format!("Content of {}", file_path)).unwrap();
    }

    // Test case insensitive (default) - use basic glob to avoid filesystem case issues
    let mut arguments = glob_args("*.txt");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    arguments.insert("respect_git_ignore".to_string(), json!(false)); // Use fallback glob

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should find both .TXT and .txt with case insensitive
    assert!(response_text.contains("Test.TXT"));
    assert!(response_text.contains("other.txt"));

    // Test case sensitive
    let mut arguments = glob_args("*.txt");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
    arguments.insert("case_sensitive".to_string(), json!(true));
    arguments.insert("respect_git_ignore".to_string(), json!(false)); // Use fallback glob

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should only find .txt files, not .TXT
    assert!(!response_text.contains("Test.TXT"));
    assert!(response_text.contains("other.txt"));
}

#[tokio::test]
async fn test_glob_tool_modification_time_sorting() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Create test files with different modification times
    let (_env, temp_dir) = create_test_dir_with_git();

    let file1 = &temp_dir.join("old_file.txt");
    fs::write(file1, "Old content").unwrap();

    // Sleep to ensure different modification times
    std::thread::sleep(std::time::Duration::from_millis(100));

    let file2 = &temp_dir.join("new_file.txt");
    fs::write(file2, "New content").unwrap();

    // Test that files are sorted by modification time (recent first)
    let mut arguments = glob_args("*.txt");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Parse the response to check order - filter out only file paths, not header lines
    let lines: Vec<&str> = response_text
        .lines()
        .filter(|line| line.contains(".txt") && line.starts_with("/"))
        .collect();

    // The newer file should appear before the older file
    if lines.len() >= 2 {
        let first_file_is_new = lines[0].contains("new_file.txt");
        let second_file_is_old = lines[1].contains("old_file.txt");

        // Both conditions should be true for proper sorting
        assert!(
            first_file_is_new && second_file_is_old,
            "Files should be sorted by modification time (recent first). Found order: {:?}",
            lines
        );
    }
}

#[tokio::test]
async fn test_glob_tool_no_matches() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Create test directory with no matching files
    let (_env, temp_dir) = create_test_dir_with_git();

    fs::write(temp_dir.join("test.txt"), "content").unwrap();

    // Search for pattern that won't match
    let mut arguments = glob_args("*.nonexistent");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "No matches should still succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    assert!(response_text.contains("No files found matching pattern"));
}

#[tokio::test]
async fn test_glob_tool_recursive_patterns() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Create nested directory structure
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_files = vec![
        "root.rs",
        "src/main.rs",
        "src/lib.rs",
        "src/utils/helper.rs",
        "tests/integration.rs",
        "docs/readme.md",
    ];

    for file_path in test_files {
        let full_path = &temp_dir.join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, format!("Content of {}", file_path)).unwrap();
    }

    // Test recursive Rust file search
    let mut arguments = glob_args("**/*.rs");
    arguments.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should find all Rust files
    assert!(response_text.contains("root.rs"));
    assert!(response_text.contains("main.rs"));
    assert!(response_text.contains("lib.rs"));
    assert!(response_text.contains("helper.rs"));
    assert!(response_text.contains("integration.rs"));

    // Should not find non-Rust files
    assert!(!response_text.contains("readme.md"));
}
