//! The write tool.
//!
//! New files, overwrites, parent-directory creation, unicode and empty
//! content, and the error paths.

use super::*;

// ============================================================================
// File Write Tool Tests
// ============================================================================

#[tokio::test]
async fn test_write_tool_discovery_and_registration() {
    let registry = create_test_registry().await;
    verify_tool_registration(
        &registry,
        "files",
        &["file"],
        &["op"],
        &["file_path", "content"],
    );
}

#[tokio::test]
async fn test_write_tool_execution_success_cases() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Create temporary directory for testing
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("test_write.txt");
    let test_content = "Hello, World!\nThis is a test file created via MCP integration.";

    // Test basic file writing
    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("write file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(test_content));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "File write should succeed: {:?}", result);

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));
    assert!(!call_result.content.is_empty());

    // Verify the file was actually created with correct content
    assert!(test_file.exists());
    let written_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(written_content, test_content);
}

#[tokio::test]
async fn test_write_tool_overwrite_existing_file() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Create temporary file with initial content
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("test_overwrite.txt");
    let initial_content = "Initial content";
    fs::write(test_file, initial_content).unwrap();

    // A full-file write clobbers an existing file unconditionally — no
    // freshness token, no hash check.
    let new_content = "New overwritten content";
    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("write file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(new_content));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "File overwrite should succeed");

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify the file was overwritten
    let written_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(written_content, new_content);
    assert_ne!(written_content, initial_content);
}

#[tokio::test]
async fn test_write_tool_existing_file_clobbers_without_token() {
    // Through the production dispatcher (`op: "write file"`), an existing-file
    // write with NO token clobbers the target and returns the normal mutation
    // envelope — there is no read-before-write guard.
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("clobber_no_token.txt");
    let initial_content = "Initial content the model never read";
    fs::write(test_file, initial_content).unwrap();

    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("write file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!("clobbered!"));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "unguarded write should succeed");
    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // File is overwritten.
    assert_eq!(fs::read_to_string(test_file).unwrap(), "clobbered!");

    // The overwrite carries the mutation envelope, not a re-base payload.
    let structured = call_result
        .structured_content
        .expect("successful overwrite sets structured content");
    let mutation = &structured["mutation"];
    assert!(mutation["bytes_written"].as_u64().unwrap() > 0);
    assert_eq!(mutation["mutated_paths"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_write_tool_creates_parent_directories() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Create test file in nested directories that don't exist
    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let nested_file = temp_dir
        .join("deeply")
        .join("nested")
        .join("directories")
        .join("test_file.txt");
    let test_content = "File in deeply nested directory";

    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("write file"));
    arguments.insert(
        "file_path".to_string(),
        json!(nested_file.to_string_lossy()),
    );
    arguments.insert("content".to_string(), json!(test_content));

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Write with parent directory creation should succeed"
    );

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify the file and directories were created
    assert!(nested_file.exists());
    let written_content = fs::read_to_string(&nested_file).unwrap();
    assert_eq!(written_content, test_content);
}

#[tokio::test]
async fn test_write_tool_unicode_content() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("unicode_test.txt");
    let unicode_content = "Hello 🦀 Rust!\n你好世界\nПривет мир\n🚀✨🎉";

    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("write file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(unicode_content));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Unicode content write should succeed");

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify Unicode content was written correctly
    let written_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(written_content, unicode_content);
}

#[tokio::test]
async fn test_write_tool_empty_content() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = &temp_dir.join("empty_file.txt");
    let empty_content = "";

    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("write file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(empty_content));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Empty content write should succeed");

    // Verify empty file was created
    assert!(test_file.exists());
    let written_content = fs::read_to_string(test_file).unwrap();
    assert_eq!(written_content, "");
}

#[tokio::test]
async fn test_write_tool_error_handling() {
    let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    // Test invalid file path (empty)
    let arguments = write_args("", "test content");

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Empty file path should fail");

    // Test relative path (should be accepted but may fail due to parent directory).
    // Pin the working directory to the isolated temp dir so the relative path
    // resolves there instead of polluting the crate directory.
    let _cwd_guard = CurrentDirGuard::new(env.temp_dir())
        .expect("Failed to pin working directory to the isolated temp dir");
    let arguments = write_args("relative/path/file.txt", "test content");

    let result = tool.execute(arguments, &context).await;

    match result {
        Ok(_) => {
            // Relative path was accepted and file was created successfully
        }
        Err(error) => {
            let error_msg = format!("{:?}", error);
            // Should not reject due to being relative anymore
            assert!(
                !error_msg.contains("absolute"),
                "Should not reject relative paths"
            );
            // May fail due to parent directory not existing, which is fine
            assert!(
                error_msg.contains("parent directory does not exist")
                    || error_msg.contains("No such file or directory"),
                "Should fail due to missing parent directory, not relative path: {}",
                error_msg
            );
        }
    }
}
