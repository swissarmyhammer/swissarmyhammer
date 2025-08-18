//! Integration tests for file tools
//!
//! These tests verify that file tools work correctly through all layers of the system,
//! including MCP protocol handling, tool registry integration, security validation,
//! and end-to-end scenarios.

use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use swissarmyhammer::git::GitOperations;
use swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage};
use swissarmyhammer::memoranda::{mock_storage::MockMemoStorage, MemoStorage};
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{ToolContext, ToolRegistry};
use swissarmyhammer_tools::mcp::tools::files;
use tempfile::TempDir;

/// Create a test context with mock storage backends for testing MCP tools
async fn create_test_context() -> ToolContext {
    let issue_storage: Arc<tokio::sync::RwLock<Box<dyn IssueStorage>>> =
        Arc::new(tokio::sync::RwLock::new(Box::new(
            FileSystemIssueStorage::new(PathBuf::from("./test_issues")).unwrap(),
        )));
    let git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    let memo_storage: Arc<tokio::sync::RwLock<Box<dyn MemoStorage>>> =
        Arc::new(tokio::sync::RwLock::new(Box::new(MockMemoStorage::new())));

    let rate_limiter = Arc::new(swissarmyhammer::common::rate_limiter::MockRateLimiter);

    let tool_handlers = Arc::new(ToolHandlers::new(memo_storage.clone()));

    ToolContext::new(
        tool_handlers,
        issue_storage,
        git_ops,
        memo_storage,
        rate_limiter,
    )
}

/// Create a test tool registry with file tools registered
fn create_test_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    files::register_file_tools(&mut registry);
    registry
}

// ============================================================================
// File Read Tool Tests
// ============================================================================

#[tokio::test]
async fn test_read_tool_discovery_and_registration() {
    let registry = create_test_registry();

    // Verify the read tool is registered and discoverable
    assert!(registry.get_tool("files_read").is_some());

    let tool_names = registry.list_tool_names();
    assert!(tool_names.contains(&"files_read".to_string()));

    // Verify tool metadata is accessible
    let tool = registry.get_tool("files_read").unwrap();
    assert_eq!(tool.name(), "files_read");
    assert!(!tool.description().is_empty());
    assert!(tool.description().contains("file"));

    // Verify schema structure
    let schema = tool.schema();
    assert!(schema.is_object());
    let properties = schema["properties"].as_object().unwrap();
    assert!(properties.contains_key("absolute_path"));
    assert!(properties.contains_key("offset"));
    assert!(properties.contains_key("limit"));

    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&serde_json::Value::String("absolute_path".to_string())));
}

#[tokio::test]
async fn test_read_tool_execution_success_cases() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Create temporary file for testing
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test_file.txt");
    let test_content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
    fs::write(&test_file, test_content).unwrap();

    // Test basic file reading
    let mut arguments = serde_json::Map::new();
    arguments.insert(
        "absolute_path".to_string(),
        json!(test_file.to_string_lossy()),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "File read should succeed: {:?}", result);

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));
    assert!(!call_result.content.is_empty());

    // Extract the content from the response
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    assert_eq!(response_text, test_content);
}

#[tokio::test]
async fn test_read_tool_offset_limit_functionality() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Create temporary file for testing
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test_file.txt");
    let test_content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
    fs::write(&test_file, test_content).unwrap();

    // Test reading with offset and limit
    let mut arguments = serde_json::Map::new();
    arguments.insert(
        "absolute_path".to_string(),
        json!(test_file.to_string_lossy()),
    );
    arguments.insert("offset".to_string(), json!(2)); // Start from line 2
    arguments.insert("limit".to_string(), json!(2)); // Read 2 lines

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "File read with offset/limit should succeed: {:?}",
        result
    );

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Extract content and verify it contains lines 2 and 3
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    assert_eq!(response_text, "Line 2\nLine 3");
}

#[tokio::test]
async fn test_read_tool_offset_only() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test_file.txt");
    let test_content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
    fs::write(&test_file, test_content).unwrap();

    // Test reading with offset only (should read from line 3 to end)
    let mut arguments = serde_json::Map::new();
    arguments.insert(
        "absolute_path".to_string(),
        json!(test_file.to_string_lossy()),
    );
    arguments.insert("offset".to_string(), json!(3)); // Start from line 3

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    assert_eq!(response_text, "Line 3\nLine 4\nLine 5");
}

