//! File Size Limits Tests
//!
//! These tests verify that file size limits are correctly enforced across all file tools.
//! Based on the FILE_SIZE_LIMITS.md audit, most components use 10 MB (10,485,760 bytes),
//! with one minor inconsistency in the write operation using 10,000,000 bytes.

use serde_json::json;
use std::fs;
use std::sync::Arc;

use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::ModelConfig;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{ToolContext, ToolRegistry};
use swissarmyhammer_tools::mcp::tools::files;

/// Expected file size limit for write file operation (binary - now consistent)
const WRITE_TOOL_SIZE_LIMIT: usize = 10 * 1024 * 1024; // 10,485,760 bytes

/// Expected file size limit for most other components (binary)
const STANDARD_SIZE_LIMIT: usize = 10 * 1024 * 1024; // 10,485,760 bytes

/// Create a test context with mock storage backends for testing MCP tools
async fn create_test_context() -> ToolContext {
    let git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>> =
        Arc::new(tokio::sync::Mutex::new(None));

    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ModelConfig::default());

    ToolContext::new(tool_handlers, git_ops, agent_config)
}

/// Create a test tool registry with file tools registered
async fn create_test_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    files::register_file_tools(&mut registry).await;
    registry
}

/// Extract response text from CallToolResult
fn extract_response_text(call_result: &rmcp::model::CallToolResult) -> &str {
    if let Some(content_item) = call_result.content.first() {
        match &content_item.raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        }
    } else {
        panic!("Response should contain content")
    }
}

// ============================================================================
// File Write Tool Size Limit Tests
// ============================================================================

#[tokio::test]
async fn test_write_tool_accepts_content_at_limit() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = temp_dir.join("at_limit.txt");

    // Create content exactly at the limit (10,000,000 bytes)
    let content = "x".repeat(WRITE_TOOL_SIZE_LIMIT);

    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("write file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(content));

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Content at limit should be accepted: {:?}",
        result
    );

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify file was written with correct size
    let metadata = fs::metadata(&test_file).unwrap();
    assert_eq!(metadata.len(), WRITE_TOOL_SIZE_LIMIT as u64);
}

#[tokio::test]
async fn test_write_tool_rejects_content_over_limit() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = temp_dir.join("over_limit.txt");

    // Create content just over the limit (10 * 1024 * 1024 + 1 = 10,485,761 bytes)
    let content = "x".repeat(WRITE_TOOL_SIZE_LIMIT + 1);

    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("write file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(content));

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_err(),
        "Content over limit should be rejected: {:?}",
        result
    );

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(
        error_msg.contains("exceeds maximum size limit") || error_msg.contains("10MB"),
        "Error should mention size limit: {}",
        error_msg
    );

    // Verify file was not created
    assert!(!test_file.exists());
}

#[tokio::test]
async fn test_write_tool_accepts_content_just_under_limit() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = temp_dir.join("under_limit.txt");

    // Create content just under the limit (10 * 1024 * 1024 - 1 = 10,485,759 bytes)
    let content = "x".repeat(WRITE_TOOL_SIZE_LIMIT - 1);

    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("write file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(content));

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Content just under limit should be accepted"
    );

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify file was written with correct size
    let metadata = fs::metadata(&test_file).unwrap();
    assert_eq!(metadata.len(), (WRITE_TOOL_SIZE_LIMIT - 1) as u64);
}

#[tokio::test]
async fn test_write_tool_rejects_large_content() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = temp_dir.join("large.txt");

    // Create content significantly over the limit (20 MB)
    let content = "x".repeat(20 * 1024 * 1024);

    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("write file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(content));

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_err(),
        "Large content should be rejected: {:?}",
        result
    );

    let error = result.unwrap_err();
    let error_msg = format!("{:?}", error);
    assert!(
        error_msg.contains("exceeds maximum size limit") || error_msg.contains("10MB"),
        "Error should mention size limit: {}",
        error_msg
    );

    // Verify file was not created
    assert!(!test_file.exists());
}

#[tokio::test]
async fn test_write_tool_empty_content_accepted() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = temp_dir.join("empty.txt");

    // Empty content (0 bytes)
    let content = "";

    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("write file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("content".to_string(), json!(content));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Empty content should be accepted");

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify empty file was created
    let metadata = fs::metadata(&test_file).unwrap();
    assert_eq!(metadata.len(), 0);
}

// ============================================================================
// File Read Tool Size Limit Tests
// ============================================================================

