//! Integration tests for the notify tool
//!
//! These tests verify that the notify tool works correctly through all layers of the system,
//! including MCP protocol handling, tool registry integration, and end-to-end scenarios.

use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use swissarmyhammer_common::rate_limiter::{RateLimiter, RateLimiterConfig};
use swissarmyhammer_issues::{FileSystemIssueStorage, IssueStorage};
use swissarmyhammer::memoranda::{MarkdownMemoStorage, MemoStorage};
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{ToolContext, ToolRegistry};
use swissarmyhammer_tools::mcp::tools::notify;
use tokio::time::{timeout, Duration};

/// Creates a test rate limiter with generous limits suitable for testing
fn create_test_rate_limiter() -> Arc<RateLimiter> {
    Arc::new(RateLimiter::with_config(RateLimiterConfig {
        global_limit: 10000,                     // Very high global limit
        per_client_limit: 1000,                  // High per-client limit
        expensive_operation_limit: 500,          // High expensive operation limit
        window_duration: Duration::from_secs(1), // Short refill window for tests
    }))
}

/// Create a test context with mock storage backends for testing MCP tools
async fn create_test_context() -> ToolContext {
    let issue_storage: Arc<tokio::sync::RwLock<Box<dyn IssueStorage>>> =
        Arc::new(tokio::sync::RwLock::new(Box::new(
            FileSystemIssueStorage::new(PathBuf::from("./test_issues")).unwrap(),
        )));
    let git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    // Create temporary directory for memo storage in tests
    let temp_dir = tempfile::tempdir().unwrap();
    let memo_temp_dir = temp_dir.path().join("memos");
    let memo_storage: Arc<tokio::sync::RwLock<Box<dyn MemoStorage>>> = Arc::new(
        tokio::sync::RwLock::new(Box::new(MarkdownMemoStorage::new(memo_temp_dir))),
    );

    let rate_limiter = create_test_rate_limiter();

    let tool_handlers = Arc::new(ToolHandlers::new(memo_storage.clone()));

    ToolContext::new(
        tool_handlers,
        issue_storage,
        git_ops,
        memo_storage,
        rate_limiter,
    )
}

/// Create a test tool registry with notify tool registered
fn create_test_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    notify::register_notify_tools(&mut registry);
    registry
}

// ============================================================================
// MCP Protocol Integration Tests
// ============================================================================

#[tokio::test]
async fn test_notify_tool_discovery_and_registration() {
    let registry = create_test_registry();

    // Verify the notify tool is registered and discoverable
    assert!(registry.get_tool("notify_create").is_some());

    let tool_names = registry.list_tool_names();
    assert!(tool_names.contains(&"notify_create".to_string()));

    // Verify tool metadata is accessible
    let tool = registry.get_tool("notify_create").unwrap();
    assert_eq!(tool.name(), "notify_create");
    assert!(!tool.description().is_empty());
    assert!(tool.description().contains("notification"));

    // Verify schema structure
    let schema = tool.schema();
    assert!(schema.is_object());
    let properties = schema["properties"].as_object().unwrap();
    assert!(properties.contains_key("message"));
    assert!(properties.contains_key("level"));
    assert!(properties.contains_key("context"));

    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&serde_json::Value::String("message".to_string())));
}

#[tokio::test]
async fn test_notify_tool_execution_success_cases() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("notify_create").unwrap();

    // Test basic notification execution
    let mut arguments = serde_json::Map::new();
    arguments.insert(
        "message".to_string(),
        json!("Integration test notification"),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));
    assert!(!call_result.content.is_empty());

    // Verify the response contains the expected success message
    let content_text = call_result.content[0].as_text().unwrap().text.clone();
    assert!(content_text.contains("Notification sent"));
    assert!(content_text.contains("Integration test notification"));
}

#[tokio::test]
async fn test_notify_tool_with_all_parameters() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("notify_create").unwrap();

    // Test notification with all parameters
    let mut arguments = serde_json::Map::new();
    arguments.insert("message".to_string(), json!("Full parameter test"));
    arguments.insert("level".to_string(), json!("warn"));
    arguments.insert(
        "context".to_string(),
        json!({"stage": "integration_test", "file_count": 42}),
    );

    let result = tool.execute(arguments, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    let content_text = call_result.content[0].as_text().unwrap().text.clone();
    assert!(content_text.contains("Notification sent"));
    assert!(content_text.contains("Full parameter test"));
}

#[tokio::test]
async fn test_notify_tool_validation_errors() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("notify_create").unwrap();

    // Test missing required parameter
    let empty_args = serde_json::Map::new();
    let result = tool.execute(empty_args, &context).await;
    assert!(result.is_err());

    // Test empty message validation
    let mut args = serde_json::Map::new();
    args.insert("message".to_string(), json!(""));
    let result = tool.execute(args, &context).await;
    assert!(result.is_err());

    // Test whitespace-only message
    let mut args = serde_json::Map::new();
    args.insert("message".to_string(), json!("   \n\t   "));
    let result = tool.execute(args, &context).await;
    assert!(result.is_err());

    // Test invalid parameter types
    let mut args = serde_json::Map::new();
    args.insert("message".to_string(), json!(123)); // Wrong type
    let result = tool.execute(args, &context).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_notify_tool_different_levels() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("notify_create").unwrap();

    // Test each notification level
    let levels = ["info", "warn", "error"];

    for level in levels {
        let mut args = serde_json::Map::new();
        args.insert(
            "message".to_string(),
            json!(format!("Test {} message", level)),
        );
        args.insert("level".to_string(), json!(level));

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok(), "Failed for level: {level}");

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));

        let content_text = call_result.content[0].as_text().unwrap().text.clone();
        assert!(content_text.contains("Notification sent"));
        assert!(content_text.contains(&format!("Test {level} message")));
    }
}

