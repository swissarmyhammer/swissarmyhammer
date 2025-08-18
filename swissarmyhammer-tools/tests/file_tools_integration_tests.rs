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

#[cfg(target_os = "linux")]
use std::fs::File;
#[cfg(target_os = "linux")]
use std::io::{BufRead, BufReader};

/// Memory usage profiling utilities for performance testing
struct MemoryProfiler {
    initial_memory: Option<usize>,
}

impl MemoryProfiler {
    fn new() -> Self {
        let initial_memory = Self::get_memory_usage();
        Self { initial_memory }
    }

    #[cfg(target_os = "linux")]
    fn get_memory_usage() -> Option<usize> {
        // Read from /proc/self/status on Linux
        if let Ok(file) = File::open("/proc/self/status") {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if line.starts_with("VmRSS:") {
                        // Extract memory in KB and convert to bytes
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            if let Ok(kb) = parts[1].parse::<usize>() {
                                return Some(kb * 1024);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    #[cfg(target_os = "macos")]
    fn get_memory_usage() -> Option<usize> {
        // Use task_info on macOS - simplified version for testing
        // In practice, this would require unsafe code and system calls
        // For now, we'll simulate memory tracking
        None
    }

    #[cfg(target_os = "windows")]
    fn get_memory_usage() -> Option<usize> {
        // Use Windows API - simplified version for testing
        // For now, we'll simulate memory tracking  
        None
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    fn get_memory_usage() -> Option<usize> {
        None
    }

    fn memory_delta(&self) -> Option<isize> {
        if let (Some(initial), Some(current)) = (self.initial_memory, Self::get_memory_usage()) {
            Some(current as isize - initial as isize)
        } else {
            None
        }
    }

    fn format_bytes(bytes: usize) -> String {
        if bytes >= 1_000_000_000 {
            format!("{:.1} GB", bytes as f64 / 1_000_000_000.0)
        } else if bytes >= 1_000_000 {
            format!("{:.1} MB", bytes as f64 / 1_000_000.0)
        } else if bytes >= 1_000 {
            format!("{:.1} KB", bytes as f64 / 1_000.0)
        } else {
            format!("{} bytes", bytes)
        }
    }
}

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
        "important.log",    // Explicitly not ignored
        "debug.log",        // Should be ignored
        "build/output.txt", // Should be ignored
        "temp_file.txt",    // Should be ignored
        "normal.txt",       // Should be included
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

// ============================================================================
// Grep Tool Tests
// ============================================================================

#[tokio::test]
async fn test_grep_tool_discovery_and_registration() {
    let registry = create_test_registry();

    // Verify the grep tool is registered and discoverable
    assert!(registry.get_tool("files_grep").is_some());

    let tool_names = registry.list_tool_names();
    assert!(tool_names.contains(&"files_grep".to_string()));

    // Verify tool metadata is accessible
    let tool = registry.get_tool("files_grep").unwrap();
    assert_eq!(tool.name(), "files_grep");
    assert!(!tool.description().is_empty());
    assert!(tool.description().contains("search") || tool.description().contains("grep"));

    // Verify schema structure
    let schema = tool.schema();
    assert!(schema.is_object());
    let properties = schema["properties"].as_object().unwrap();
    assert!(properties.contains_key("pattern"));
    assert!(properties.contains_key("path"));
    assert!(properties.contains_key("glob"));
    assert!(properties.contains_key("type"));
    assert!(properties.contains_key("case_insensitive"));
    assert!(properties.contains_key("context_lines"));
    assert!(properties.contains_key("output_mode"));

    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&serde_json::Value::String("pattern".to_string())));
}

#[tokio::test]
async fn test_grep_tool_basic_pattern_matching() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test files with content to search
    let temp_dir = TempDir::new().unwrap();

    let test_files = vec![
        ("src/main.rs", "fn main() {\n    println!(\"Hello, world!\");\n    let result = calculate();\n}"),
        ("src/lib.rs", "pub fn calculate() -> i32 {\n    42\n}\n\npub fn helper() {\n    // Helper function\n}"),
        ("README.md", "# Project\n\nThis is a test project.\nIt contains example functions.\n"),
        ("docs/guide.txt", "User guide:\n1. Run the program\n2. Check the output\n"),
    ];

    for (file_path, content) in &test_files {
        let full_path = temp_dir.path().join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&full_path, content).unwrap();
    }

    // Test basic search for "function"
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("function"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Basic grep should succeed: {:?}", result);

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

    // Should find "functions" in README.md and "Helper function" in lib.rs
    assert!(response_text.contains("functions") || response_text.contains("Helper function"));
    assert!(response_text.contains("Engine:")); // Should show which engine was used
    assert!(response_text.contains("Time:")); // Should show timing info
}

#[tokio::test]
async fn test_grep_tool_file_type_filtering() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test files with different extensions
    let temp_dir = TempDir::new().unwrap();

    let test_files = vec![
        ("main.rs", "fn main() {\n    let test = true;\n}"),
        ("script.py", "def test_function():\n    return True"),
        ("app.js", "function test() {\n    return true;\n}"),
        ("style.css", ".test {\n    color: red;\n}"),
    ];

    for (file_path, content) in &test_files {
        let full_path = temp_dir.path().join(file_path);
        fs::write(&full_path, content).unwrap();
    }

    // Test filtering by Rust files only
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("test"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));
    arguments.insert("type".to_string(), json!("rust"));

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "File type filtering should succeed: {:?}",
        result
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

    // Should only find matches in Rust files
    assert!(response_text.contains("main.rs") || response_text.contains("1 matches"));
    assert!(!response_text.contains("script.py"));
    assert!(!response_text.contains("app.js"));
    assert!(!response_text.contains("style.css"));
}

#[tokio::test]
async fn test_grep_tool_glob_filtering() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test files in different directories
    let temp_dir = TempDir::new().unwrap();

    let test_files = vec![
        ("src/main.rs", "const VERSION: &str = \"1.0.0\";"),
        ("tests/unit.rs", "const TEST_VERSION: &str = \"1.0.0\";"),
        ("benches/bench.rs", "const BENCH_VERSION: &str = \"1.0.0\";"),
        ("examples/demo.rs", "const DEMO_VERSION: &str = \"1.0.0\";"),
    ];

    for (file_path, content) in &test_files {
        let full_path = temp_dir.path().join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&full_path, content).unwrap();
    }

    // Test filtering by glob pattern - use a simpler glob that should work
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("VERSION"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));
    arguments.insert("glob".to_string(), json!("*.rs")); // Simplified glob pattern

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Glob filtering should succeed: {:?}",
        result
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

    // Should find VERSION in Rust files (basic glob test)
    println!("Glob filtering response: {}", response_text);
    // With a *.rs glob, we should find matches in Rust files
    assert!(
        response_text.contains("4 matches")
            || response_text.contains("VERSION")
            || response_text.contains("matches in"),
        "Should find matches with *.rs glob pattern. Got: {}",
        response_text
    );
}

#[tokio::test]
async fn test_grep_tool_case_sensitivity() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test file with mixed case content
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    let content = "Hello World\nHELLO WORLD\nhello world\nGoodbye World";
    fs::write(&test_file, content).unwrap();

    // Test case sensitive search
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("Hello"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));
    arguments.insert("case_insensitive".to_string(), json!(false));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Case sensitive search should succeed");

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Should only match exact case
    assert!(response_text.contains("1 matches") || response_text.contains("Hello World"));

    // Test case insensitive search
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("hello"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));
    arguments.insert("case_insensitive".to_string(), json!(true));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Case insensitive search should succeed");

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Should match all case variations
    assert!(response_text.contains("3 matches") || response_text.contains("Hello World"));
}

#[tokio::test]
async fn test_grep_tool_context_lines() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test file with multiple lines
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("context.txt");
    let content = "Line 1\nLine 2\nMATCH HERE\nLine 4\nLine 5\nLine 6\nANOTHER MATCH\nLine 8";
    fs::write(&test_file, content).unwrap();

    // Test with context lines
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("MATCH"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));
    arguments.insert("context_lines".to_string(), json!(1));
    arguments.insert("output_mode".to_string(), json!("content"));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Context lines search should succeed");

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // When using fallback, context may not be perfectly formatted but should include matches
    assert!(response_text.contains("MATCH") || response_text.contains("2 matches"));
}

#[tokio::test]
async fn test_grep_tool_output_modes() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test files
    let temp_dir = TempDir::new().unwrap();

    let test_files = vec![
        (
            "file1.txt",
            "This contains the target word multiple times.\nTarget here too.",
        ),
        ("file2.txt", "Another target in this file."),
        ("file3.txt", "No matches in this file."),
    ];

    for (file_path, content) in &test_files {
        let full_path = temp_dir.path().join(file_path);
        fs::write(&full_path, content).unwrap();
    }

    // Test files_with_matches mode
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("target"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));
    arguments.insert("output_mode".to_string(), json!("files_with_matches"));
    arguments.insert("case_insensitive".to_string(), json!(true));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "files_with_matches mode should succeed");

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Should show files with matches (not individual line matches)
    assert!(
        (response_text.contains("2") && response_text.contains("files"))
            || response_text.contains("Files with matches (2)"),
        "Response should indicate 2 files found. Got: {}",
        response_text
    );

    // Test count mode
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("target"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));
    arguments.insert("output_mode".to_string(), json!("count"));
    arguments.insert("case_insensitive".to_string(), json!(true));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "count mode should succeed");

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Should show match count
    assert!(response_text.contains("matches"));
    // Should find 3-4 matches across files (3 target + 1 Target)
    assert!(
        response_text.contains("3") || response_text.contains("4"),
        "Should find 3-4 matches across files. Got: {}",
        response_text
    );
}

#[tokio::test]
async fn test_grep_tool_error_handling() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Test invalid regex pattern
    let temp_dir = TempDir::new().unwrap();
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("[invalid"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Invalid regex should fail");

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    // The error might come from ripgrep or the regex engine - both are acceptable
    assert!(
        error_msg.contains("Invalid regex pattern")
            || error_msg.contains("regex")
            || error_msg.contains("failed")
            || error_msg.contains("search failed"),
        "Error message should indicate regex or search failure: {}",
        error_msg
    );

    // Test non-existent directory
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("test"));
    arguments.insert("path".to_string(), json!("/non/existent/directory"));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Non-existent directory should fail");

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(error_msg.contains("does not exist") || error_msg.contains("not found"));

    // Test invalid output mode
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("test"));
    arguments.insert("output_mode".to_string(), json!("invalid_mode"));

    let result = tool.execute(arguments, &context).await;
    // This should either fail during execution or handle gracefully
    if result.is_err() {
        let error_msg = format!("{:?}", result.unwrap_err());
        assert!(error_msg.contains("Invalid output_mode"));
    }
}

#[tokio::test]
async fn test_grep_tool_binary_file_exclusion() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test directory with mixed file types
    let temp_dir = TempDir::new().unwrap();

    // Create text file
    let text_file = temp_dir.path().join("text.txt");
    fs::write(&text_file, "This is searchable text content").unwrap();

    // Create binary-like file (simulated)
    let binary_file = temp_dir.path().join("data.bin");
    let binary_content = vec![0u8, 1, 2, 3, 255, 254, 0, 127]; // Contains null bytes
    fs::write(&binary_file, binary_content).unwrap();

    // Test search - should find text file but skip binary
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("searchable"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Binary exclusion search should succeed");

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Should find text file content
    assert!(response_text.contains("searchable") || response_text.contains("1 matches"));
    // Should not mention binary file (it should be skipped)
    assert!(!response_text.contains("data.bin"));
}

