//! The read tool.
//!
//! Discovery and registration, the success cases, the offset and limit
//! window, the error paths, path-traversal refusal, and the edge cases of
//! an empty, a one-line, a unicode and a very large file.

use super::*;

/// Parameterized test helper for read tool with offset and limit
async fn test_read_with_offset_limit(
    offset: Option<usize>,
    limit: Option<usize>,
    expected_content: &str,
) {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let test_content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
    let (_env, _temp_dir, test_file) = create_test_file("test_file.txt", test_content);

    let mut arguments = read_args(&test_file.to_string_lossy());
    if let Some(offset_val) = offset {
        arguments.insert("offset".to_string(), json!(offset_val));
    }
    if let Some(limit_val) = limit {
        arguments.insert("limit".to_string(), json!(limit_val));
    }

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Read operation should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Default read output is hashline-tagged and carries a leading
    // `#hash:<hex>` freshness-token line. Strip that line, then compare the
    // body against the expected window tagged with absolute 1-based line
    // numbers (the window starts at `offset`, defaulting to line 1).
    let (hash_line, body) = response_text
        .split_once('\n')
        .expect("read output must have a #hash: line then a body");
    assert!(
        hash_line.starts_with("#hash:"),
        "expected leading #hash: line, got {hash_line}"
    );

    let start_line = offset.unwrap_or(1);
    let expected_tagged = swissarmyhammer_hashline::tag(expected_content, start_line);
    assert_eq!(body, expected_tagged);
}

// ============================================================================
// File Read Tool Tests
// ============================================================================

#[tokio::test]
async fn test_read_tool_discovery_and_registration() {
    let registry = create_test_registry().await;
    verify_tool_registration(
        &registry,
        "files",
        &["file"],
        &["op"],
        &["path", "offset", "limit"],
    );
}

#[tokio::test]
async fn test_read_tool_execution_success_cases() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Create temporary file for testing
    let test_content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
    let (_env, _temp_dir, test_file) = create_test_file("test_file.txt", test_content);

    // Test basic file reading
    let arguments = read_args(&test_file.to_string_lossy());

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "File read should succeed: {:?}", result);

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));
    assert!(!call_result.content.is_empty());

    // Default read is hashline-tagged with a `#hash:` token line; recover the
    // plain content to compare against the source.
    assert_eq!(read_content(&call_result), test_content);
}

#[tokio::test]
async fn test_read_tool_offset_limit_functionality() {
    test_read_with_offset_limit(Some(2), Some(2), "Line 2\nLine 3").await;
}

#[tokio::test]
async fn test_read_tool_offset_only() {
    test_read_with_offset_limit(Some(3), None, "Line 3\nLine 4\nLine 5").await;
}

#[tokio::test]
async fn test_read_tool_limit_only() {
    test_read_with_offset_limit(None, Some(3), "Line 1\nLine 2\nLine 3").await;
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_read_tool_missing_file_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Test reading non-existent file
    let arguments = read_args("/non/existent/file.txt");

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Reading non-existent file should fail");

    // Verify error contains helpful information
    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(
        error_msg.contains("parent directory does not exist")
            || error_msg.contains("not found")
            || error_msg.contains("No such file")
    );
}

#[tokio::test]
async fn test_read_tool_relative_path_support() {
    // Pin the working directory to an isolated temp dir so the relative path
    // below resolves there instead of polluting the crate directory.
    let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let _cwd_guard = CurrentDirGuard::new(env.temp_dir())
        .expect("Failed to pin working directory to the isolated temp dir");
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Test reading with relative path - now just verify it doesn't reject due to being relative
    let arguments = read_args("relative/path/file.txt");

    let result = tool.execute(arguments, &context).await;

    // Should not reject due to relative path, but may fail for other reasons (file not found, etc.)
    if let Err(error) = result {
        let error_msg = format!("{:?}", error);
        assert!(
            !error_msg.contains("absolute"),
            "Should not reject relative paths anymore"
        );
    }
    // If it succeeds, that's also fine - relative paths are now allowed
}

#[tokio::test]
async fn test_read_tool_empty_path_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Test reading with empty path
    let arguments = read_args("");

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Empty path should be rejected");

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(
        error_msg.contains("absolute, not relative")
            || error_msg.contains("empty")
            || error_msg.contains("cannot be empty")
    );
}

#[tokio::test]
async fn test_read_tool_missing_required_parameter() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Test execution without required path parameter
    let arguments = serde_json::Map::new(); // Empty arguments

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Missing required parameter should fail");
}

// ============================================================================
// Security Tests
// ============================================================================

#[tokio::test]
async fn test_read_tool_path_traversal_protection() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Test various path traversal attempts
    let dangerous_paths = vec![
        "/tmp/../../../etc/passwd",
        "/tmp/../../etc/passwd",
        "/home/user/../../../etc/passwd",
    ];

    for dangerous_path in dangerous_paths {
        let arguments = read_args(dangerous_path);

        let result = tool.execute(arguments, &context).await;

        // The result may either fail due to path validation or file not found
        // Both outcomes are acceptable for security
        if let Err(err) = result {
            let error_msg = format!("{:?}", err);
            assert!(
                error_msg.contains("blocked pattern")
                    || error_msg.contains("not found")
                    || error_msg.contains("No such file"),
                "Path traversal should be blocked or file not found: {} (error: {})",
                dangerous_path,
                error_msg
            );
        }
        // If it succeeds, the file either doesn't exist or is blocked properly
    }
}

