//! The edit tool.
//!
//! One replacement and replace-all, the string-not-found error, line-ending
//! preservation, path aliases, and chains of edits in one call.

use super::*;

// ============================================================================
// File Edit Tool Tests
// ============================================================================

#[tokio::test]
async fn test_edit_tool_discovery_and_registration() {
    let registry = create_test_registry().await;
    // `find`/`replace` are the canonical schema properties; the legacy
    // `old_string`/`new_string` names are now aliases resolved by the
    // normalizer at parse time and are not separate schema properties.
    verify_tool_registration(
        &registry,
        "files",
        &["file"],
        &["op"],
        &["file_path", "find", "replace", "edits", "replace_all"],
    );
}

#[tokio::test]
async fn test_edit_tool_single_replacement_success() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Create test file with content to edit (single occurrence)
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("test_edit.txt");
    let initial_content = "Hello world! This is a test file with unique content.";
    fs::write(test_file, initial_content).unwrap();

    // Test single replacement
    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("edit file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("world"));
    arguments.insert("new_string".to_string(), json!("universe"));
    arguments.insert("replace_all".to_string(), json!(false));

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Single replacement should succeed: {:?}",
        result
    );

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify the occurrence was replaced
    let edited_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(
        edited_content,
        "Hello universe! This is a test file with unique content."
    );
}

#[tokio::test]
async fn test_edit_tool_replace_all_success() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Create test file with multiple occurrences
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("test_replace_all.txt");
    let initial_content = "test test test";
    fs::write(test_file, initial_content).unwrap();

    // Test replace all
    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("edit file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("test"));
    arguments.insert("new_string".to_string(), json!("example"));
    arguments.insert("replace_all".to_string(), json!(true));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Replace all should succeed");

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify all occurrences were replaced
    let edited_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(edited_content, "example example example");
}

#[tokio::test]
async fn test_edit_tool_string_not_found_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Create test file
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("test_not_found.txt");
    let initial_content = "Hello world!";
    fs::write(test_file, initial_content).unwrap();

    // Try to replace non-existent string
    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("edit file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("nonexistent"));
    arguments.insert("new_string".to_string(), json!("replacement"));
    arguments.insert("replace_all".to_string(), json!(false));

    let result = tool.execute(arguments, &context).await;
    // A `find` with no confident match now returns a SUCCESSFUL structured
    // near-miss (the model can act on it), not a bare "not found" error. The file
    // is left byte-identical.
    let call_result = result.expect("no-match is a successful structured near-miss");
    assert_eq!(call_result.is_error, Some(false));

    let response_text = extract_response_text(&call_result);
    assert!(
        response_text.contains("nonexistent"),
        "near-miss must echo the find: {response_text}"
    );
    assert!(
        !response_text.contains("not found in file"),
        "legacy bare error string must be gone: {response_text}"
    );

    // File unchanged.
    assert_eq!(fs::read_to_string(test_file).unwrap(), initial_content);
}

#[tokio::test]
async fn test_edit_tool_multiple_occurrences_without_replace_all() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Create test file with duplicate content
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("test_multiple.txt");
    let initial_content = "duplicate duplicate duplicate";
    fs::write(test_file, initial_content).unwrap();

    // Try single replacement on multiple occurrences (should fail)
    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("edit file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("duplicate"));
    arguments.insert("new_string".to_string(), json!("unique"));
    arguments.insert("replace_all".to_string(), json!(false));

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Single replacement with multiple occurrences should succeed and replace first occurrence"
    );

    // Verify only the first occurrence was replaced
    let edited_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(edited_content, "unique duplicate duplicate");
}

#[tokio::test]
async fn test_edit_tool_unicode_content() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("unicode_edit.txt");
    let unicode_content = "Hello 🌍! Здравствуй мир! 你好世界!";
    fs::write(test_file, unicode_content).unwrap();

    // Edit unicode content
    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("edit file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("🌍"));
    arguments.insert("new_string".to_string(), json!("🦀"));
    arguments.insert("replace_all".to_string(), json!(false));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Unicode edit should succeed");

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify Unicode content was edited correctly
    let edited_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(edited_content, "Hello 🦀! Здравствуй мир! 你好世界!");
}