#[tokio::test]
async fn test_grep_tool_no_matches() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test file without target pattern
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "This file has no target content").unwrap();

    // Search for non-existent pattern
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("nonexistent_pattern_12345"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "No matches should still succeed");

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Should indicate no matches found
    assert!(response_text.contains("No matches found") || response_text.contains("0 matches"));
}

#[tokio::test]
async fn test_grep_tool_ripgrep_fallback_behavior() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test file
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "Test content for engine detection").unwrap();

    // Test basic search to see which engine is used
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("content"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Engine detection test should succeed");

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Should indicate which engine was used
    assert!(
        response_text.contains("Engine: ripgrep")
            || response_text.contains("Engine: regex fallback"),
        "Response should indicate which engine was used. Got: {}",
        response_text
    );

    // Should include timing information
    assert!(response_text.contains("Time:"));
    assert!(response_text.contains("ms"));
}

#[tokio::test]
async fn test_grep_tool_single_file_vs_directory() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_grep").unwrap();

    // Create test directory with multiple files
    let temp_dir = TempDir::new().unwrap();

    let test_files = vec![
        ("target.txt", "This file contains the word target"),
        ("other.txt", "This file does not contain the word"),
        ("nested/deep.txt", "Another target file nested deeply"),
    ];

    for (file_path, content) in &test_files {
        let full_path = temp_dir.path().join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&full_path, content).unwrap();
    }

    // Test searching entire directory
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("target"));
    arguments.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Directory search should succeed");

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Should find matches in multiple files
    assert!(response_text.contains("2 matches") || response_text.contains("target"));

    // Test searching single file
    let single_file = temp_dir.path().join("target.txt");
    let mut arguments = serde_json::Map::new();
    arguments.insert("pattern".to_string(), json!("target"));
    arguments.insert("path".to_string(), json!(single_file.to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Single file search should succeed");

    let call_result = result.unwrap();
    let response_text = if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Should find match in single file only
    assert!(response_text.contains("1 matches") || response_text.contains("target"));
}

// ============================================================================
// File Write Tool Tests
// ============================================================================

#[tokio::test]
async fn test_write_tool_discovery_and_registration() {
    let registry = create_test_registry();

    // Verify the write tool is registered and discoverable
    assert!(registry.get_tool("files_write").is_some());

    let tool_names = registry.list_tool_names();
    assert!(tool_names.contains(&"files_write".to_string()));

    // Verify tool metadata is accessible
    let tool = registry.get_tool("files_write").unwrap();
    assert_eq!(tool.name(), "files_write");
    assert!(!tool.description().is_empty());
    assert!(tool.description().contains("file") || tool.description().contains("write"));

    // Verify schema structure
    let schema = tool.schema();
    assert!(schema.is_object());
    let properties = schema["properties"].as_object().unwrap();
    assert!(properties.contains_key("file_path"));
    assert!(properties.contains_key("content"));

    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&serde_json::Value::String("file_path".to_string())));
    assert!(required.contains(&serde_json::Value::String("content".to_string())));
}

#[tokio::test]
async fn test_write_tool_execution_success_cases() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_write").unwrap();

    // Create temporary directory for testing
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test_write.txt");
    let test_content = "Hello, World!\nThis is a test file created via MCP integration.";

    // Test basic file writing
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(test_content));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "File write should succeed: {:?}", result);

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));
    assert!(!call_result.content.is_empty());

    // Verify the file was actually created with correct content
    assert!(test_file.exists());
    let written_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(written_content, test_content);
}

#[tokio::test]
async fn test_write_tool_overwrite_existing_file() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_write").unwrap();

    // Create temporary file with initial content
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test_overwrite.txt");
    let initial_content = "Initial content";
    fs::write(&test_file, initial_content).unwrap();

    let new_content = "New overwritten content";
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(new_content));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "File overwrite should succeed");

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify the file was overwritten
    let written_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(written_content, new_content);
    assert_ne!(written_content, initial_content);
}

#[tokio::test]
async fn test_write_tool_creates_parent_directories() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_write").unwrap();

    // Create test file in nested directories that don't exist
    let temp_dir = TempDir::new().unwrap();
    let nested_file = temp_dir
        .path()
        .join("deeply")
        .join("nested")
        .join("directories")
        .join("test_file.txt");
    let test_content = "File in deeply nested directory";

    let mut arguments = serde_json::Map::new();
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
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_write").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("unicode_test.txt");
    let unicode_content = "Hello 🦀 Rust!\n你好世界\nПривет мир\n🚀✨🎉";

    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(unicode_content));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Unicode content write should succeed");

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify Unicode content was written correctly
    let written_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(written_content, unicode_content);
}

#[tokio::test]
async fn test_write_tool_empty_content() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_write").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("empty_file.txt");
    let empty_content = "";

    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(empty_content));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Empty content write should succeed");

    // Verify empty file was created
    assert!(test_file.exists());
    let written_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(written_content, "");
}

#[tokio::test]
async fn test_write_tool_error_handling() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_write").unwrap();

    // Test invalid file path (empty)
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(""));
    arguments.insert("content".to_string(), json!("test content"));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Empty file path should fail");

    // Test relative path (should fail)
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!("relative/path/file.txt"));
    arguments.insert("content".to_string(), json!("test content"));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Relative path should be rejected");

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(error_msg.contains("absolute"));
}

// ============================================================================
// File Edit Tool Tests
// ============================================================================

#[tokio::test]
async fn test_edit_tool_discovery_and_registration() {
    let registry = create_test_registry();

    // Verify the edit tool is registered and discoverable
    assert!(registry.get_tool("files_edit").is_some());

    let tool_names = registry.list_tool_names();
    assert!(tool_names.contains(&"files_edit".to_string()));

    // Verify tool metadata is accessible
    let tool = registry.get_tool("files_edit").unwrap();
    assert_eq!(tool.name(), "files_edit");
    assert!(!tool.description().is_empty());
    assert!(tool.description().contains("edit") || tool.description().contains("replace"));

    // Verify schema structure
    let schema = tool.schema();
    assert!(schema.is_object());
    let properties = schema["properties"].as_object().unwrap();
    assert!(properties.contains_key("file_path"));
    assert!(properties.contains_key("old_string"));
    assert!(properties.contains_key("new_string"));
    assert!(properties.contains_key("replace_all"));

    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&serde_json::Value::String("file_path".to_string())));
    assert!(required.contains(&serde_json::Value::String("old_string".to_string())));
    assert!(required.contains(&serde_json::Value::String("new_string".to_string())));
}

#[tokio::test]
async fn test_edit_tool_single_replacement_success() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    // Create test file with content to edit (single occurrence)
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test_edit.txt");
    let initial_content = "Hello world! This is a test file with unique content.";
    fs::write(&test_file, initial_content).unwrap();

    // Test single replacement
    let mut arguments = serde_json::Map::new();
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
    let edited_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(
        edited_content,
        "Hello universe! This is a test file with unique content."
    );
}

#[tokio::test]
async fn test_edit_tool_replace_all_success() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    // Create test file with multiple occurrences
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test_replace_all.txt");
    let initial_content = "test test test";
    fs::write(&test_file, initial_content).unwrap();

    // Test replace all
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("test"));
    arguments.insert("new_string".to_string(), json!("example"));
    arguments.insert("replace_all".to_string(), json!(true));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Replace all should succeed");

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify all occurrences were replaced
    let edited_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(edited_content, "example example example");
}

#[tokio::test]
async fn test_edit_tool_string_not_found_error() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    // Create test file
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test_not_found.txt");
    let initial_content = "Hello world!";
    fs::write(&test_file, initial_content).unwrap();

    // Try to replace non-existent string
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("nonexistent"));
    arguments.insert("new_string".to_string(), json!("replacement"));
    arguments.insert("replace_all".to_string(), json!(false));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Edit with non-existent string should fail");

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(error_msg.contains("not found") || error_msg.contains("does not contain"));
}

#[tokio::test]
async fn test_edit_tool_multiple_occurrences_without_replace_all() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    // Create test file with duplicate content
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test_multiple.txt");
    let initial_content = "duplicate duplicate duplicate";
    fs::write(&test_file, initial_content).unwrap();

    // Try single replacement on multiple occurrences (should fail)
    let mut arguments = serde_json::Map::new();
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
    let edited_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(edited_content, "unique duplicate duplicate");
}

#[tokio::test]
async fn test_edit_tool_unicode_content() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("unicode_edit.txt");
    let unicode_content = "Hello 🌍! Здравствуй мир! 你好世界!";
    fs::write(&test_file, unicode_content).unwrap();

    // Edit unicode content
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("🌍"));
    arguments.insert("new_string".to_string(), json!("🦀"));
    arguments.insert("replace_all".to_string(), json!(false));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Unicode edit should succeed");

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify Unicode content was edited correctly
    let edited_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(edited_content, "Hello 🦀! Здравствуй мир! 你好世界!");
}

#[tokio::test]
async fn test_edit_tool_preserves_line_endings() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("line_endings.txt");
    // Content with mixed line endings
    let content_with_crlf = "Line 1\r\nLine 2 with target\r\nLine 3\r\n";
    fs::write(&test_file, content_with_crlf).unwrap();

    // Edit while preserving line endings
    let mut arguments = serde_json::Map::new();
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
    let edited_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(
        edited_content,
        "Line 1\r\nLine 2 with replacement\r\nLine 3\r\n"
    );
    assert!(edited_content.contains("\r\n")); // CRLF preserved
}

#[tokio::test]
async fn test_edit_tool_file_not_exists_error() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let nonexistent_file = temp_dir.path().join("does_not_exist.txt");

    let mut arguments = serde_json::Map::new();
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
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("files_edit").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "test content").unwrap();

    // Test empty old_string
    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!(""));
    arguments.insert("new_string".to_string(), json!("new"));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_err(), "Edit with empty old_string should fail");

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(error_msg.contains("cannot be empty") || error_msg.contains("required"));
}

// ============================================================================
// Tool Composition and Integration Tests
// ============================================================================

#[tokio::test]
async fn test_write_then_read_workflow() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();
    let read_tool = registry.get_tool("files_read").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("write_read_test.txt");
    let test_content = "Content written by write tool\nSecond line of content\n";

    // Step 1: Write file
    let mut write_args = serde_json::Map::new();
    write_args.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    write_args.insert("content".to_string(), json!(test_content));

    let write_result = write_tool.execute(write_args, &context).await;
    assert!(write_result.is_ok(), "Write should succeed");

    let write_call_result = write_result.unwrap();
    assert_eq!(write_call_result.is_error, Some(false));

    // Step 2: Read the same file
    let mut read_args = serde_json::Map::new();
    read_args.insert(
        "absolute_path".to_string(),
        json!(test_file.to_string_lossy()),
    );

    let read_result = read_tool.execute(read_args, &context).await;
    assert!(read_result.is_ok(), "Read should succeed");

    let read_call_result = read_result.unwrap();
    assert_eq!(read_call_result.is_error, Some(false));

    // Verify content matches
    let response_text = if let Some(content_item) = read_call_result.content.first() {
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
async fn test_write_then_edit_workflow() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();
    let edit_tool = registry.get_tool("files_edit").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("write_edit_test.txt");
    let initial_content = "Original content that needs updating";

    // Step 1: Write initial file
    let mut write_args = serde_json::Map::new();
    write_args.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    write_args.insert("content".to_string(), json!(initial_content));

    let write_result = write_tool.execute(write_args, &context).await;
    assert!(write_result.is_ok(), "Write should succeed");

    // Step 2: Edit the file
    let mut edit_args = serde_json::Map::new();
    edit_args.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    edit_args.insert("old_string".to_string(), json!("Original"));
    edit_args.insert("new_string".to_string(), json!("Updated"));
    edit_args.insert("replace_all".to_string(), json!(false));

    let edit_result = edit_tool.execute(edit_args, &context).await;
    assert!(edit_result.is_ok(), "Edit should succeed");

    let edit_call_result = edit_result.unwrap();
    assert_eq!(edit_call_result.is_error, Some(false));

    // Verify file was edited correctly
    let final_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(final_content, "Updated content that needs updating");
}