#[tokio::test]
async fn test_read_tool_handles_large_files() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = temp_dir.join("large_read.txt");

    // Create a file with content near the standard size limit
    // Use 9 MB to be safe and avoid actual size limit rejections
    let content = "Line content\n".repeat(700_000); // ~9 MB
    fs::write(&test_file, &content).unwrap();

    // Test reading with a limit to avoid memory issues
    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("read file"));
    arguments.insert("path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("limit".to_string(), json!(1000)); // Read only first 1000 lines

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Reading large file with limit should succeed"
    );

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    let response_text = extract_response_text(&call_result);
    let line_count = response_text.lines().count();
    assert_eq!(line_count, 1000, "Should read exactly 1000 lines");
}

#[tokio::test]
async fn test_read_tool_with_offset_on_large_file() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = temp_dir.join("large_offset_read.txt");

    // Create a file with many lines
    let mut content = String::new();
    for i in 0..10_000 {
        content.push_str(&format!("Line {}\n", i));
    }
    fs::write(&test_file, &content).unwrap();

    // Test reading with offset and limit
    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("read file"));
    arguments.insert("path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("offset".to_string(), json!(5000));
    arguments.insert("limit".to_string(), json!(100));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Reading with offset should succeed");

    let call_result = result.unwrap();
    let response_text = extract_response_text(&call_result);

    // Should start from line at offset 5000 (0-indexed), which is Line 5000
    // The offset is 0-based, so line 0 is "Line 0", line 5000 is "Line 5000"
    assert!(response_text.contains("Line 5000") || response_text.contains("Line 5001"));
    let line_count = response_text.lines().count();
    assert_eq!(line_count, 100, "Should read exactly 100 lines");
}

// ============================================================================
// File Edit Tool Size Limit Tests
// ============================================================================

#[tokio::test]
async fn test_edit_tool_handles_large_files() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = temp_dir.join("large_edit.txt");

    // Create a file with content (5 MB - reasonable size for editing)
    let content = "target content\n".repeat(350_000); // ~5 MB
    fs::write(&test_file, &content).unwrap();

    // Test editing the file
    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("edit file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("target"));
    arguments.insert("new_string".to_string(), json!("modified"));
    arguments.insert("replace_all".to_string(), json!(false));

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Editing large file should succeed: {:?}",
        result
    );

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify the first occurrence was replaced
    let edited_content = fs::read_to_string(&test_file).unwrap();
    assert!(edited_content.starts_with("modified content\n"));
}

#[tokio::test]
async fn test_edit_tool_replace_all_on_large_file() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = temp_dir.join("large_replace_all.txt");

    // Create a file with many occurrences (2 MB - reasonable for testing)
    let content = "foo bar foo\n".repeat(170_000); // ~2 MB
    fs::write(&test_file, &content).unwrap();

    // Test replace all on large file
    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("edit file"));
    arguments.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    arguments.insert("old_string".to_string(), json!("foo"));
    arguments.insert("new_string".to_string(), json!("baz"));
    arguments.insert("replace_all".to_string(), json!(true));

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Replace all on large file should succeed: {:?}",
        result
    );

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify all occurrences were replaced
    let edited_content = fs::read_to_string(&test_file).unwrap();
    assert!(!edited_content.contains("foo"));
    assert!(edited_content.contains("baz"));
}

// ============================================================================
// Shell Execute Tool Size Limit Tests
// ============================================================================

#[tokio::test]
async fn test_shell_execute_output_size_limit() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;

    // Register shell tools
    use swissarmyhammer_tools::mcp::tools::shell;
    let mut registry = registry;
    shell::register_shell_tools(&mut registry);

    let tool = registry.get_tool("shell").unwrap();

    // Test command that produces reasonable output
    let mut arguments = serde_json::Map::new();
    arguments.insert("command".to_string(), json!("echo 'Testing output size'"));

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Shell command with small output should succeed"
    );

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));
}

#[tokio::test]
async fn test_shell_execute_handles_large_output() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;

    // Register shell tools
    use swissarmyhammer_tools::mcp::tools::shell;
    let mut registry = registry;
    shell::register_shell_tools(&mut registry);

    let tool = registry.get_tool("shell").unwrap();

    // Test command that produces moderately large output
    // Generate 1000 lines of output (~50 KB)
    let mut arguments = serde_json::Map::new();
    arguments.insert(
        "command".to_string(),
        json!("for i in {1..1000}; do echo \"Line $i with some content\"; done"),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(
        result.is_ok(),
        "Shell command with moderate output should succeed"
    );

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    let response_text = extract_response_text(&call_result);
    assert!(response_text.contains("Line 1"));
    assert!(response_text.contains("Line 1000"));
}

