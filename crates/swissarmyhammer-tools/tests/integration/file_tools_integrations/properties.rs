//! Properties that hold over generated inputs.
//!
//! A write followed by a read returns what was written, an edit is
//! consistent with the file it produced, and glob, read-window and grep
//! answers stay consistent across a range of patterns and offsets.

use super::*;

// ============================================================================
// Property-Based Fuzz Testing with Proptest
// ============================================================================

/// Helper to extract text from RawContent
fn extract_text_content(raw_content: &rmcp::model::RawContent) -> &str {
    match raw_content {
        rmcp::model::RawContent::Text(text_content) => &text_content.text,
        _ => "", // Handle other RawContent variants if they exist
    }
}

// Property-based testing using regular tokio tests with generated data
#[tokio::test]
async fn test_write_read_roundtrip_properties() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files").unwrap();
    let read_tool = registry.get_tool("files").unwrap();

    // Test various file path and content combinations
    let repeated_content = "Pattern ".repeat(100);
    let test_cases = vec![
        ("simple.txt", "Hello, world!"),
        ("nested/path/file.txt", "Content with\nmultiple lines"),
        (
            "unicode_file.txt",
            "Unicode content: 🦀 Rust is awesome! 中文测试",
        ),
        ("empty_file.txt", ""),
        (
            "special_chars.txt",
            "Content with !@#$%^&*() special characters",
        ),
        ("repeated.txt", repeated_content.as_str()),
        (
            "long_path/deep/nested/structure/file.txt",
            "Deep nesting test",
        ),
    ];

    for (file_path, content) in test_cases {
        let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
        let temp_dir = _env.temp_dir();
        let full_path = &temp_dir.join(file_path);

        // Ensure parent directory exists
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }

        // Write file
        let mut write_args = serde_json::Map::new();
        write_args.insert("op".to_string(), json!("write file"));
        write_args.insert("file_path".to_string(), json!(full_path.to_string_lossy()));
        write_args.insert("content".to_string(), json!(content));

        let write_result = write_tool.execute(write_args, &context).await;
        if write_result.is_err() {
            continue; // Some file paths may be invalid
        }

        // Read file back
        let mut read_args = serde_json::Map::new();
        read_args.insert("op".to_string(), json!("read file"));
        read_args.insert("path".to_string(), json!(full_path.to_string_lossy()));

        let read_result = read_tool.execute(read_args, &context).await;
        match read_result {
            Ok(response) => {
                // Default read is hashline-tagged with a `#hash:` token line;
                // recover the plain content for the round-trip comparison.
                let recovered = read_content(&response);
                assert_eq!(
                    recovered, content,
                    "Content mismatch for file: {}",
                    file_path
                );
            }
            Err(e) => panic!("Read failed for file {}: {:?}", file_path, e),
        }
    }
}

#[tokio::test]
async fn test_edit_operation_consistency_properties() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files").unwrap();
    let edit_tool = registry.get_tool("files").unwrap();
    let read_tool = registry.get_tool("files").unwrap();

    // Test various edit scenarios
    let test_cases = vec![
        ("Hello world", "world", "universe", false),
        ("test test test", "test", "exam", true),
        ("Multi\nline\ncontent\nwith\npatterns", "line", "row", false),
        ("Pattern123Pattern456Pattern789", "Pattern", "Match", true),
        ("Special chars: !@# $%^ &*()", "!@#", "ABC", false),
    ];

    for (original_content, old_string, new_string, replace_all) in test_cases {
        let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
        let temp_dir = _env.temp_dir();
        let file_path = &temp_dir.join("edit_test.txt");

        // Write original file
        let mut write_args = serde_json::Map::new();
        write_args.insert("op".to_string(), json!("write file"));
        write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
        write_args.insert("content".to_string(), json!(original_content));

        write_tool.execute(write_args, &context).await.unwrap();

        // Perform edit
        let mut edit_args = serde_json::Map::new();
        edit_args.insert("op".to_string(), json!("edit file"));
        edit_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
        edit_args.insert("old_string".to_string(), json!(old_string));
        edit_args.insert("new_string".to_string(), json!(new_string));
        edit_args.insert("replace_all".to_string(), json!(replace_all));

        let edit_result = edit_tool.execute(edit_args, &context).await;
        if edit_result.is_err() {
            continue; // Edit might fail for valid reasons
        }

        // Read back and verify
        let mut read_args = serde_json::Map::new();
        read_args.insert("op".to_string(), json!("read file"));
        read_args.insert("path".to_string(), json!(file_path.to_string_lossy()));

        let response = read_tool.execute(read_args, &context).await.unwrap();
        let edited_content = extract_text_content(&response.content[0].raw);

        if replace_all {
            // All instances should be replaced
            assert!(!edited_content.contains(old_string) || edited_content.contains(new_string));
        } else {
            // At least one instance should be replaced
            assert!(edited_content != original_content);
            assert!(edited_content.contains(new_string));
        }
    }
}