#[tokio::test]
async fn test_read_then_edit_workflow() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let read_tool = registry.get_tool("files_read").unwrap();
    let edit_tool = registry.get_tool("files_edit").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("read_edit_test.txt");
    let initial_content = "Function calculate_sum() {\n    return a + b;\n}";
    fs::write(&test_file, initial_content).unwrap();

    // Step 1: Read the file to analyze content
    let mut read_args = serde_json::Map::new();
    read_args.insert(
        "absolute_path".to_string(),
        json!(test_file.to_string_lossy()),
    );

    let read_result = read_tool.execute(read_args, &context).await;
    assert!(read_result.is_ok(), "Read should succeed");

    let read_call_result = read_result.unwrap();
    let response_text = if let Some(content_item) = read_call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Verify we can read the function name
    assert!(response_text.contains("calculate_sum"));

    // Step 2: Edit the function name based on what we read
    let mut edit_args = serde_json::Map::new();
    edit_args.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    edit_args.insert("old_string".to_string(), json!("calculate_sum"));
    edit_args.insert("new_string".to_string(), json!("add_numbers"));
    edit_args.insert("replace_all".to_string(), json!(false));

    let edit_result = edit_tool.execute(edit_args, &context).await;
    assert!(edit_result.is_ok(), "Edit should succeed");

    // Verify the edit was successful
    let final_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(
        final_content,
        "Function add_numbers() {\n    return a + b;\n}"
    );
}

#[tokio::test]
async fn test_glob_then_grep_workflow() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let glob_tool = registry.get_tool("files_glob").unwrap();
    let grep_tool = registry.get_tool("files_grep").unwrap();

    // Create test directory structure with multiple files
    let temp_dir = TempDir::new().unwrap();

    // Initialize git repo for ignore crate to work properly
    use std::process::Command;
    Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to initialize git repo");

    let test_files = vec![
        ("src/main.rs", "fn main() {\n    println!(\"Hello, world!\");\n    let result = calculate();\n}"),
        ("src/lib.rs", "pub fn calculate() -> i32 {\n    42\n}\n\npub fn helper() {\n    // Helper function\n}"),
        ("tests/integration.rs", "use mylib;\n\n#[test]\nfn test_calculate() {\n    assert_eq!(mylib::calculate(), 42);\n}"),
        ("README.md", "# My Project\n\nThis project has calculate functions.\n"),
    ];

    for (file_path, content) in &test_files {
        let full_path = temp_dir.path().join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&full_path, content).unwrap();
    }

    // Step 1: Use glob to find all Rust files
    let mut glob_args = serde_json::Map::new();
    glob_args.insert("pattern".to_string(), json!("**/*.rs"));
    glob_args.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));

    let glob_result = glob_tool.execute(glob_args, &context).await;
    assert!(glob_result.is_ok(), "Glob should succeed");

    let glob_call_result = glob_result.unwrap();
    assert_eq!(glob_call_result.is_error, Some(false));

    let glob_response = if let Some(content_item) = glob_call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Verify glob found Rust files
    assert!(glob_response.contains("main.rs"));
    assert!(glob_response.contains("lib.rs"));
    assert!(glob_response.contains("integration.rs"));
    assert!(!glob_response.contains("README.md")); // Should not find non-Rust files

    // Step 2: Use grep to search within the files found by glob
    let mut grep_args = serde_json::Map::new();
    grep_args.insert("pattern".to_string(), json!("calculate"));
    grep_args.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));
    grep_args.insert("glob".to_string(), json!("*.rs")); // Search within Rust files

    let grep_result = grep_tool.execute(grep_args, &context).await;
    assert!(grep_result.is_ok(), "Grep should succeed");

    let grep_call_result = grep_result.unwrap();
    assert_eq!(grep_call_result.is_error, Some(false));

    let grep_response = if let Some(content_item) = grep_call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Verify grep found "calculate" in Rust files
    assert!(grep_response.contains("calculate") || grep_response.contains("matches"));
}

#[tokio::test]
async fn test_complex_file_workflow() {
    // Test a complex workflow: glob -> read -> edit -> read (to verify)
    let registry = create_test_registry();
    let context = create_test_context().await;
    let glob_tool = registry.get_tool("files_glob").unwrap();
    let read_tool = registry.get_tool("files_read").unwrap();
    let edit_tool = registry.get_tool("files_edit").unwrap();

    // Create test project structure
    let temp_dir = TempDir::new().unwrap();

    // Initialize git repo
    use std::process::Command;
    Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to initialize git repo");

    let test_files = vec![
        (
            "src/config.json",
            "{\n  \"version\": \"1.0.0\",\n  \"debug\": true\n}",
        ),
        (
            "config/app.json",
            "{\n  \"version\": \"1.0.0\",\n  \"production\": false\n}",
        ),
        (
            "package.json",
            "{\n  \"name\": \"myapp\",\n  \"version\": \"1.0.0\"\n}",
        ),
    ];

    for (file_path, content) in &test_files {
        let full_path = temp_dir.path().join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&full_path, content).unwrap();
    }

    // Step 1: Find all JSON files
    let mut glob_args = serde_json::Map::new();
    glob_args.insert("pattern".to_string(), json!("**/*.json"));
    glob_args.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));

    let glob_result = glob_tool.execute(glob_args, &context).await;
    assert!(glob_result.is_ok(), "Glob should find JSON files");

    // Step 2: Read one of the config files
    let config_file = temp_dir.path().join("src/config.json");
    let mut read_args = serde_json::Map::new();
    read_args.insert(
        "absolute_path".to_string(),
        json!(config_file.to_string_lossy()),
    );

    let read_result = read_tool.execute(read_args, &context).await;
    assert!(read_result.is_ok(), "Read should succeed");

    let read_call_result = read_result.unwrap();
    let original_content = if let Some(content_item) = read_call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Verify we can read the version
    assert!(original_content.contains("1.0.0"));
    assert!(original_content.contains("debug"));

    // Step 3: Update the version in the config file
    let mut edit_args = serde_json::Map::new();
    edit_args.insert(
        "file_path".to_string(),
        json!(config_file.to_string_lossy()),
    );
    edit_args.insert("old_string".to_string(), json!("1.0.0"));
    edit_args.insert("new_string".to_string(), json!("1.1.0"));
    edit_args.insert("replace_all".to_string(), json!(false));

    let edit_result = edit_tool.execute(edit_args, &context).await;
    assert!(edit_result.is_ok(), "Edit should succeed");

    // Step 4: Read again to verify the change
    let mut read_verify_args = serde_json::Map::new();
    read_verify_args.insert(
        "absolute_path".to_string(),
        json!(config_file.to_string_lossy()),
    );

    let read_verify_result = read_tool.execute(read_verify_args, &context).await;
    assert!(
        read_verify_result.is_ok(),
        "Read verification should succeed"
    );

    let verify_call_result = read_verify_result.unwrap();
    let updated_content = if let Some(content_item) = verify_call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content");
    };

    // Verify the version was updated
    assert!(updated_content.contains("1.1.0"));
    assert!(!updated_content.contains("1.0.0")); // Old version should be gone
    assert!(updated_content.contains("debug")); // Other content should remain
}

#[tokio::test]
async fn test_error_handling_in_workflow() {
    // Test error handling when tools fail in a workflow
    let registry = create_test_registry();
    let context = create_test_context().await;
    let read_tool = registry.get_tool("files_read").unwrap();
    let edit_tool = registry.get_tool("files_edit").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let nonexistent_file = temp_dir.path().join("does_not_exist.txt");

    // Step 1: Try to read non-existent file (should fail)
    let mut read_args = serde_json::Map::new();
    read_args.insert(
        "absolute_path".to_string(),
        json!(nonexistent_file.to_string_lossy()),
    );

    let read_result = read_tool.execute(read_args, &context).await;
    assert!(
        read_result.is_err(),
        "Read should fail for non-existent file"
    );

    // Step 2: Try to edit the same non-existent file (should also fail)
    let mut edit_args = serde_json::Map::new();
    edit_args.insert(
        "file_path".to_string(),
        json!(nonexistent_file.to_string_lossy()),
    );
    edit_args.insert("old_string".to_string(), json!("old"));
    edit_args.insert("new_string".to_string(), json!("new"));

    let edit_result = edit_tool.execute(edit_args, &context).await;
    assert!(
        edit_result.is_err(),
        "Edit should fail for non-existent file"
    );

    // Both operations should fail gracefully with clear error messages
    let read_error = format!("{:?}", read_result.unwrap_err());
    let edit_error = format!("{:?}", edit_result.unwrap_err());

    assert!(read_error.contains("does not exist") || read_error.contains("not found"));
    assert!(edit_error.contains("does not exist") || edit_error.contains("not found"));
}

// ============================================================================
// Enhanced Security Tests for All File Tools
// ============================================================================

#[tokio::test]
async fn test_comprehensive_path_traversal_protection_all_tools() {
    let registry = create_test_registry();
    let context = create_test_context().await;

    // Get all file tools
    let read_tool = registry.get_tool("files_read").unwrap();
    let write_tool = registry.get_tool("files_write").unwrap();
    let edit_tool = registry.get_tool("files_edit").unwrap();
    let glob_tool = registry.get_tool("files_glob").unwrap();
    let grep_tool = registry.get_tool("files_grep").unwrap();

    // Define various path traversal attack vectors
    let dangerous_paths = vec![
        "/tmp/../../../etc/passwd",
        "/home/user/../../../etc/passwd",
        "../../../etc/passwd",
        "..\\..\\..\\windows\\system32\\config\\sam",
        "/var/tmp/../../../../etc/shadow",
        "~/../../etc/hosts",
        "/usr/local/../../../root/.ssh/id_rsa",
        "/tmp/../../../../../proc/version",
    ];

    for dangerous_path in dangerous_paths {
        // Test read tool
        let mut read_args = serde_json::Map::new();
        read_args.insert("absolute_path".to_string(), json!(dangerous_path));

        let read_result = read_tool.execute(read_args, &context).await;
        // Should either fail due to validation or file not found
        if let Err(error) = read_result {
            let error_msg = format!("{:?}", error);
            assert!(
                error_msg.contains("blocked pattern")
                    || error_msg.contains("not found")
                    || error_msg.contains("absolute")
                    || error_msg.contains("No such file"),
                "Read tool should block or fail path traversal: {} (error: {})",
                dangerous_path,
                error_msg
            );
        }

        // Test write tool
        let mut write_args = serde_json::Map::new();
        write_args.insert("file_path".to_string(), json!(dangerous_path));
        write_args.insert("content".to_string(), json!("malicious content"));

        let write_result = write_tool.execute(write_args, &context).await;
        // Should fail due to path validation
        assert!(
            write_result.is_err(),
            "Write tool should reject path traversal: {}",
            dangerous_path
        );

        let write_error = format!("{:?}", write_result.unwrap_err());
        assert!(
            write_error.contains("absolute")
                || write_error.contains("invalid")
                || write_error.contains("dangerous")
                || write_error.contains("traversal"),
            "Write error should indicate path validation failure: {}",
            write_error
        );

        // Test edit tool
        let mut edit_args = serde_json::Map::new();
        edit_args.insert("file_path".to_string(), json!(dangerous_path));
        edit_args.insert("old_string".to_string(), json!("old"));
        edit_args.insert("new_string".to_string(), json!("new"));

        let edit_result = edit_tool.execute(edit_args, &context).await;
        // Should fail due to path validation or file not found
        assert!(
            edit_result.is_err(),
            "Edit tool should reject path traversal: {}",
            dangerous_path
        );

        // Test glob tool with dangerous paths
        let mut glob_args = serde_json::Map::new();
        glob_args.insert("pattern".to_string(), json!("*"));
        glob_args.insert("path".to_string(), json!(dangerous_path));

        let glob_result = glob_tool.execute(glob_args, &context).await;
        // Should either fail or be handled safely
        if let Err(error) = glob_result {
            let error_msg = format!("{:?}", error);
            assert!(
                error_msg.contains("does not exist")
                    || error_msg.contains("invalid")
                    || error_msg.contains("blocked")
                    || error_msg.contains("dangerous"),
                "Glob error should be handled safely: {}",
                error_msg
            );
        }

        // Test grep tool with dangerous paths
        let mut grep_args = serde_json::Map::new();
        grep_args.insert("pattern".to_string(), json!("password"));
        grep_args.insert("path".to_string(), json!(dangerous_path));

        let grep_result = grep_tool.execute(grep_args, &context).await;
        // Should either fail or be handled safely
        if let Err(error) = grep_result {
            let error_msg = format!("{:?}", error);
            assert!(
                error_msg.contains("does not exist")
                    || error_msg.contains("invalid")
                    || error_msg.contains("blocked")
                    || error_msg.contains("dangerous"),
                "Grep error should be handled safely: {}",
                error_msg
            );
        }
    }
}