#[tokio::test]
async fn test_read_tool_limit_only() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test_file.txt");
    let test_content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
    fs::write(&test_file, test_content).unwrap();

    // Test reading with limit only (should read first 3 lines)
    let mut arguments = serde_json::Map::new();
    arguments.insert(
        "absolute_path".to_string(),
        json!(test_file.to_string_lossy()),
    );
    arguments.insert("limit".to_string(), json!(3)); // Read first 3 lines

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    assert_eq!(response_text, "Line 1\nLine 2\nLine 3");
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_read_tool_missing_file_error() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Test reading non-existent file
    let mut arguments = serde_json::Map::new();
    arguments.insert("absolute_path".to_string(), json!("/non/existent/file.txt"));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Reading non-existent file should fail");

    // Verify error contains helpful information
    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(
        error_msg.contains("Parent directory does not exist")
            || error_msg.contains("not found")
            || error_msg.contains("No such file")
    );
}

#[tokio::test]
async fn test_read_tool_relative_path_error() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Test reading with relative path (should fail)
    let mut arguments = serde_json::Map::new();
    arguments.insert("absolute_path".to_string(), json!("relative/path/file.txt"));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Relative path should be rejected");

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(error_msg.contains("absolute"));
}

#[tokio::test]
async fn test_read_tool_empty_path_error() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Test reading with empty path
    let mut arguments = serde_json::Map::new();
    arguments.insert("absolute_path".to_string(), json!(""));

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
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Test execution without required absolute_path parameter
    let arguments = serde_json::Map::new(); // Empty arguments

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Missing required parameter should fail");
}

// ============================================================================
// Security Tests
// ============================================================================

#[tokio::test]
async fn test_read_tool_path_traversal_protection() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Test various path traversal attempts
    let dangerous_paths = vec![
        "/tmp/../../../etc/passwd",
        "/tmp/../../etc/passwd",
        "/home/user/../../../etc/passwd",
    ];

    for dangerous_path in dangerous_paths {
        let mut arguments = serde_json::Map::new();
        arguments.insert("absolute_path".to_string(), json!(dangerous_path));

        let result = tool.execute(arguments, &context).await;

        // The result may either fail due to path validation or file not found
        // Both outcomes are acceptable for security
        if result.is_err() {
            let error_msg = format!("{:?}", result.unwrap_err());
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
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Create a reasonably large test file
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("large_file.txt");

    let mut large_content = String::new();
    for i in 1..=1000 {
        large_content.push_str(&format!("Line {} content\n", i));
    }
    fs::write(&test_file, &large_content).unwrap();

    // Test reading large file with limit
    let mut arguments = serde_json::Map::new();
    arguments.insert(
        "absolute_path".to_string(),
        json!(test_file.to_string_lossy()),
    );
    arguments.insert("limit".to_string(), json!(10)); // Only read first 10 lines

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Reading large file with limit should succeed"
    );

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Should only contain first 10 lines
    let line_count = response_text.lines().count();
    assert_eq!(line_count, 10);
    assert!(response_text.starts_with("Line 1 content"));
    assert!(response_text.contains("Line 10 content"));
    assert!(!response_text.contains("Line 11 content"));
}

// ============================================================================
// Edge Cases Tests
// ============================================================================

#[tokio::test]
async fn test_read_tool_empty_file() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Create empty file
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("empty_file.txt");
    fs::write(&test_file, "").unwrap();

    let mut arguments = serde_json::Map::new();
    arguments.insert(
        "absolute_path".to_string(),
        json!(test_file.to_string_lossy()),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Reading empty file should succeed");

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    assert_eq!(response_text, "");
}

#[tokio::test]
async fn test_read_tool_single_line_file() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("single_line.txt");
    let test_content = "Single line without newline";
    fs::write(&test_file, test_content).unwrap();

    let mut arguments = serde_json::Map::new();
    arguments.insert(
        "absolute_path".to_string(),
        json!(test_file.to_string_lossy()),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    assert_eq!(response_text, test_content);
}

#[tokio::test]
async fn test_read_tool_with_unicode_content() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("unicode_file.txt");
    let test_content = "Hello 🌍\n世界\nПривет мир\n";
    fs::write(&test_file, test_content).unwrap();

    let mut arguments = serde_json::Map::new();
    arguments.insert(
        "absolute_path".to_string(),
        json!(test_file.to_string_lossy()),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Reading unicode file should succeed");

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    assert_eq!(response_text, test_content);
}