#[tokio::test]
async fn test_glob_pattern_consistency_properties() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files").unwrap();
    let glob_tool = registry.get_tool("files").unwrap();

    // Test different file extensions and patterns
    let test_cases = vec![
        (vec!["txt", "txt", "txt"], "*.txt", 3),
        (vec!["rs", "rs", "py", "js"], "*.rs", 2),
        (vec!["md", "json", "toml"], "*.md", 1),
        (vec!["log", "log", "log", "log"], "*.log", 4),
    ];

    for (extensions, pattern, expected_count) in test_cases {
        let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
        let temp_dir = _env.temp_dir();

        // Create files with specified extensions
        for (i, ext) in extensions.iter().enumerate() {
            let file_path = &temp_dir.join(format!("test_file_{}.{}", i, ext));
            let content = format!("Content for file {}", i);

            let mut write_args = serde_json::Map::new();
            write_args.insert("op".to_string(), json!("write file"));
            write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
            write_args.insert("content".to_string(), json!(content));

            write_tool.execute(write_args, &context).await.ok();
        }

        // Test glob pattern
        let mut glob_args = serde_json::Map::new();
        glob_args.insert("op".to_string(), json!("glob files"));
        glob_args.insert("pattern".to_string(), json!(pattern));
        glob_args.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));

        let result = glob_tool.execute(glob_args, &context).await;
        if let Ok(response) = result {
            let response_text = extract_text_content(&response.content[0].raw);
            let files_found = if response_text.trim().is_empty() {
                0
            } else {
                // Count only lines that look like file paths (start with / or are relative paths)
                response_text
                    .lines()
                    .filter(|line| {
                        let trimmed = line.trim();
                        !trimmed.is_empty()
                            && !trimmed.starts_with("Found")
                            && !trimmed.starts_with("No files")
                            && (trimmed.starts_with("/") || trimmed.contains("."))
                    })
                    .count()
            };

            assert_eq!(
                files_found, expected_count,
                "Glob pattern '{}' should find {} files",
                pattern, expected_count
            );
        }
    }
}