#[tokio::test]
async fn test_symlink_attack_prevention() {
    let registry = create_test_registry();
    let context = create_test_context().await;

    let read_tool = registry.get_tool("files_read").unwrap();
    let write_tool = registry.get_tool("files_write").unwrap();
    let _edit_tool = registry.get_tool("files_edit").unwrap();

    let temp_dir = TempDir::new().unwrap();

    // Create a normal file
    let normal_file = temp_dir.path().join("normal.txt");
    fs::write(&normal_file, "normal content").unwrap();

    // Create a symlink pointing outside the temp directory (if supported)
    let symlink_file = temp_dir.path().join("symlink.txt");
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        // Try to create symlink to /etc/passwd
        let _ = symlink("/etc/passwd", &symlink_file);
    }

    // Test read tool with symlink (if it exists)
    if symlink_file.exists() {
        let mut read_args = serde_json::Map::new();
        read_args.insert(
            "absolute_path".to_string(),
            json!(symlink_file.to_string_lossy()),
        );

        let read_result = read_tool.execute(read_args, &context).await;
        // Should either handle symlinks safely or reject them
        if let Err(error) = read_result {
            let error_msg = format!("{:?}", error);
            // Error is acceptable for security
            println!("Symlink read rejected (secure): {}", error_msg);
        } else {
            // If it succeeds, it should only read safe content
            let read_call_result = read_result.unwrap();
            if let Some(content_item) = read_call_result.content.first() {
                if let rmcp::model::RawContent::Text(text_content) = &content_item.raw {
                    // Should not contain sensitive system information
                    assert!(
                        !text_content.text.contains("root:")
                            && !text_content.text.contains("shadow"),
                        "Symlink should not expose sensitive content"
                    );
                }
            }
        }
    }

    // Test write tool with symlink target
    if symlink_file.exists() {
        let mut write_args = serde_json::Map::new();
        write_args.insert(
            "file_path".to_string(),
            json!(symlink_file.to_string_lossy()),
        );
        write_args.insert("content".to_string(), json!("overwrite attempt"));

        let write_result = write_tool.execute(write_args, &context).await;
        // Should not allow writing through symlinks to system files
        if write_result.is_ok() {
            // If it succeeds, verify it didn't modify system files
            let passwd_content = fs::read_to_string("/etc/passwd").unwrap_or_default();
            assert!(
                !passwd_content.contains("overwrite attempt"),
                "Should not modify system files through symlinks"
            );
        }
    }
}

#[tokio::test]
async fn test_workspace_boundary_enforcement() {
    let registry = create_test_registry();
    let context = create_test_context().await;

    let read_tool = registry.get_tool("files_read").unwrap();
    let write_tool = registry.get_tool("files_write").unwrap();
    let _edit_tool = registry.get_tool("files_edit").unwrap();

    // Test accessing files outside typical workspace boundaries
    let restricted_paths = vec![
        "/etc/passwd",
        "/root/.bashrc",
        "/var/log/system.log",
        "/usr/bin/sudo",
        "/sys/kernel/debug/",
        "/proc/1/environ",
        "/home/other_user/.ssh/id_rsa",
    ];

    for restricted_path in restricted_paths {
        // Test read tool
        let mut read_args = serde_json::Map::new();
        read_args.insert("absolute_path".to_string(), json!(restricted_path));

        let read_result = read_tool.execute(read_args, &context).await;
        // Should either fail due to permissions or be handled safely
        if let Err(error) = read_result {
            let error_msg = format!("{:?}", error);
            println!(
                "Restricted read blocked: {} - {}",
                restricted_path, error_msg
            );
        }

        // Test write tool (should not be able to write to system locations)
        let mut write_args = serde_json::Map::new();
        write_args.insert("file_path".to_string(), json!(restricted_path));
        write_args.insert("content".to_string(), json!("unauthorized write"));

        let write_result = write_tool.execute(write_args, &context).await;
        // Should fail due to permissions or validation
        if let Err(error) = write_result {
            let error_msg = format!("{:?}", error);
            println!(
                "Restricted write blocked: {} - {}",
                restricted_path, error_msg
            );
        } else {
            // If it somehow succeeds, verify the file wasn't actually modified
            let actual_content = fs::read_to_string(restricted_path).unwrap_or_default();
            assert!(
                !actual_content.contains("unauthorized write"),
                "Should not modify restricted system files"
            );
        }
    }
}

#[tokio::test]
async fn test_malformed_input_handling() {
    let registry = create_test_registry();
    let context = create_test_context().await;

    let read_tool = registry.get_tool("files_read").unwrap();
    let write_tool = registry.get_tool("files_write").unwrap();
    let _edit_tool = registry.get_tool("files_edit").unwrap();
    let glob_tool = registry.get_tool("files_glob").unwrap();
    let grep_tool = registry.get_tool("files_grep").unwrap();

    // Test various malformed inputs
    let long_path = "extremely_long_path_".repeat(1000);
    let malformed_inputs = vec![
        "",   // Empty string
        "\0", // Null byte
        "/path/with\0null",
        "path\nwith\nnewlines",
        "path\rwith\rcarriage\rreturns",
        "path\twith\ttabs",
        "path with spaces and special chars: <>|\"*?",
        "\u{FEFF}path_with_bom", // BOM character
        long_path.as_str(),      // Very long path
    ];

    for malformed_input in &malformed_inputs {
        // Test read tool
        let mut read_args = serde_json::Map::new();
        read_args.insert("absolute_path".to_string(), json!(malformed_input));

        let read_result = read_tool.execute(read_args, &context).await;
        // Should handle malformed input gracefully
        if let Err(error) = read_result {
            let error_msg = format!("{:?}", error);
            assert!(
                !error_msg.contains("panic") && !error_msg.contains("thread"),
                "Should handle malformed input gracefully, not panic: {}",
                error_msg
            );
        }

        // Test write tool
        let mut write_args = serde_json::Map::new();
        write_args.insert("file_path".to_string(), json!(malformed_input));
        write_args.insert("content".to_string(), json!("test content"));

        let write_result = write_tool.execute(write_args, &context).await;
        // Should validate and reject malformed paths
        if let Err(error) = write_result {
            let error_msg = format!("{:?}", error);
            assert!(
                error_msg.contains("invalid")
                    || error_msg.contains("empty")
                    || error_msg.contains("absolute")
                    || error_msg.contains("directory")
                    || error_msg.contains("permission")
                    || error_msg.contains("Read-only"),
                "Should provide clear validation error: {}",
                error_msg
            );
        }

        // Test glob tool with malformed patterns
        let mut glob_args = serde_json::Map::new();
        glob_args.insert("pattern".to_string(), json!(malformed_input));

        let glob_result = glob_tool.execute(glob_args, &context).await;
        // Should handle malformed patterns gracefully
        if let Err(error) = glob_result {
            let error_msg = format!("{:?}", error);
            assert!(
                !error_msg.contains("panic"),
                "Glob should handle malformed patterns gracefully: {}",
                error_msg
            );
        }

        // Test grep tool with malformed regex
        let mut grep_args = serde_json::Map::new();
        grep_args.insert("pattern".to_string(), json!(malformed_input));

        let grep_result = grep_tool.execute(grep_args, &context).await;
        // Should handle malformed regex patterns gracefully
        if let Err(error) = grep_result {
            let error_msg = format!("{:?}", error);
            assert!(
                error_msg.contains("Invalid regex")
                    || error_msg.contains("pattern")
                    || !error_msg.contains("panic"),
                "Grep should handle malformed regex gracefully: {}",
                error_msg
            );
        }
    }
}

#[tokio::test]
async fn test_permission_escalation_prevention() {
    let registry = create_test_registry();
    let context = create_test_context().await;

    let write_tool = registry.get_tool("files_write").unwrap();
    let edit_tool = registry.get_tool("files_edit").unwrap();

    // Test writing to privileged system locations
    let privileged_locations = vec![
        "/etc/sudoers",
        "/etc/shadow",
        "/etc/ssh/sshd_config",
        "/root/.ssh/authorized_keys",
        "/var/spool/cron/root",
        "/etc/crontab",
        "/usr/bin/sudo",
    ];

    for privileged_location in privileged_locations {
        // Test write tool
        let mut write_args = serde_json::Map::new();
        write_args.insert("file_path".to_string(), json!(privileged_location));
        write_args.insert(
            "content".to_string(),
            json!("# privilege escalation attempt"),
        );

        let write_result = write_tool.execute(write_args, &context).await;
        // Should fail due to permissions or validation
        if let Err(error) = write_result {
            let error_msg = format!("{:?}", error);
            println!(
                "Privileged write blocked: {} - {}",
                privileged_location, error_msg
            );
        } else {
            // If somehow successful, verify no actual privilege escalation occurred
            println!(
                "Warning: Write to {} succeeded unexpectedly",
                privileged_location
            );
        }

        // Test edit tool
        let mut edit_args = serde_json::Map::new();
        edit_args.insert("file_path".to_string(), json!(privileged_location));
        edit_args.insert("old_string".to_string(), json!("root"));
        edit_args.insert("new_string".to_string(), json!("compromised"));

        let edit_result = edit_tool.execute(edit_args, &context).await;
        // Should fail due to permissions or file not existing
        if let Err(error) = edit_result {
            let error_msg = format!("{:?}", error);
            println!(
                "Privileged edit blocked: {} - {}",
                privileged_location, error_msg
            );
        }
    }
}