#[tokio::test]
async fn test_notify_tool_complex_context() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("notify_create").unwrap();

    let mut args = serde_json::Map::new();
    args.insert("message".to_string(), json!("Complex context test"));
    args.insert(
        "context".to_string(),
        json!({
            "nested": {
                "data": "value",
                "numbers": [1, 2, 3],
                "boolean": true
            },
            "array": ["a", "b", "c"],
            "unicode": "通知消息 🔔"
        }),
    );

    let result = tool.execute(args, &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));
}

#[tokio::test]
async fn test_notify_tool_unicode_and_special_characters() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("notify_create").unwrap();

    // Test Unicode message
    let unicode_message = "通知消息 🔔 with émojis and ñoñ-ASCII characters";
    let mut args = serde_json::Map::new();
    args.insert("message".to_string(), json!(unicode_message));

    let result = tool.execute(args, &context).await;
    assert!(result.is_ok());

    // Test special characters
    let special_message = r#"Special chars: {}[]()\"'`~!@#$%^&*-_+=|\\/:;<>,.?"#;
    let mut args = serde_json::Map::new();
    args.insert("message".to_string(), json!(special_message));

    let result = tool.execute(args, &context).await;
    assert!(result.is_ok());

    // Test multiline message
    let multiline_message = "Line 1\nLine 2\nLine 3\n\nLine 5";
    let mut args = serde_json::Map::new();
    args.insert("message".to_string(), json!(multiline_message));

    let result = tool.execute(args, &context).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_notify_tool_error_recovery() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("notify_create").unwrap();

    // Test recovery after validation errors
    let mut bad_args = serde_json::Map::new();
    bad_args.insert("message".to_string(), json!("")); // Empty message

    let result = tool.execute(bad_args, &context).await;
    assert!(result.is_err());

    // Verify tool still works after error
    let mut good_args = serde_json::Map::new();
    good_args.insert("message".to_string(), json!("Recovery test"));

    let result = tool.execute(good_args, &context).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_notify_tool_rate_limiting_integration() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("notify_create").unwrap();

    // Test multiple notifications succeed (generous rate limiter allows all in tests)
    for i in 0..5 {
        let mut args = serde_json::Map::new();
        args.insert(
            "message".to_string(),
            json!(format!("Rate limit test {}", i)),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_notify_tool_performance_characteristics() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("notify_create").unwrap();

    let start_time = std::time::Instant::now();
    let num_operations = 50;

    // Perform many notifications and measure time
    for i in 0..num_operations {
        let mut args = serde_json::Map::new();
        args.insert(
            "message".to_string(),
            json!(format!("Performance test {}", i)),
        );

        let result = timeout(Duration::from_millis(100), tool.execute(args, &context)).await;

        assert!(result.is_ok(), "Timeout on operation {i}");
        let execution_result = result.unwrap();
        assert!(
            execution_result.is_ok(),
            "Execution failed on operation {i}"
        );
    }

    let elapsed = start_time.elapsed();

    // Performance assertion: should handle 50 notifications reasonably quickly
    assert!(
        elapsed < Duration::from_secs(2),
        "Performance test took too long: {elapsed:?}"
    );
}

#[tokio::test]
async fn test_notify_tool_resource_cleanup() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("notify_create").unwrap();

    // Execute many notifications and verify no resource leaks
    for i in 0..30 {
        let mut args = serde_json::Map::new();
        args.insert("message".to_string(), json!(format!("Cleanup test {}", i)));

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());
    }

    // Verify context is still valid after many operations
    let mut final_args = serde_json::Map::new();
    final_args.insert("message".to_string(), json!("Final cleanup test"));

    let result = tool.execute(final_args, &context).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_realistic_usage_scenarios() {
    let registry = create_test_registry();
    let context = create_test_context().await;
    let tool = registry.get_tool("notify_create").unwrap();

    // Scenario 1: Code analysis notification
    let mut args = serde_json::Map::new();
    args.insert(
        "message".to_string(),
        json!("Found potential security vulnerability in authentication logic at line 145"),
    );
    args.insert("level".to_string(), json!("warn"));
    args.insert(
        "context".to_string(),
        json!({"file": "auth.rs", "line": 145, "severity": "medium"}),
    );

    let result = tool.execute(args, &context).await;
    assert!(result.is_ok());

    // Scenario 2: Workflow status update
    let mut args = serde_json::Map::new();
    args.insert(
        "message".to_string(),
        json!("Processing large codebase - this may take a few minutes"),
    );
    args.insert("level".to_string(), json!("info"));
    args.insert(
        "context".to_string(),
        json!({"stage": "analysis", "total_files": 247}),
    );

    let result = tool.execute(args, &context).await;
    assert!(result.is_ok());

    // Scenario 3: Decision point communication
    let mut args = serde_json::Map::new();
    args.insert(
        "message".to_string(),
        json!("Automatically selected main branch as merge target based on git history"),
    );
    args.insert("level".to_string(), json!("info"));
    args.insert(
        "context".to_string(),
        json!({"action": "merge_target_selection", "branch": "main", "confidence": 0.95}),
    );

    let result = tool.execute(args, &context).await;
    assert!(result.is_ok());
}