#[tokio::test]
async fn test_edit_tool_preserves_line_endings() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("line_endings.txt");
    // Content with mixed line endings
    let content_with_crlf = "Line 1\r\nLine 2 with target\r\nLine 3\r\n";
    fs::write(test_file, content_with_crlf).unwrap();

    // Edit while preserving line endings
    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("edit file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("target"));
    arguments.insert("new_string".to_string(), json!("replacement"));
    arguments.insert("replace_all".to_string(), json!(false));

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Edit preserving line endings should succeed"
    );

    // Verify line endings were preserved
    let edited_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(
        edited_content,
        "Line 1\r\nLine 2 with replacement\r\nLine 3\r\n"
    );
    assert!(edited_content.contains("\r\n")); // CRLF preserved
}

#[tokio::test]
async fn test_edit_tool_file_not_exists_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let nonexistent_file = &temp_dir.join("does_not_exist.txt");

    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("edit file"));
    arguments.insert(
        "file_path".to_string(),
        json!(nonexistent_file.to_string_lossy()),
    );
    arguments.insert("old_string".to_string(), json!("old"));
    arguments.insert("new_string".to_string(), json!("new"));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Edit on non-existent file should fail");

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(error_msg.contains("does not exist") || error_msg.contains("not found"));
}

#[tokio::test]
async fn test_edit_tool_empty_parameters_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let (_env, _temp_dir, test_file) = create_test_file("test.txt", "test content");

    // Test empty old_string
    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("edit file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!(""));
    arguments.insert("new_string".to_string(), json!("new"));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Edit with empty old_string should fail");

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(error_msg.contains("cannot be empty") || error_msg.contains("required"));
}

#[tokio::test]
async fn test_edit_tool_multiple_edits_sequential() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let (_env, _temp_dir, test_file) =
        create_test_file("multi_edit_test.txt", "Hello world! This is a test.");

    // Test multiple sequential edits
    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("edit file"));
    arguments.insert("path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert(
        "edits".to_string(),
        json!([
            {
                "oldText": "world",
                "newText": "universe"
            },
            {
                "oldText": "test",
                "newText": "example"
            }
        ]),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Multiple edits should succeed");

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify both edits were applied
    let edited_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(edited_content, "Hello universe! This is a example.");
}

#[tokio::test]
async fn test_edit_tool_multiple_edits_with_aliases() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let (_env, _temp_dir, test_file) = create_test_file("alias_test.txt", "foo bar baz");

    // Test parameter aliases
    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("edit file"));
    arguments.insert("filePath".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert(
        "edits".to_string(),
        json!([
            {
                "old_string": "foo",
                "new_text": "FOO"
            },
            {
                "old_text": "bar",
                "new_string": "BAR"
            }
        ]),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Multiple edits with aliases should succeed");

    let edited_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(edited_content, "FOO BAR baz");
}

#[tokio::test]
async fn test_edit_tool_multiple_edits_with_replace_all() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let (_env, _temp_dir, test_file) =
        create_test_file("replace_all_multi.txt", "test test test, example example");

    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("edit file"));
    arguments.insert("path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert(
        "edits".to_string(),
        json!([
            {
                "oldText": "test",
                "newText": "exam",
                "replace_all": true
            },
            {
                "oldText": "example",
                "newText": "sample",
                "replace_all": true
            }
        ]),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Multiple edits with replace_all should succeed"
    );

    let edited_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(edited_content, "exam exam exam, sample sample");
}

#[tokio::test]
async fn test_edit_tool_single_mode_with_path_aliases() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let (_env, _temp_dir, test_file) = create_test_file("single_alias.txt", "test content");

    // Test single edit mode with different parameter aliases
    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("edit file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("oldText".to_string(), json!("test"));
    arguments.insert("newText".to_string(), json!("demo"));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Single edit with aliases should succeed");

    let edited_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(edited_content, "demo content");
}

#[tokio::test]
async fn test_edit_tool_empty_edits_array_error() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let (_env, _temp_dir, test_file) = create_test_file("empty_edits.txt", "content");

    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("edit file"));
    arguments.insert("path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("edits".to_string(), json!([]));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Empty edits array should fail");

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(error_msg.contains("edits array cannot be empty"));
}

#[tokio::test]
async fn test_edit_tool_chain_of_transformations() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let (_env, _temp_dir, test_file) = create_test_file(
        "chain_test.txt",
        "The quick brown fox jumps over the lazy dog",
    );

    // Apply a chain of transformations
    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("edit file"));
    arguments.insert("path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert(
        "edits".to_string(),
        json!([
            {
                "oldText": "quick",
                "newText": "swift"
            },
            {
                "oldText": "brown",
                "newText": "red"
            },
            {
                "oldText": "lazy",
                "newText": "sleepy"
            }
        ]),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Chain of transformations should succeed");

    let edited_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(
        edited_content,
        "The swift red fox jumps over the sleepy dog"
    );
}