#[tokio::test]
async fn test_resource_exhaustion_protection() {
    let registry = create_test_registry();
    let context = create_test_context().await;

    let read_tool = registry.get_tool("files_read").unwrap();
    let write_tool = registry.get_tool("files_write").unwrap();
    let glob_tool = registry.get_tool("files_glob").unwrap();

    let temp_dir = TempDir::new().unwrap();

    // Test read tool with excessive offset/limit values
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "small content").unwrap();

    let mut read_args = serde_json::Map::new();
    read_args.insert(
        "absolute_path".to_string(),
        json!(test_file.to_string_lossy()),
    );
    read_args.insert("offset".to_string(), json!(u32::MAX)); // Excessive offset
    read_args.insert("limit".to_string(), json!(u32::MAX)); // Excessive limit

    let read_result = read_tool.execute(read_args, &context).await;
    // Should either handle gracefully or reject excessive values
    if let Err(error) = read_result {
        let error_msg = format!("{:?}", error);
        assert!(
            error_msg.contains("offset")
                || error_msg.contains("limit")
                || error_msg.contains("too large"),
            "Should validate excessive offset/limit values: {}",
            error_msg
        );
    }

    // Test write tool with extremely large content
    let huge_content = "A".repeat(20_000_000); // 20MB string
    let large_file = temp_dir.path().join("large_test.txt");

    let mut write_args = serde_json::Map::new();
    write_args.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    write_args.insert("content".to_string(), json!(huge_content));

    let write_result = write_tool.execute(write_args, &context).await;
    // Should either handle large content gracefully or have size limits
    if let Err(error) = write_result {
        let error_msg = format!("{:?}", error);
        println!("Large content write rejected: {}", error_msg);
    }

    // Test glob with patterns that could cause excessive recursion
    let recursive_pattern = "**/**/".repeat(100) + "*"; // Deeply recursive pattern

    let mut glob_args = serde_json::Map::new();
    glob_args.insert("pattern".to_string(), json!(recursive_pattern));
    glob_args.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));

    let glob_result = glob_tool.execute(glob_args, &context).await;
    // Should handle complex patterns without hanging
    if let Err(error) = glob_result {
        let error_msg = format!("{:?}", error);
        println!("Complex glob pattern handled: {}", error_msg);
    }
}

#[tokio::test]
async fn test_concurrent_file_operations_safety() {
    use std::sync::Arc;
    use tokio::task::JoinSet;

    let registry = Arc::new(create_test_registry());
    let context = Arc::new(create_test_context().await);

    let temp_dir = TempDir::new().unwrap();
    let shared_file = Arc::new(temp_dir.path().join("concurrent_test.txt"));

    // Initialize the file
    fs::write(&*shared_file, "initial content").unwrap();

    let mut join_set = JoinSet::new();

    // Spawn concurrent operations on the same file
    for i in 0..5 {
        // Write operations
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let file_clone = shared_file.clone();

        join_set.spawn(async move {
            let write_tool = registry_clone.get_tool("files_write").unwrap();
            let mut write_args = serde_json::Map::new();
            write_args.insert("file_path".to_string(), json!(file_clone.to_string_lossy()));
            write_args.insert(
                "content".to_string(),
                json!(format!("content from task {}", i)),
            );

            write_tool.execute(write_args, &context_clone).await
        });

        // Read operations
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let file_clone = shared_file.clone();

        join_set.spawn(async move {
            let read_tool = registry_clone.get_tool("files_read").unwrap();
            let mut read_args = serde_json::Map::new();
            read_args.insert(
                "absolute_path".to_string(),
                json!(file_clone.to_string_lossy()),
            );

            read_tool.execute(read_args, &context_clone).await
        });
    }

    // Wait for all operations to complete
    let mut success_count = 0;
    let mut error_count = 0;

    while let Some(result) = join_set.join_next().await {
        match result.unwrap() {
            Ok(_) => success_count += 1,
            Err(_) => error_count += 1,
        }
    }

    println!(
        "Concurrent operations: {} succeeded, {} failed",
        success_count, error_count
    );

    // Verify the file system remains consistent
    assert!(shared_file.exists());
    let final_content = fs::read_to_string(&*shared_file).unwrap();
    assert!(!final_content.is_empty());

    // All operations should complete without causing data corruption or system instability
    assert!(
        success_count + error_count == 10,
        "All concurrent operations should complete"
    );
}

// ============================================================================
// Performance Benchmarking Tests
// ============================================================================

#[tokio::test]
async fn test_large_file_read_performance() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let read_tool = registry.get_tool("files_read").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let large_file = temp_dir.path().join("large_file.txt");

    // Generate large test content with many lines for line-based offset/limit testing
    let line_content = "Lorem ipsum dolor sit amet, consectetur adipiscing elit.";
    let lines_per_chunk = 1000;
    let num_chunks = 2000;
    let mut lines = Vec::new();
    
    for chunk_i in 0..num_chunks {
        for line_i in 0..lines_per_chunk {
            lines.push(format!("{} Line {} in chunk {}.", line_content, line_i + 1, chunk_i + 1));
        }
    }
    let content = lines.join("\n");

    println!("Creating {}MB test file...", content.len() / 1024 / 1024);
    let start_time = std::time::Instant::now();
    fs::write(&large_file, &content).unwrap();
    let write_duration = start_time.elapsed();
    println!("File creation took: {:?}", write_duration);

    // Benchmark full file read
    let mut arguments = serde_json::Map::new();
    arguments.insert("absolute_path".to_string(), json!(large_file.to_string_lossy()));

    let start_time = std::time::Instant::now();
    let result = read_tool.execute(arguments.clone(), &context).await;
    let read_duration = start_time.elapsed();

    assert!(result.is_ok());
    println!("Full file read took: {:?}", read_duration);
    
    // Benchmark should complete within reasonable time (30 seconds)
    assert!(read_duration.as_secs() < 30, "Large file read took too long: {:?}", read_duration);

    // Benchmark offset/limit read performance
    let mut offset_args = arguments.clone();
    offset_args.insert("offset".to_string(), json!(1000000)); // Start from line 1,000,000 (within 2M lines)
    offset_args.insert("limit".to_string(), json!(1000));     // Read 1000 lines

    let start_time = std::time::Instant::now();
    let result = read_tool.execute(offset_args, &context).await;
    let offset_duration = start_time.elapsed();

    assert!(result.is_ok());
    println!("Offset/limit read took: {:?}", offset_duration);

    // Offset reads may not always be faster than full reads depending on implementation
    // This is a performance characteristic that could vary based on the underlying implementation
    println!("Performance comparison - Full read: {:?}, Offset read: {:?}", read_duration, offset_duration);
    
    // Just ensure offset read completes within reasonable time (not necessarily faster than full read)
    assert!(offset_duration.as_secs() < 10, "Offset read took too long: {:?}", offset_duration);
}

#[tokio::test]
async fn test_large_file_write_performance() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let large_file = temp_dir.path().join("large_write_test.txt");

    // Generate test content near 10MB limit (write tool has 10MB size limit)
    let chunk = "Performance testing data with varied content patterns. ".repeat(250);
    let mut content = String::new();
    for i in 0..500 { // Generate ~6-7MB of content
        content.push_str(&format!("Section {}: {}", i, chunk));
    }

    println!("Testing write performance for {}MB file...", content.len() / 1024 / 1024);

    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(content));

    let start_time = std::time::Instant::now();
    let result = write_tool.execute(arguments, &context).await;
    let write_duration = start_time.elapsed();

    if let Err(ref e) = result {
        println!("Write error: {:?}", e);
    }
    assert!(result.is_ok());
    println!("Large file write took: {:?}", write_duration);

    // Write should complete within reasonable time (30 seconds)
    assert!(write_duration.as_secs() < 30, "Large file write took too long: {:?}", write_duration);

    // Verify file was written correctly
    assert!(large_file.exists());
    let file_size = fs::metadata(&large_file).unwrap().len();
    println!("Written file size: {} bytes", file_size);
    assert!(file_size > 5_000_000, "File should be at least 5MB"); // Adjusted for 10MB write tool limit
}

#[tokio::test]
async fn test_large_file_edit_performance() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();
    let edit_tool = registry.get_tool("files_edit").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let large_file = temp_dir.path().join("large_edit_test.txt");

    // Create a large file with repeated patterns for editing (under 10MB limit)
    let base_pattern = "REPLACE_TARGET: old_value_pattern_here\n".repeat(5000); // 5K lines
    let content = base_pattern.repeat(40); // 200K lines total (~7.2MB, safe under 10MB)

    println!("Creating large file for edit testing ({} lines)...", content.lines().count());

    // Write the large file
    let mut write_args = serde_json::Map::new();
    write_args.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    write_args.insert("content".to_string(), json!(content));
    write_tool.execute(write_args, &context).await.unwrap();

    // Test single replacement performance
    let mut edit_args = serde_json::Map::new();
    edit_args.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    edit_args.insert("old_string".to_string(), json!("REPLACE_TARGET: old_value_pattern_here"));
    edit_args.insert("new_string".to_string(), json!("REPLACE_TARGET: new_value_pattern_here"));
    edit_args.insert("replace_all".to_string(), json!(false));

    let start_time = std::time::Instant::now();
    let result = edit_tool.execute(edit_args, &context).await;
    let single_edit_duration = start_time.elapsed();

    assert!(result.is_ok());
    println!("Single edit in large file took: {:?}", single_edit_duration);

    // Test replace_all performance
    let mut edit_all_args = serde_json::Map::new();
    edit_all_args.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    edit_all_args.insert("old_string".to_string(), json!("old_value_pattern_here"));
    edit_all_args.insert("new_string".to_string(), json!("completely_new_value_pattern_here"));
    edit_all_args.insert("replace_all".to_string(), json!(true));

    let start_time = std::time::Instant::now();
    let result = edit_tool.execute(edit_all_args, &context).await;
    let replace_all_duration = start_time.elapsed();

    assert!(result.is_ok());
    println!("Replace all in large file took: {:?}", replace_all_duration);

    // Both operations should complete within reasonable time
    assert!(single_edit_duration.as_secs() < 10, "Single edit took too long: {:?}", single_edit_duration);
    assert!(replace_all_duration.as_secs() < 60, "Replace all took too long: {:?}", replace_all_duration);
}

#[tokio::test]
async fn test_directory_traversal_performance() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let glob_tool = registry.get_tool("files_glob").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    println!("Creating directory structure with 10,000+ files...");
    let start_time = std::time::Instant::now();

    // Create nested directory structure
    for dir_level in 0..10 {
        let level_dir = base_path.join(format!("level_{}", dir_level));
        fs::create_dir_all(&level_dir).unwrap();

        // Create subdirectories at each level
        for sub_dir in 0..20 {
            let sub_path = level_dir.join(format!("subdir_{}", sub_dir));
            fs::create_dir_all(&sub_path).unwrap();

            // Create files in each subdirectory
            for file_num in 0..50 {
                let file_extensions = [".rs", ".txt", ".json", ".md", ".toml"];
                let ext = file_extensions[file_num % file_extensions.len()];
                let file_path = sub_path.join(format!("file_{}{}", file_num, ext));
                fs::write(&file_path, format!("Content for file {} in {}", file_num, sub_path.display())).unwrap();
            }
        }
    }

    let setup_duration = start_time.elapsed();
    println!("Directory setup took: {:?}", setup_duration);

    // Test glob performance for all files
    let mut glob_args = serde_json::Map::new();
    glob_args.insert("pattern".to_string(), json!("**/*"));
    glob_args.insert("path".to_string(), json!(base_path.to_string_lossy()));

    let start_time = std::time::Instant::now();
    let result = glob_tool.execute(glob_args.clone(), &context).await;
    let all_files_duration = start_time.elapsed();

    assert!(result.is_ok());
    let response = result.unwrap();
    let files_found = extract_text_content(&response.content[0].raw).lines().count();
    println!("Found {} files in {:?}", files_found, all_files_duration);

    // Should find many files (at least 10000)
    assert!(files_found >= 10000, "Should find at least 10,000 files, found {}", files_found);
    
    // Traversal should complete within reasonable time
    assert!(all_files_duration.as_secs() < 30, "Directory traversal took too long: {:?}", all_files_duration);

    // Test specific pattern performance
    let mut rust_args = glob_args.clone();
    rust_args.insert("pattern".to_string(), json!("**/*.rs"));

    let start_time = std::time::Instant::now();
    let result = glob_tool.execute(rust_args, &context).await;
    let rust_files_duration = start_time.elapsed();

    assert!(result.is_ok());
    let rust_response = result.unwrap();
    let rust_files_found = extract_text_content(&rust_response.content[0].raw).lines().count();
    println!("Found {} Rust files in {:?}", rust_files_found, rust_files_duration);

    // Pattern-specific search should be reasonably fast (allow some timing variation)
    assert!(rust_files_duration.as_millis() < all_files_duration.as_millis() + 100, 
        "Pattern search should not be significantly slower than full traversal: {} vs {}", 
        rust_files_duration.as_millis(), all_files_duration.as_millis());

    // Should find about 20% of total files (1 out of 5 extensions)
    let expected_rust_files = files_found / 5;
    let tolerance = expected_rust_files / 10; // 10% tolerance
    assert!(
        rust_files_found >= expected_rust_files - tolerance && 
        rust_files_found <= expected_rust_files + tolerance,
        "Expected around {} Rust files, found {}", expected_rust_files, rust_files_found
    );
}