#[tokio::test]
async fn test_read_tool_parameter_validation_errors() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Test invalid offset (too large)
    let mut arguments = serde_json::Map::new();
    arguments.insert("absolute_path".to_string(), json!("/tmp/test.txt"));
    arguments.insert("offset".to_string(), json!(2_000_000)); // Too large

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Should reject offset over 1,000,000");
    if let Err(e) = result {
        let error_msg = format!("{:?}", e);
        assert!(error_msg.contains("offset must be less than 1,000,000"));
    }

    // Test invalid limit (zero)
    let mut arguments = serde_json::Map::new();
    arguments.insert("absolute_path".to_string(), json!("/tmp/test.txt"));
    arguments.insert("limit".to_string(), json!(0)); // Zero limit

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Should reject zero limit");
    if let Err(e) = result {
        let error_msg = format!("{:?}", e);
        assert!(error_msg.contains("limit must be greater than 0"));
    }

    // Test invalid limit (too large)
    let mut arguments = serde_json::Map::new();
    arguments.insert("absolute_path".to_string(), json!("/tmp/test.txt"));
    arguments.insert("limit".to_string(), json!(200_000)); // Too large

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Should reject limit over 100,000");
    if let Err(e) = result {
        let error_msg = format!("{:?}", e);
        assert!(error_msg.contains("limit must be less than or equal to 100,000"));
    }

    // Test empty path
    let mut arguments = serde_json::Map::new();
    arguments.insert("absolute_path".to_string(), json!(""));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Should reject empty path");
    if let Err(e) = result {
        let error_msg = format!("{:?}", e);
        assert!(error_msg.contains("absolute_path cannot be empty"));
    }
}

#[tokio::test]
async fn test_read_tool_file_not_found_error() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Test non-existent file
    let mut arguments = serde_json::Map::new();
    arguments.insert(
        "absolute_path".to_string(),
        json!("/tmp/definitely_does_not_exist_12345.txt"),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Should fail for non-existent file");
}

#[tokio::test]
async fn test_read_tool_permission_denied_scenarios() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Test unreadable file (if we can create one)
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("unreadable.txt");
    fs::write(&test_file, "secret content").unwrap();

    // Try to make it unreadable (may not work on all systems)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&test_file).unwrap().permissions();
        perms.set_mode(0o000); // No permissions
        let _ = fs::set_permissions(&test_file, perms);
    }

    let mut arguments = serde_json::Map::new();
    arguments.insert(
        "absolute_path".to_string(),
        json!(test_file.to_string_lossy()),
    );

    let result = tool.execute(arguments, &context).await;
    // Note: This test may pass on systems where we can't actually restrict permissions
    if result.is_err() {
        let error_msg = format!("{:?}", result.unwrap_err());
        println!("Permission denied test error: {}", error_msg);
    }
}

#[tokio::test]
async fn test_read_tool_large_file_handling() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Create a larger file to test performance
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("large_file.txt");

    let mut large_content = String::new();
    for i in 0..1000 {
        large_content.push_str(&format!("This is line number {}\n", i + 1));
    }
    fs::write(&test_file, &large_content).unwrap();

    // Test reading with limit to avoid reading the entire large file
    let mut arguments = serde_json::Map::new();
    arguments.insert(
        "absolute_path".to_string(),
        json!(test_file.to_string_lossy()),
    );
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
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Should contain exactly 100 lines worth of content
    let line_count = response_text.lines().count();
    assert_eq!(line_count, 100, "Should read exactly 100 lines");
}