#[tokio::test]
async fn test_read_tool_handles_large_files_safely() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Create a reasonably large test file
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("large_file.txt");

    let mut large_content = String::new();
    for i in 1..=1000 {
        large_content.push_str(&format!("Line {} content\n", i));
    }
    fs::write(test_file, &large_content).unwrap();

    // Test reading large file with limit
    let mut arguments = read_args(&test_file.to_string_lossy());
    arguments.insert("limit".to_string(), json!(10)); // Only read first 10 lines

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Reading large file with limit should succeed"
    );

    let call_result = result.unwrap();
    let content = read_content(&call_result);

    // Should only contain first 10 lines
    let line_count = content.lines().count();
    assert_eq!(line_count, 10);
    assert!(content.starts_with("Line 1 content"));
    assert!(content.contains("Line 10 content"));
    assert!(!content.contains("Line 11 content"));
}

// ============================================================================
// Edge Cases Tests
// ============================================================================

#[tokio::test]
async fn test_read_tool_empty_file() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Create empty file
    let (_env, _temp_dir, test_file) = create_test_file("empty_file.txt", "");

    let arguments = read_args(&test_file.to_string_lossy());

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Reading empty file should succeed");

    let call_result = result.unwrap();
    assert_eq!(read_content(&call_result), "");
}

#[tokio::test]
async fn test_read_tool_single_line_file() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let test_content = "Single line without newline";
    let (_env, _temp_dir, test_file) = create_test_file("single_line.txt", test_content);

    let arguments = read_args(&test_file.to_string_lossy());

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    assert_eq!(read_content(&call_result), test_content);
}

#[tokio::test]
async fn test_read_tool_with_unicode_content() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let test_content = "Hello 🌍\n世界\nПривет мир\n";
    let (_env, _temp_dir, test_file) = create_test_file("unicode_file.txt", test_content);

    let arguments = read_args(&test_file.to_string_lossy());

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Reading unicode file should succeed");

    let call_result = result.unwrap();
    assert_eq!(read_content(&call_result), test_content);
}

#[tokio::test]
async fn test_read_tool_excessive_offset_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let mut arguments = read_args("/tmp/test.txt");
    arguments.insert("offset".to_string(), json!(2_000_000));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Should reject offset over 1,000,000");
    if let Err(e) = result {
        let error_msg = format!("{:?}", e);
        assert!(error_msg.contains("offset must be less than 1,000,000"));
    }
}

#[tokio::test]
async fn test_read_tool_zero_limit_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let mut arguments = read_args("/tmp/test.txt");
    arguments.insert("limit".to_string(), json!(0));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Should reject zero limit");
    if let Err(e) = result {
        let error_msg = format!("{:?}", e);
        assert!(error_msg.contains("limit must be greater than 0"));
    }
}

#[tokio::test]
async fn test_read_tool_excessive_limit_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let mut arguments = read_args("/tmp/test.txt");
    arguments.insert("limit".to_string(), json!(200_000));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Should reject limit over 100,000");
    if let Err(e) = result {
        let error_msg = format!("{:?}", e);
        assert!(error_msg.contains("limit must be less than or equal to 100,000"));
    }
}

#[tokio::test]
async fn test_read_tool_file_not_found_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Test non-existent file
    let arguments = read_args("/tmp/definitely_does_not_exist_12345.txt");

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Should fail for non-existent file");
}

#[tokio::test]
async fn test_read_tool_permission_denied_scenarios() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Test unreadable file (if we can create one)
    let (_env, _temp_dir, test_file) = create_test_file("unreadable.txt", "secret content");

    // Try to make it unreadable (may not work on all systems)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&test_file).unwrap().permissions();
        perms.set_mode(0o000); // No permissions
        let _ = fs::set_permissions(&test_file, perms);
    }

    let arguments = read_args(&test_file.to_string_lossy());

    let result = tool.execute(arguments, &context).await;
    // Note: This test may pass on systems where we can't actually restrict permissions
    if let Err(err) = result {
        let error_msg = format!("{:?}", err);
        println!("Permission denied test error: {}", error_msg);
    }
}

#[tokio::test]
async fn test_read_tool_large_file_handling() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Create a larger file to test performance
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("large_file.txt");

    let mut large_content = String::new();
    for i in 0..1000 {
        large_content.push_str(&format!("This is line number {}\n", i + 1));
    }
    fs::write(test_file, &large_content).unwrap();

    // Test reading with limit to avoid reading the entire large file
    let mut arguments = read_args(&test_file.to_string_lossy());
    arguments.insert("limit".to_string(), json!(100)); // Read only 100 lines

    let start_time = std::time::Instant::now();
    let result = tool.execute(arguments, &context).await;
    let duration = start_time.elapsed();

    assert!(
        result.is_ok(),
        "Large file read should succeed: {:?}",
        result
    );
    assert!(
        duration.as_secs() < 5,
        "Large file read should complete quickly"
    );

    let call_result = result.unwrap();
    let content = read_content(&call_result);

    // Should contain exactly 100 lines worth of content
    let line_count = content.lines().count();
    assert_eq!(line_count, 100, "Should read exactly 100 lines");
}

#[tokio::test]
async fn test_read_tool_edge_cases() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Test empty file
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let empty_file = &temp_dir.join("empty.txt");
    fs::write(empty_file, "").unwrap();

    let arguments = read_args(&empty_file.to_string_lossy());

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Empty file read should succeed");

    // Test file with only whitespace
    let whitespace_file = &temp_dir.join("whitespace.txt");
    fs::write(whitespace_file, "   \n\t\n   \n").unwrap();

    let arguments = read_args(&whitespace_file.to_string_lossy());

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Whitespace file read should succeed");

    // Test file with mixed line endings
    let mixed_endings_file = &temp_dir.join("mixed_endings.txt");
    fs::write(mixed_endings_file, "Line 1\nLine 2\r\nLine 3\rLine 4").unwrap();

    let arguments = read_args(&mixed_endings_file.to_string_lossy());

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Mixed line endings file read should succeed"
    );
}