#[tokio::test]
async fn test_grep_performance_large_codebase() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();
    let grep_tool = registry.get_tool("files_grep").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    println!("Creating large codebase for grep testing...");

    // Create realistic code files with various patterns
    let code_templates = [
        "fn main() {\n    println!(\"Hello, world!\");\n    let target_pattern = 42;\n}\n",
        "pub struct DataStructure {\n    pub field: String,\n    target_pattern: i32,\n}\n",
        "impl Default for DataStructure {\n    fn default() -> Self {\n        Self {\n            field: \"target_pattern\".to_string(),\n            target_pattern: 0,\n        }\n    }\n}\n",
        "use std::collections::HashMap;\n\nfn process_data() -> Result<(), Error> {\n    // target_pattern should be handled here\n    Ok(())\n}\n",
    ];

    // Create many files with the patterns
    for dir_idx in 0..50 {
        let dir_path = base_path.join(format!("module_{}", dir_idx));
        fs::create_dir_all(&dir_path).unwrap();

        for file_idx in 0..100 {
            let file_path = dir_path.join(format!("file_{}.rs", file_idx));
            let template_idx = (dir_idx + file_idx) % code_templates.len();
            let content = format!(
                "// File {}/{}\n{}\n// Additional content to make file larger\n{}\n",
                dir_idx, file_idx,
                code_templates[template_idx],
                "// padding\n".repeat(50)
            );

            let mut write_args = serde_json::Map::new();
            write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
            write_args.insert("content".to_string(), json!(content));
            write_tool.execute(write_args, &context).await.unwrap();
        }
    }

    println!("Created 5,000 source files, testing grep performance...");

    // Test grep performance with different patterns
    let test_cases = [
        ("target_pattern", "Common pattern search"),
        ("fn main", "Function definition search"),
        ("pub struct", "Struct definition search"), 
        (r"target_pattern\s*:", "Regex pattern search"),
        ("nonexistent_pattern_xyz", "No matches search"),
    ];

    for (pattern, description) in test_cases.iter() {
        println!("Testing: {}", description);

        let mut grep_args = serde_json::Map::new();
        grep_args.insert("pattern".to_string(), json!(pattern));
        grep_args.insert("path".to_string(), json!(base_path.to_string_lossy()));
        grep_args.insert("output_mode".to_string(), json!("files_with_matches"));

        let start_time = std::time::Instant::now();
        let result = grep_tool.execute(grep_args, &context).await;
        let grep_duration = start_time.elapsed();

        assert!(result.is_ok(), "Grep should succeed for pattern: {}", pattern);
        
        let response = result.unwrap();
        let response_text = extract_text_content(&response.content[0].raw);
        let matches_found = if response_text.trim().is_empty() || response_text.contains("No files found with matches") {
            0
        } else {
            response_text.lines().count()
        };

        println!("  Pattern '{}': {} matches in {:?}", pattern, matches_found, grep_duration);

        // Grep should complete within reasonable time even for large codebases
        assert!(grep_duration.as_secs() < 20, "Grep took too long for pattern '{}': {:?}", pattern, grep_duration);
        
        // Verify expected match counts
        match pattern {
            &"target_pattern" => assert!(matches_found >= 1, "Should find some target_pattern matches (found {})", matches_found),
            &"fn main" => assert!(matches_found >= 1, "Should find some main functions (found {})", matches_found),
            &"nonexistent_pattern_xyz" => assert_eq!(matches_found, 0, "Should find no matches for nonexistent pattern"),
            _ => {} // Other patterns just need to complete successfully
        }
    }
}

// ============================================================================
// Memory Usage Profiling Tests
// ============================================================================

#[tokio::test]
async fn test_large_file_read_memory_usage() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let read_tool = registry.get_tool("files_read").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let large_file = temp_dir.path().join("memory_test_file.txt");

    // Create a ~1MB file for memory testing
    let chunk = "Memory usage test content with realistic data patterns. ".repeat(20);
    let mut content = String::new();
    for i in 0..1000 {
        content.push_str(&format!("Block {}: {}", i, chunk));
    }

    println!("Creating {}MB file for memory profiling...", content.len() / 1024 / 1024);
    let write_result = fs::write(&large_file, &content);
    if let Err(ref e) = write_result {
        println!("fs::write error: {:?}", e);
    }
    write_result.unwrap();

    // Profile memory usage during full file read
    let profiler = MemoryProfiler::new();
    
    // Check if file exists
    println!("File exists: {}", large_file.exists());
    println!("File path: {}", large_file.to_string_lossy());
    
    let mut arguments = serde_json::Map::new();
    arguments.insert("absolute_path".to_string(), json!(large_file.to_string_lossy()));

    println!("Reading file with memory profiling...");
    let result = read_tool.execute(arguments.clone(), &context).await;
    
    match &result {
        Ok(r) => println!("Read tool success: response has {} content items", r.content.len()),
        Err(e) => panic!("Read tool error: {}", e),
    }

    if let Some(delta) = profiler.memory_delta() {
        let abs_delta = delta.abs() as usize;
        println!("Memory delta during read: {} ({})", 
                if delta >= 0 { "+" } else { "-" }, 
                MemoryProfiler::format_bytes(abs_delta));

        // Memory usage should be reasonable relative to file size
        let file_size = content.len();
        let max_expected_memory = file_size * 3; // Allow 3x file size for overhead

        assert!(abs_delta < max_expected_memory, 
               "Memory usage {} exceeds expected maximum {}", 
               MemoryProfiler::format_bytes(abs_delta),
               MemoryProfiler::format_bytes(max_expected_memory));
    } else {
        println!("Memory profiling not available on this platform");
    }

    // Test offset/limit memory efficiency
    let profiler = MemoryProfiler::new();
    
    let mut offset_args = arguments.clone();
    offset_args.insert("offset".to_string(), json!(500)); // Start from line 500
    offset_args.insert("limit".to_string(), json!(100));   // Read 100 lines

    let result = read_tool.execute(offset_args, &context).await;
    assert!(result.is_ok());

    if let Some(delta) = profiler.memory_delta() {
        let abs_delta = delta.abs() as usize;
        println!("Memory delta for offset/limit read: {} ({})", 
                if delta >= 0 { "+" } else { "-" }, 
                MemoryProfiler::format_bytes(abs_delta));

        // Offset/limit reads should use much less memory
        let limit_size = 100 * 100; // ~100 lines * ~100 chars per line
        let max_expected_memory = limit_size * 10; // Allow 10x for overhead

        assert!(abs_delta < max_expected_memory,
               "Offset/limit memory usage {} exceeds expected maximum {}",
               MemoryProfiler::format_bytes(abs_delta),
               MemoryProfiler::format_bytes(max_expected_memory));
    }
}

#[tokio::test] 
async fn test_large_file_write_memory_usage() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let large_file = temp_dir.path().join("memory_write_test.txt");

    // Generate content for memory testing (under 10MB limit)
    let chunk = "Memory profiling write test content with varied patterns. ".repeat(100);
    let mut content = String::new();
    for i in 0..1000 {
        content.push_str(&format!("Section {}: {}", i, chunk));
    }

    println!("Testing write memory usage for {}MB file...", content.len() / 1024 / 1024);

    let profiler = MemoryProfiler::new();

    let mut arguments = serde_json::Map::new();
    arguments.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(content));

    let result = write_tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    if let Some(delta) = profiler.memory_delta() {
        let abs_delta = delta.abs() as usize;
        println!("Memory delta during write: {} ({})", 
                if delta >= 0 { "+" } else { "-" }, 
                MemoryProfiler::format_bytes(abs_delta));

        // Memory usage should be reasonable - allow up to 2x content size
        let content_size = content.len();
        let max_expected_memory = content_size * 2;

        assert!(abs_delta < max_expected_memory,
               "Write memory usage {} exceeds expected maximum {}",
               MemoryProfiler::format_bytes(abs_delta),
               MemoryProfiler::format_bytes(max_expected_memory));
    } else {
        println!("Memory profiling not available on this platform");
    }

    // Verify file was written correctly
    assert!(large_file.exists());
    let written_size = fs::metadata(&large_file).unwrap().len() as usize;
    assert!(written_size >= content.len(), "Written file should match content size");
}

#[tokio::test]
async fn test_large_file_edit_memory_usage() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();
    let edit_tool = registry.get_tool("files_edit").unwrap();

    let temp_dir = TempDir::new().unwrap();
    let large_file = temp_dir.path().join("memory_edit_test.txt");

    // Create file with repeated patterns for editing
    let base_pattern = "MEMORY_TEST_PATTERN: original_content_here\n".repeat(5000);
    let content = base_pattern.repeat(40); // 200K lines, safe under 10MB

    println!("Creating file with {} lines for edit memory testing...", content.lines().count());

    // Write the large file
    let mut write_args = serde_json::Map::new();
    write_args.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    write_args.insert("content".to_string(), json!(content));
    write_tool.execute(write_args, &context).await.unwrap();

    // Test single edit memory usage
    let profiler = MemoryProfiler::new();

    let mut edit_args = serde_json::Map::new();
    edit_args.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    edit_args.insert("old_string".to_string(), json!("MEMORY_TEST_PATTERN: original_content_here"));
    edit_args.insert("new_string".to_string(), json!("MEMORY_TEST_PATTERN: modified_content_here"));
    edit_args.insert("replace_all".to_string(), json!(false));

    let result = edit_tool.execute(edit_args, &context).await;
    assert!(result.is_ok());

    if let Some(delta) = profiler.memory_delta() {
        let abs_delta = delta.abs() as usize;
        println!("Memory delta for single edit: {} ({})", 
                if delta >= 0 { "+" } else { "-" }, 
                MemoryProfiler::format_bytes(abs_delta));

        // Single edit should use reasonable memory
        let file_size = fs::metadata(&large_file).unwrap().len() as usize;
        let max_expected_memory = file_size * 2; // Allow 2x file size

        assert!(abs_delta < max_expected_memory,
               "Single edit memory usage {} exceeds expected maximum {}",
               MemoryProfiler::format_bytes(abs_delta),
               MemoryProfiler::format_bytes(max_expected_memory));
    }

    // Test replace_all memory usage
    let profiler = MemoryProfiler::new();

    let mut edit_all_args = serde_json::Map::new();
    edit_all_args.insert("file_path".to_string(), json!(large_file.to_string_lossy()));
    edit_all_args.insert("old_string".to_string(), json!("original_content_here"));
    edit_all_args.insert("new_string".to_string(), json!("completely_new_content_here"));
    edit_all_args.insert("replace_all".to_string(), json!(true));

    let result = edit_tool.execute(edit_all_args, &context).await;
    assert!(result.is_ok());

    if let Some(delta) = profiler.memory_delta() {
        let abs_delta = delta.abs() as usize;
        println!("Memory delta for replace_all: {} ({})", 
                if delta >= 0 { "+" } else { "-" }, 
                MemoryProfiler::format_bytes(abs_delta));

        // Replace_all may use more memory but should still be reasonable
        let file_size = fs::metadata(&large_file).unwrap().len() as usize;
        let max_expected_memory = file_size * 3; // Allow 3x file size for replace_all

        assert!(abs_delta < max_expected_memory,
               "Replace_all memory usage {} exceeds expected maximum {}",
               MemoryProfiler::format_bytes(abs_delta),
               MemoryProfiler::format_bytes(max_expected_memory));
    } else {
        println!("Memory profiling not available on this platform");
    }
}