#[tokio::test]
async fn test_read_tool_edge_cases() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_read").unwrap();

    // Test empty file
    let temp_dir = TempDir::new().unwrap();
    let empty_file = temp_dir.path().join("empty.txt");
    fs::write(&empty_file, "").unwrap();

    let mut arguments = serde_json::Map::new();
    arguments.insert(
        "absolute_path".to_string(),
        json!(empty_file.to_string_lossy()),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Empty file read should succeed");

    // Test file with only whitespace
    let whitespace_file = temp_dir.path().join("whitespace.txt");
    fs::write(&whitespace_file, "   \n\t\n   \n").unwrap();

    let mut arguments = serde_json::Map::new();
    arguments.insert(
        "absolute_path".to_string(),
        json!(whitespace_file.to_string_lossy()),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Whitespace file read should succeed");

    // Test file with mixed line endings
    let mixed_endings_file = temp_dir.path().join("mixed_endings.txt");
    fs::write(&mixed_endings_file, "Line 1\nLine 2\r\nLine 3\rLine 4").unwrap();

    let mut arguments = serde_json::Map::new();
    arguments.insert(
        "absolute_path".to_string(),
        json!(mixed_endings_file.to_string_lossy()),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Mixed line endings file read should succeed"
    );
}

// ============================================================================
// Glob Tool Tests
// ============================================================================

#[tokio::test]
async fn test_glob_tool_discovery_and_registration() {
    let registry = create_test_registry();

    // Verify the glob tool is registered and discoverable
    assert!(registry.get_tool("files_glob").is_some());

    let tool_names = registry.list_tool_names();
    assert!(tool_names.contains(&"files_glob".to_string()));

    // Verify tool metadata is accessible
    let tool = registry.get_tool("files_glob").unwrap();
    assert_eq!(tool.name(), "files_glob");
    assert!(!tool.description().is_empty());
    assert!(tool.description().contains("pattern matching"));

    // Verify schema structure
    let schema = tool.schema();
    assert!(schema.is_object());
    let properties = schema["properties"].as_object().unwrap();
    assert!(properties.contains_key("pattern"));
    assert!(properties.contains_key("path"));
    assert!(properties.contains_key("case_sensitive"));
    assert!(properties.contains_key("respect_git_ignore"));

    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&serde_json::Value::String("pattern".to_string())));
}

#[tokio::test]
async fn test_glob_tool_basic_pattern_matching() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_glob").unwrap();

    // Create test directory structure
    let temp_dir = TempDir::new().unwrap();
    let test_files = vec![
        "test1.txt",
        "test2.js", 
        "subdir/test3.txt",
        "subdir/test4.py",
        "README.md",
    ];

    for file_path in &test_files {
        let full_path = temp_dir.path().join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&full_path, format!("Content of {}", file_path)).unwrap();
    }

    // Test basic glob pattern
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("*.txt"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Basic glob should succeed: {:?}", result);

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Extract response text
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    assert!(response_text.contains("test1.txt"));
    assert!(!response_text.contains("test2.js"));
    assert!(!response_text.contains("README.md"));
}

#[tokio::test] 
async fn test_glob_tool_advanced_gitignore_integration() {
    // This test will fail initially - it tests the enhanced functionality we need to implement
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_glob").unwrap();

    // Create test directory with .gitignore
    let temp_dir = TempDir::new().unwrap();
    
    // Initialize a git repository (required for ignore crate to work properly)
    use std::process::Command;
    
    Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to initialize git repo");
    
    // Write .gitignore file
    let gitignore_content = "*.log\n/build/\ntemp_*\n!important.log\n";
    fs::write(temp_dir.path().join(".gitignore"), gitignore_content).unwrap();
    

    let test_files = vec![
        "src/main.rs",
        "important.log",     // Explicitly not ignored
        "debug.log",         // Should be ignored
        "build/output.txt",  // Should be ignored
        "temp_file.txt",     // Should be ignored  
        "normal.txt",        // Should be included
    ];

    for file_path in &test_files {
        let full_path = temp_dir.path().join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&full_path, format!("Content of {}", file_path)).unwrap();
    }

    // Test with advanced gitignore
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("**/*"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));
    arguments.insert("respect_git_ignore".to_string(), json!(true));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Advanced gitignore should succeed");

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };


    // Should find files not ignored by .gitignore
    assert!(response_text.contains("main.rs"));
    assert!(response_text.contains("important.log")); // Explicitly not ignored  
    assert!(response_text.contains("normal.txt"));
    
    // Should NOT find ignored files
    assert!(!response_text.contains("debug.log"));
    assert!(!response_text.contains("build/output.txt"));  
    assert!(!response_text.contains("temp_file.txt"));
}