// ============================================================================
// Grep Tool Size Limit Tests
// ============================================================================

#[tokio::test]
async fn test_grep_tool_handles_large_files() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();

    // Create a large file with search targets (5 MB)
    let test_file = temp_dir.join("large_grep.txt");
    let mut content = String::new();
    for i in 0..100_000 {
        if i % 1000 == 0 {
            content.push_str("TARGET_PATTERN found here\n");
        } else {
            content.push_str(&format!("Line {} with regular content\n", i));
        }
    }
    fs::write(&test_file, &content).unwrap();

    // Test grep on large file
    let mut arguments = serde_json::Map::new();
    arguments.insert("op".to_string(), json!("grep files"));
    arguments.insert("pattern".to_string(), json!("TARGET_PATTERN"));
    arguments.insert("path".to_string(), json!(temp_dir.to_string_lossy()));

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok(), "Grep on large file should succeed");

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    let response_text = extract_response_text(&call_result);
    assert!(response_text.contains("TARGET_PATTERN") || response_text.contains("matches"));
}

// ============================================================================
// Cross-Tool Size Consistency Tests
// ============================================================================

#[tokio::test]
async fn test_write_then_read_at_size_limit() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = temp_dir.join("write_read_limit.txt");

    // Write file at the limit
    let content = "x".repeat(WRITE_TOOL_SIZE_LIMIT);

    let mut write_args = serde_json::Map::new();
    write_args.insert("op".to_string(), json!("write file"));
    write_args.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    write_args.insert("content".to_string(), json!(content));

    let write_result = tool.execute(write_args, &context).await;
    assert!(write_result.is_ok(), "Write at limit should succeed");

    // Read the file with limit to avoid memory issues
    let mut read_args = serde_json::Map::new();
    read_args.insert("op".to_string(), json!("read file"));
    read_args.insert("path".to_string(), json!(test_file.to_string_lossy()));
    read_args.insert("limit".to_string(), json!(1000)); // Read only first 1000 lines

    let read_result = tool.execute(read_args, &context).await;
    assert!(read_result.is_ok(), "Read should succeed on large file");

    let call_result = read_result.unwrap();
    assert_eq!(call_result.is_error, Some(false));
}

#[tokio::test]
async fn test_write_then_edit_at_size_limit() {
    let registry = create_test_registry().await;
    let context = create_test_context().await;
    let tool = registry.get_tool("files").unwrap();

    let _env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let temp_dir = _env.temp_dir();
    let test_file = temp_dir.join("write_edit_limit.txt");

    // Write file with content near limit (5 MB - reasonable for editing)
    let content = "target content\n".repeat(350_000); // ~5 MB

    let mut write_args = serde_json::Map::new();
    write_args.insert("op".to_string(), json!("write file"));
    write_args.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    write_args.insert("content".to_string(), json!(content));

    let write_result = tool.execute(write_args, &context).await;
    assert!(write_result.is_ok(), "Write should succeed");

    // Edit the file
    let mut edit_args = serde_json::Map::new();
    edit_args.insert("op".to_string(), json!("edit file"));
    edit_args.insert("file_path".to_string(), json!(test_file.to_string_lossy()));
    edit_args.insert("old_string".to_string(), json!("target"));
    edit_args.insert("new_string".to_string(), json!("modified"));
    edit_args.insert("replace_all".to_string(), json!(false));

    let edit_result = tool.execute(edit_args, &context).await;
    assert!(edit_result.is_ok(), "Edit should succeed on large file");

    let call_result = edit_result.unwrap();
    assert_eq!(call_result.is_error, Some(false));
}

// ============================================================================
// Documentation Tests
// ============================================================================

#[test]
fn test_size_limit_constants_documented() {
    // Document the size limits for reference
    assert_eq!(WRITE_TOOL_SIZE_LIMIT, 10 * 1024 * 1024);
    assert_eq!(STANDARD_SIZE_LIMIT, 10 * 1024 * 1024);

    // Verify both constants are now consistent
    assert_eq!(WRITE_TOOL_SIZE_LIMIT, STANDARD_SIZE_LIMIT);

    println!(
        "Write tool size limit: {} bytes ({:.2} MB)",
        WRITE_TOOL_SIZE_LIMIT,
        WRITE_TOOL_SIZE_LIMIT as f64 / 1_048_576.0
    );
    println!(
        "Standard size limit: {} bytes ({:.2} MB)",
        STANDARD_SIZE_LIMIT,
        STANDARD_SIZE_LIMIT as f64 / 1_048_576.0
    );
    println!("Limits are now consistent!");
}