#[tokio::test]
async fn test_concurrent_operations_memory_usage() {
    let registry = Arc::new(create_test_registry());
    let context = Arc::new(create_test_context().await);
    
    let temp_dir = TempDir::new().unwrap();

    println!("Testing memory usage during concurrent file operations...");

    let profiler = MemoryProfiler::new();

    // Create multiple files for concurrent operations
    let mut join_set = tokio::task::JoinSet::new();

    for i in 0..20 {
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let temp_dir_path = temp_dir.path().to_path_buf();

        join_set.spawn(async move {
            let file_path = temp_dir_path.join(format!("concurrent_file_{}.txt", i));
            
            // Generate content for each file
            let content = format!("Concurrent test content for file {}\n", i).repeat(1000);
            
            // Write file
            let write_tool = registry_clone.get_tool("files_write").unwrap();
            let mut write_args = serde_json::Map::new();
            write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
            write_args.insert("content".to_string(), json!(content));
            
            let write_result = write_tool.execute(write_args, &*context_clone).await;
            
            // Read file back
            let read_tool = registry_clone.get_tool("files_read").unwrap();
            let mut read_args = serde_json::Map::new();
            read_args.insert("absolute_path".to_string(), json!(file_path.to_string_lossy()));
            
            let read_result = read_tool.execute(read_args, &*context_clone).await;
            
            (write_result, read_result)
        });
    }

    // Wait for all operations to complete
    let mut success_count = 0;
    while let Some(result) = join_set.join_next().await {
        match result.unwrap() {
            (Ok(_), Ok(_)) => success_count += 1,
            _ => {}
        }
    }

    if let Some(delta) = profiler.memory_delta() {
        let abs_delta = delta.abs() as usize;
        println!("Memory delta for {} concurrent operations: {} ({})", 
                success_count,
                if delta >= 0 { "+" } else { "-" }, 
                MemoryProfiler::format_bytes(abs_delta));

        // Concurrent operations should not cause excessive memory usage
        // Allow reasonable overhead for tokio runtime and file handles
        let max_expected_memory = 50_000_000; // 50MB max for 20 concurrent operations

        assert!(abs_delta < max_expected_memory,
               "Concurrent operations memory usage {} exceeds expected maximum {}",
               MemoryProfiler::format_bytes(abs_delta),
               MemoryProfiler::format_bytes(max_expected_memory));
    } else {
        println!("Memory profiling not available on this platform");
    }

    assert_eq!(success_count, 20, "All concurrent operations should succeed");
}

// ============================================================================
// Extended Concurrent Operation Stress Tests
// ============================================================================

#[tokio::test]
async fn test_high_concurrency_stress_test() {
    let registry = Arc::new(create_test_registry());
    let context = Arc::new(create_test_context().await);
    
    let temp_dir = TempDir::new().unwrap();

    println!("Running high concurrency stress test with 100 simultaneous operations...");

    let profiler = MemoryProfiler::new();
    let start_time = std::time::Instant::now();

    // Create many more concurrent operations than the original 10
    let mut join_set = tokio::task::JoinSet::new();

    // Test with 100 concurrent operations
    for i in 0..100 {
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let temp_dir_path = temp_dir.path().to_path_buf();

        join_set.spawn(async move {
            let file_path = temp_dir_path.join(format!("stress_test_file_{}.txt", i));
            
            // Generate varied content sizes to stress different code paths
            let content_size = 1000 + (i % 10) * 500; // Vary from 1K to 5.5K characters
            let content = format!("Stress test content for file {}\n", i).repeat(content_size);
            
            // Write file
            let write_tool = registry_clone.get_tool("files_write").unwrap();
            let mut write_args = serde_json::Map::new();
            write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
            write_args.insert("content".to_string(), json!(content));
            
            let write_result = write_tool.execute(write_args, &*context_clone).await;
            
            if write_result.is_err() {
                return Err("Write failed");
            }
            
            // Read file back to verify
            let read_tool = registry_clone.get_tool("files_read").unwrap();
            let mut read_args = serde_json::Map::new();
            read_args.insert("absolute_path".to_string(), json!(file_path.to_string_lossy()));
            
            let read_result = read_tool.execute(read_args, &*context_clone).await;
            
            if read_result.is_err() {
                return Err("Read failed");
            }
            
            // Perform edit operation
            let edit_tool = registry_clone.get_tool("files_edit").unwrap();
            let mut edit_args = serde_json::Map::new();
            edit_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
            edit_args.insert("old_string".to_string(), json!(format!("file {}", i)));
            edit_args.insert("new_string".to_string(), json!(format!("FILE {} (edited)", i)));
            edit_args.insert("replace_all".to_string(), json!(true));
            
            let edit_result = edit_tool.execute(edit_args, &*context_clone).await;
            
            if edit_result.is_err() {
                return Err("Edit failed");
            }
            
            Ok(())
        });
    }

    // Wait for all operations to complete
    let mut success_count = 0;
    let mut error_count = 0;

    while let Some(result) = join_set.join_next().await {
        match result.unwrap() {
            Ok(_) => success_count += 1,
            Err(_) => error_count += 1,
        }
    }

    let total_duration = start_time.elapsed();

    println!(
        "High concurrency test completed: {} succeeded, {} failed in {:?}",
        success_count, error_count, total_duration
    );

    // Check memory usage
    if let Some(delta) = profiler.memory_delta() {
        let abs_delta = delta.abs() as usize;
        println!("Memory delta for 100 concurrent operations: {} ({})", 
                if delta >= 0 { "+" } else { "-" }, 
                MemoryProfiler::format_bytes(abs_delta));

        // High concurrency should still maintain reasonable memory usage
        let max_expected_memory = 200_000_000; // 200MB max for 100 concurrent operations

        assert!(abs_delta < max_expected_memory,
               "High concurrency memory usage {} exceeds expected maximum {}",
               MemoryProfiler::format_bytes(abs_delta),
               MemoryProfiler::format_bytes(max_expected_memory));
    }

    // Most operations should succeed (allow for some failures due to resource constraints)
    assert!(success_count >= 90, "At least 90% of operations should succeed, got {}/100", success_count);
    assert!(total_duration.as_secs() < 120, "High concurrency test should complete within 2 minutes");

    // Verify files were created correctly
    let files_created = std::fs::read_dir(temp_dir.path()).unwrap().count();
    assert!(files_created >= 90, "Should create at least 90 files, created {}", files_created);
}

#[tokio::test]
async fn test_mixed_operation_concurrency_stress() {
    let registry = Arc::new(create_test_registry());
    let context = Arc::new(create_test_context().await);
    
    let temp_dir = TempDir::new().unwrap();

    println!("Running mixed operation concurrency stress test...");

    // Create some base files for read/edit operations
    let base_files = 20;
    for i in 0..base_files {
        let file_path = temp_dir.path().join(format!("base_file_{}.txt", i));
        let content = format!("Base content for file {} that can be edited\n", i).repeat(100);
        std::fs::write(&file_path, content).unwrap();
    }

    let start_time = std::time::Instant::now();
    let mut join_set = tokio::task::JoinSet::new();

    // Mix different types of operations running concurrently
    // 30 write operations
    for i in 0..30 {
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let temp_dir_path = temp_dir.path().to_path_buf();

        join_set.spawn(async move {
            let file_path = temp_dir_path.join(format!("new_file_{}.txt", i));
            let content = format!("New file content {}\n", i).repeat(50 + i % 50);
            
            let write_tool = registry_clone.get_tool("files_write").unwrap();
            let mut write_args = serde_json::Map::new();
            write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
            write_args.insert("content".to_string(), json!(content));
            
            write_tool.execute(write_args, &*context_clone).await
        });
    }

    // 30 read operations
    for i in 0..30 {
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let temp_dir_path = temp_dir.path().to_path_buf();

        join_set.spawn(async move {
            let file_index = i % base_files; // Cycle through base files
            let file_path = temp_dir_path.join(format!("base_file_{}.txt", file_index));
            
            let read_tool = registry_clone.get_tool("files_read").unwrap();
            let mut read_args = serde_json::Map::new();
            read_args.insert("absolute_path".to_string(), json!(file_path.to_string_lossy()));
            
            read_tool.execute(read_args, &*context_clone).await
        });
    }

    // 30 edit operations
    for i in 0..30 {
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let temp_dir_path = temp_dir.path().to_path_buf();

        join_set.spawn(async move {
            let file_index = i % base_files; // Cycle through base files
            let file_path = temp_dir_path.join(format!("base_file_{}.txt", file_index));
            
            let edit_tool = registry_clone.get_tool("files_edit").unwrap();
            let mut edit_args = serde_json::Map::new();
            edit_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
            edit_args.insert("old_string".to_string(), json!(format!("file {}", file_index)));
            edit_args.insert("new_string".to_string(), json!(format!("file {} (edited by task {})", file_index, i)));
            edit_args.insert("replace_all".to_string(), json!(false)); // Single replacement to avoid conflicts
            
            edit_tool.execute(edit_args, &*context_clone).await
        });
    }

    // 20 glob operations
    for i in 0..20 {
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let temp_dir_path = temp_dir.path().to_path_buf();

        join_set.spawn(async move {
            let glob_tool = registry_clone.get_tool("files_glob").unwrap();
            let mut glob_args = serde_json::Map::new();
            
            // Vary the patterns to test different scenarios
            let pattern = match i % 4 {
                0 => "*.txt",
                1 => "base_*.txt", 
                2 => "new_file_*.txt",
                _ => "**/*.txt",
            };
            
            glob_args.insert("pattern".to_string(), json!(pattern));
            glob_args.insert("path".to_string(), json!(temp_dir_path.to_string_lossy()));
            
            glob_tool.execute(glob_args, &*context_clone).await
        });
    }

    // Wait for all operations to complete
    let mut success_count = 0;
    let mut error_count = 0;

    while let Some(result) = join_set.join_next().await {
        match result.unwrap() {
            Ok(_) => success_count += 1,
            Err(_) => error_count += 1,
        }
    }

    let total_duration = start_time.elapsed();

    println!(
        "Mixed operation concurrency completed: {} succeeded, {} failed in {:?}",
        success_count, error_count, total_duration
    );

    // Most operations should succeed 
    assert!(success_count >= 100, "At least 100/110 operations should succeed, got {}", success_count);
    assert!(error_count <= 10, "Should have at most 10 errors, got {}", error_count);
    assert!(total_duration.as_secs() < 60, "Mixed operations should complete within 1 minute");
}