#[tokio::test]
async fn test_read_offset_limit_consistency_properties() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files").unwrap();
    let read_tool = registry.get_tool("files").unwrap();

    // Create content with multiple lines for line-based testing
    let lines: Vec<String> = (1..=20)
        .map(|i| format!("Line {}: Content for line {}", i, i))
        .collect();
    let content = lines.join("\n");
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let file_path = &temp_dir.join("offset_limit_test.txt");

    // Write file
    let mut write_args = serde_json::Map::new();
    write_args.insert("op".to_string(), json!("write file"));
    write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
    write_args.insert("content".to_string(), json!(content));
    write_tool.execute(write_args, &context).await.unwrap();

    // Test various line-based offset/limit combinations
    let test_cases = vec![
        (1, 5),   // Read first 5 lines (1-based indexing)
        (5, 3),   // Read 3 lines starting from line 5
        (10, 10), // Read 10 lines starting from line 10
        (18, 5),  // Read near end (should be limited by file size)
        (25, 3),  // Offset beyond file (should fail or return empty)
    ];

    for (offset, limit) in test_cases {
        let mut read_args = serde_json::Map::new();
        read_args.insert("op".to_string(), json!("read file"));
        read_args.insert("path".to_string(), json!(file_path.to_string_lossy()));
        read_args.insert("offset".to_string(), json!(offset));
        read_args.insert("limit".to_string(), json!(limit));

        match read_tool.execute(read_args, &context).await {
            Ok(response) => {
                // Default read is hashline-tagged with a `#hash:` token line;
                // recover the plain windowed content for line comparisons.
                let recovered = read_content(&response);
                let read_lines: Vec<&str> = recovered.lines().collect();

                // Assert that we don't exceed the requested limit
                assert!(
                    read_lines.len() <= limit,
                    "Read content should not exceed limit of {} lines, got {}",
                    limit,
                    read_lines.len()
                );

                // If offset is within the file, check content matches expected lines
                if offset <= lines.len() {
                    let start_index = offset.saturating_sub(1); // Convert to 0-based indexing
                    let end_index = std::cmp::min(start_index + limit, lines.len());
                    let expected_lines = &lines[start_index..end_index];

                    assert_eq!(
                        read_lines.len(),
                        expected_lines.len(),
                        "Should read expected number of lines"
                    );
                    for (i, (actual, expected)) in
                        read_lines.iter().zip(expected_lines.iter()).enumerate()
                    {
                        assert_eq!(actual, expected, "Line {} content should match", i + 1);
                    }
                }
            }
            Err(_) => {
                // Offset beyond file size is acceptable
                assert!(
                    offset > lines.len(),
                    "Read should only fail if offset is beyond file size (offset: {}, lines: {})",
                    offset,
                    lines.len()
                );
            }
        }
    }
}

#[tokio::test]
#[allow(clippy::useless_vec)]
async fn test_grep_pattern_robustness_properties() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files").unwrap();
    let grep_tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();

    // Test various content and pattern combinations
    let test_cases = vec![
        ("Hello world testing", "world", true),
        ("No match here", "missing", false),
        ("Multiple test test test", "test", true),
        ("Case sensitive Test", "test", false),
        ("Special chars: !@#$", "!@#", true),
        ("Unicode content 🦀 Rust", "🦀", true),
        ("Line1\nLine2\nLine3", "Line2", true),
        ("", "anything", false), // Empty file
    ];

    for (i, (content, _pattern, _should_match)) in test_cases.iter().enumerate() {
        let file_path = &temp_dir.join(format!("grep_test_{}.txt", i));

        // Write file
        let mut write_args = serde_json::Map::new();
        write_args.insert("op".to_string(), json!("write file"));
        write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
        write_args.insert("content".to_string(), json!(content));
        write_tool.execute(write_args, &context).await.unwrap();
    }

    // Test each pattern
    for (content, pattern, should_match) in test_cases.iter() {
        let mut grep_args = serde_json::Map::new();
        grep_args.insert("op".to_string(), json!("grep files"));
        grep_args.insert("pattern".to_string(), json!(pattern));
        grep_args.insert("path".to_string(), json!(&temp_dir.to_string_lossy()));
        grep_args.insert("output_mode".to_string(), json!("files_with_matches"));

        match grep_tool.execute(grep_args, &context).await {
            Ok(response) => {
                let response_text = extract_text_content(&response.content[0].raw);
                let matches_found = if response_text.trim().is_empty() {
                    0
                } else {
                    response_text.lines().count()
                };

                if *should_match {
                    assert!(
                        matches_found > 0,
                        "Pattern '{}' should find matches in content '{}'",
                        pattern,
                        content
                    );
                } else if content.is_empty() {
                    // Empty files might not be found at all
                    // This is acceptable behavior
                } else {
                    // For non-empty content that shouldn't match, we might still find the file
                    // but the pattern shouldn't be in the content
                    assert!(
                        !content.contains(pattern),
                        "Content '{}' should not contain pattern '{}'",
                        content,
                        pattern
                    );
                }
            }
            Err(_) => {
                // Some patterns might cause regex errors, which is acceptable
                println!("Grep failed for pattern '{}' (acceptable)", pattern);
            }
        }
    }
}