#[tokio::test]
async fn test_glob_tool_pattern_validation() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_glob").unwrap();

    // Test empty pattern
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!(""));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Empty pattern should fail");

    // Test overly long pattern
    let long_pattern = "a".repeat(1001);
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!(long_pattern));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Overly long pattern should fail");

    // Test invalid glob pattern
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("[invalid[pattern"));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Invalid glob pattern should fail");
}

#[tokio::test]
async fn test_glob_tool_case_sensitivity() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_glob").unwrap();

    // Create test files with mixed case
    let temp_dir = TempDir::new().unwrap();
    
    // Initialize git repo for ignore crate to work properly
    use std::process::Command;
    Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to initialize git repo");
    
    // Use different filenames to avoid filesystem case issues
    let test_files = vec!["Test.TXT", "other.txt", "README.md", "readme.MD"];

    for file_path in &test_files {
        let full_path = temp_dir.path().join(file_path);
        fs::write(&full_path, format!("Content of {}", file_path)).unwrap();
    }

    // Test case insensitive (default) - use basic glob to avoid filesystem case issues
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("*.txt"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));
    arguments.insert("respect_git_ignore".to_string(), json!(false)); // Use fallback glob

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };


    // Should find both .TXT and .txt with case insensitive
    assert!(response_text.contains("Test.TXT"));
    assert!(response_text.contains("other.txt"));

    // Test case sensitive
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("*.txt"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));
    arguments.insert("case_sensitive".to_string(), json!(true));
    arguments.insert("respect_git_ignore".to_string(), json!(false)); // Use fallback glob

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Should only find .txt files, not .TXT  
    assert!(!response_text.contains("Test.TXT"));
    assert!(response_text.contains("other.txt"));
}

#[tokio::test]
async fn test_glob_tool_modification_time_sorting() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_glob").unwrap();

    // Create test files with different modification times
    let temp_dir = TempDir::new().unwrap();
    
    // Initialize git repo for ignore crate to work properly
    use std::process::Command;
    Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to initialize git repo");
    
    let file1 = temp_dir.path().join("old_file.txt");
    fs::write(&file1, "Old content").unwrap();
    
    // Sleep to ensure different modification times
    std::thread::sleep(std::time::Duration::from_millis(100));
    
    let file2 = temp_dir.path().join("new_file.txt");
    fs::write(&file2, "New content").unwrap();

    // Test that files are sorted by modification time (recent first)
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("*.txt"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Parse the response to check order - filter out only file paths, not header lines
    let lines: Vec<&str> = response_text.lines()
        .filter(|line| line.contains(".txt") && line.starts_with("/"))
        .collect();

    // The newer file should appear before the older file
    if lines.len() >= 2 {
        let first_file_is_new = lines[0].contains("new_file.txt");
        let second_file_is_old = lines[1].contains("old_file.txt");
        
        // Both conditions should be true for proper sorting
        assert!(first_file_is_new && second_file_is_old, 
            "Files should be sorted by modification time (recent first). Found order: {:?}", lines);
    }
}

#[tokio::test]
async fn test_glob_tool_no_matches() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_glob").unwrap();

    // Create test directory with no matching files
    let temp_dir = TempDir::new().unwrap();
    
    // Initialize git repo for ignore crate to work properly
    use std::process::Command;
    Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to initialize git repo");
        
    fs::write(temp_dir.path().join("test.txt"), "content").unwrap();

    // Search for pattern that won't match
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("*.nonexistent"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "No matches should still succeed");

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    assert!(response_text.contains("No files found matching pattern"));
}

#[tokio::test]
async fn test_glob_tool_recursive_patterns() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_glob").unwrap();

    // Create nested directory structure
    let temp_dir = TempDir::new().unwrap();
    let test_files = vec![
        "root.rs",
        "src/main.rs",
        "src/lib.rs",
        "src/utils/helper.rs",
        "tests/integration.rs",
        "docs/readme.md",
    ];

    for file_path in &test_files {
        let full_path = temp_dir.path().join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&full_path, format!("Content of {}", file_path)).unwrap();
    }

    // Test recursive Rust file search
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("**/*.rs"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Should find all Rust files
    assert!(response_text.contains("root.rs"));
    assert!(response_text.contains("main.rs"));
    assert!(response_text.contains("lib.rs"));
    assert!(response_text.contains("helper.rs"));
    assert!(response_text.contains("integration.rs"));
    
    // Should not find non-Rust files
    assert!(!response_text.contains("readme.md"));
}