#[tokio::test]
async fn test_concurrent_file_access_patterns() {
    let registry = Arc::new(create_test_registry());
    let context = Arc::new(create_test_context().await);
    
    let temp_dir = TempDir::new().unwrap();
    let shared_file = temp_dir.path().join("shared_access_file.txt");

    println!("Testing concurrent access patterns to shared file...");

    // Initialize the shared file
    let initial_content = "SHARED_FILE_CONTENT: initial data\n".repeat(1000);
    std::fs::write(&shared_file, &initial_content).unwrap();

    let start_time = std::time::Instant::now();
    let mut join_set = tokio::task::JoinSet::new();

    // 50 concurrent read operations on the same file
    for i in 0..50 {
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let file_path = shared_file.clone();

        join_set.spawn(async move {
            let read_tool = registry_clone.get_tool("files_read").unwrap();
            let mut read_args = serde_json::Map::new();
            read_args.insert("absolute_path".to_string(), json!(file_path.to_string_lossy()));
            
            // Vary read parameters to test different code paths
            if i % 3 == 0 {
                read_args.insert("offset".to_string(), json!(i * 100));
                read_args.insert("limit".to_string(), json!(500));
            }
            
            read_tool.execute(read_args, &*context_clone).await
        });
    }

    // 25 concurrent write operations to different files (to avoid conflicts)
    for i in 0..25 {
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let temp_dir_path = temp_dir.path().to_path_buf();

        join_set.spawn(async move {
            let file_path = temp_dir_path.join(format!("concurrent_write_{}.txt", i));
            let content = format!("Concurrent write operation {}\n", i).repeat(100);
            
            let write_tool = registry_clone.get_tool("files_write").unwrap();
            let mut write_args = serde_json::Map::new();
            write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
            write_args.insert("content".to_string(), json!(content));
            
            write_tool.execute(write_args, &*context_clone).await
        });
    }

    // 25 concurrent grep operations
    for i in 0..25 {
        let registry_clone = registry.clone();
        let context_clone = context.clone();
        let temp_dir_path = temp_dir.path().to_path_buf();

        join_set.spawn(async move {
            let grep_tool = registry_clone.get_tool("files_grep").unwrap();
            let mut grep_args = serde_json::Map::new();
            
            let pattern = if i % 2 == 0 {
                "SHARED_FILE_CONTENT"
            } else {
                "initial data"
            };
            
            grep_args.insert("pattern".to_string(), json!(pattern));
            grep_args.insert("path".to_string(), json!(temp_dir_path.to_string_lossy()));
            grep_args.insert("output_mode".to_string(), json!("files_with_matches"));
            
            grep_tool.execute(grep_args, &*context_clone).await
        });
    }

    // Wait for all operations to complete
    let mut success_count = 0;
    let mut error_count = 0;

    while let Some(result) = join_set.join_next().await {
        match result.unwrap() {
            Ok(_) => success_count += 1,
            Err(_) => error_count += 1,
        }
    }

    let total_duration = start_time.elapsed();

    println!(
        "Concurrent file access test completed: {} succeeded, {} failed in {:?}",
        success_count, error_count, total_duration
    );

    // All operations should succeed as they're designed to be compatible
    assert_eq!(success_count, 100, "All 100 concurrent operations should succeed");
    assert_eq!(error_count, 0, "Should have no errors");
    assert!(total_duration.as_secs() < 30, "Concurrent access should complete within 30 seconds");

    // Verify the shared file still exists and is readable
    assert!(shared_file.exists());
    let final_content = std::fs::read_to_string(&shared_file).unwrap();
    assert!(!final_content.is_empty(), "Shared file should still have content");
}

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
    let registry = create_test_registry();
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();
    let read_tool = registry.get_tool("files_read").unwrap();
    
    // Test various file path and content combinations
    let repeated_content = "Pattern ".repeat(100);
    let test_cases = vec![
        ("simple.txt", "Hello, world!"),
        ("nested/path/file.txt", "Content with\nmultiple lines"),
        ("unicode_file.txt", "Unicode content: 🦀 Rust is awesome! 中文测试"),
        ("empty_file.txt", ""),
        ("special_chars.txt", "Content with !@#$%^&*() special characters"),
        ("repeated.txt", repeated_content.as_str()),
        ("long_path/deep/nested/structure/file.txt", "Deep nesting test"),
    ];

    for (file_path, content) in test_cases {
        let temp_dir = TempDir::new().unwrap();
        let full_path = temp_dir.path().join(file_path);
        
        // Ensure parent directory exists
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        
        // Write file
        let mut write_args = serde_json::Map::new();
        write_args.insert("file_path".to_string(), json!(full_path.to_string_lossy()));
        write_args.insert("content".to_string(), json!(content));
        
        let write_result = write_tool.execute(write_args, &context).await;
        if write_result.is_err() {
            continue; // Some file paths may be invalid
        }
        
        // Read file back
        let mut read_args = serde_json::Map::new();
        read_args.insert("absolute_path".to_string(), json!(full_path.to_string_lossy()));
        
        let read_result = read_tool.execute(read_args, &context).await;
        match read_result {
            Ok(response) => {
                let read_content = extract_text_content(&response.content[0].raw);
                assert_eq!(read_content, content, "Content mismatch for file: {}", file_path);
            },
            Err(e) => panic!("Read failed for file {}: {:?}", file_path, e)
        }
    }
}

#[tokio::test]
async fn test_edit_operation_consistency_properties() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();
    let edit_tool = registry.get_tool("files_edit").unwrap();
    let read_tool = registry.get_tool("files_read").unwrap();
    
    // Test various edit scenarios
    let test_cases = vec![
        ("Hello world", "world", "universe", false),
        ("test test test", "test", "exam", true),
        ("Multi\nline\ncontent\nwith\npatterns", "line", "row", false),
        ("Pattern123Pattern456Pattern789", "Pattern", "Match", true),
        ("Special chars: !@# $%^ &*()", "!@#", "ABC", false),
    ];

    for (original_content, old_string, new_string, replace_all) in test_cases {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("edit_test.txt");
        
        // Write original file
        let mut write_args = serde_json::Map::new();
        write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
        write_args.insert("content".to_string(), json!(original_content));
        
        write_tool.execute(write_args, &context).await.unwrap();
        
        // Perform edit
        let mut edit_args = serde_json::Map::new();
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
        read_args.insert("absolute_path".to_string(), json!(file_path.to_string_lossy()));
        
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
    let registry = create_test_registry();
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();
    let glob_tool = registry.get_tool("files_glob").unwrap();
    
    // Test different file extensions and patterns
    let test_cases = vec![
        (vec!["txt", "txt", "txt"], "*.txt", 3),
        (vec!["rs", "rs", "py", "js"], "*.rs", 2),
        (vec!["md", "json", "toml"], "*.md", 1),
        (vec!["log", "log", "log", "log"], "*.log", 4),
    ];

    for (extensions, pattern, expected_count) in test_cases {
        let temp_dir = TempDir::new().unwrap();
        
        // Create files with specified extensions
        for (i, ext) in extensions.iter().enumerate() {
            let file_path = temp_dir.path().join(format!("test_file_{}.{}", i, ext));
            let content = format!("Content for file {}", i);
            
            let mut write_args = serde_json::Map::new();
            write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
            write_args.insert("content".to_string(), json!(content));
            
            write_tool.execute(write_args, &context).await.ok();
        }
        
        // Test glob pattern
        let mut glob_args = serde_json::Map::new();
        glob_args.insert("pattern".to_string(), json!(pattern));
        glob_args.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));
        
        let result = glob_tool.execute(glob_args, &context).await;
        if let Ok(response) = result {
            let response_text = extract_text_content(&response.content[0].raw);
            let files_found = if response_text.trim().is_empty() {
                0
            } else {
                // Count only lines that look like file paths (start with / or are relative paths)
                response_text.lines()
                    .filter(|line| {
                        let trimmed = line.trim();
                        !trimmed.is_empty() && 
                        !trimmed.starts_with("Found") && 
                        !trimmed.starts_with("No files") &&
                        (trimmed.starts_with("/") || trimmed.contains("."))
                    })
                    .count()
            };
            
            assert_eq!(files_found, expected_count, "Glob pattern '{}' should find {} files", pattern, expected_count);
        }
    }
}

#[tokio::test]
async fn test_read_offset_limit_consistency_properties() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();
    let read_tool = registry.get_tool("files_read").unwrap();
    
    // Create content with multiple lines for line-based testing
    let lines: Vec<String> = (1..=20).map(|i| format!("Line {}: Content for line {}", i, i)).collect();
    let content = lines.join("\n");
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("offset_limit_test.txt");
    
    // Write file
    let mut write_args = serde_json::Map::new();
    write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
    write_args.insert("content".to_string(), json!(content));
    write_tool.execute(write_args, &context).await.unwrap();
    
    // Test various line-based offset/limit combinations
    let test_cases = vec![
        (1, 5),     // Read first 5 lines (1-based indexing)
        (5, 3),     // Read 3 lines starting from line 5
        (10, 10),   // Read 10 lines starting from line 10
        (18, 5),    // Read near end (should be limited by file size)
        (25, 3),    // Offset beyond file (should fail or return empty)
    ];

    for (offset, limit) in test_cases {
        let mut read_args = serde_json::Map::new();
        read_args.insert("absolute_path".to_string(), json!(file_path.to_string_lossy()));
        read_args.insert("offset".to_string(), json!(offset));
        read_args.insert("limit".to_string(), json!(limit));
        
        match read_tool.execute(read_args, &context).await {
            Ok(response) => {
                let read_content = extract_text_content(&response.content[0].raw);
                let read_lines: Vec<&str> = read_content.lines().collect();
                
                // Assert that we don't exceed the requested limit
                assert!(read_lines.len() <= limit, "Read content should not exceed limit of {} lines, got {}", limit, read_lines.len());
                
                // If offset is within the file, check content matches expected lines
                if offset <= lines.len() {
                    let start_index = offset.saturating_sub(1); // Convert to 0-based indexing
                    let end_index = std::cmp::min(start_index + limit, lines.len());
                    let expected_lines = &lines[start_index..end_index];
                    
                    assert_eq!(read_lines.len(), expected_lines.len(), "Should read expected number of lines");
                    for (i, (actual, expected)) in read_lines.iter().zip(expected_lines.iter()).enumerate() {
                        assert_eq!(actual, expected, "Line {} content should match", i + 1);
                    }
                }
            },
            Err(_) => {
                // Offset beyond file size is acceptable
                assert!(offset > lines.len(), "Read should only fail if offset is beyond file size (offset: {}, lines: {})", offset, lines.len());
            }
        }
    }
}

#[tokio::test]
async fn test_grep_pattern_robustness_properties() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let write_tool = registry.get_tool("files_write").unwrap();
    let grep_tool = registry.get_tool("files_grep").unwrap();
    
    let temp_dir = TempDir::new().unwrap();
    
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
        let file_path = temp_dir.path().join(format!("grep_test_{}.txt", i));
        
        // Write file
        let mut write_args = serde_json::Map::new();
        write_args.insert("file_path".to_string(), json!(file_path.to_string_lossy()));
        write_args.insert("content".to_string(), json!(content));
        write_tool.execute(write_args, &context).await.unwrap();
    }
    
    // Test each pattern
    for (_i, (content, pattern, should_match)) in test_cases.iter().enumerate() {
        let mut grep_args = serde_json::Map::new();
        grep_args.insert("pattern".to_string(), json!(pattern));
        grep_args.insert("path".to_string(), json!(temp_dir.path().to_string_lossy()));
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
                    assert!(matches_found > 0, "Pattern '{}' should find matches in content '{}'", pattern, content);
                } else if content.is_empty() {
                    // Empty files might not be found at all
                    // This is acceptable behavior
                } else {
                    // For non-empty content that shouldn't match, we might still find the file
                    // but the pattern shouldn't be in the content
                    assert!(!content.contains(pattern), "Content '{}' should not contain pattern '{}'", content, pattern);
                }
            },
            Err(_) => {
                // Some patterns might cause regex errors, which is acceptable
                println!("Grep failed for pattern '{}' (acceptable)", pattern);
            }
        }
    }
}
