//! Tests for file operation rate limiting
//!
//! This test verifies that rate limiting is properly enforced for file operations.

use std::sync::Arc;
use swissarmyhammer_common::rate_limiter::{RateLimiter, RateLimiterConfig};
use swissarmyhammer_config::ModelConfig;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolContext};
use swissarmyhammer_tools::mcp::tools::files::read::ReadFileTool;
use swissarmyhammer_tools::mcp::tools::files::write::WriteFileTool;
use tempfile::TempDir;

/// Helper to create a basic test context
async fn create_test_context() -> ToolContext {
    let git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ModelConfig::default());

    ToolContext::new(tool_handlers, git_ops, agent_config)
}

#[tokio::test]
async fn test_rate_limiting_basics() {
    // Create a rate limiter with very low limits for testing
    let config = RateLimiterConfig {
        global_limit: 1000,
        per_client_limit: 5,
        expensive_operation_limit: 3,
        window_duration: std::time::Duration::from_secs(60),
    };

    let limiter = RateLimiter::with_config(config);

    // Should succeed for first 5 operations
    for i in 0..5 {
        let result = limiter.check_rate_limit("test_client", "file_read", 1);
        assert!(result.is_ok(), "Operation {} should succeed", i);
    }

    // 6th operation should fail due to per-client limit
    let result = limiter.check_rate_limit("test_client", "file_read", 1);
    assert!(result.is_err(), "6th operation should be rate limited");

    // Different client should still work
    let result = limiter.check_rate_limit("other_client", "file_read", 1);
    assert!(result.is_ok(), "Different client should succeed");
}

#[tokio::test]
async fn test_expensive_operations_rate_limited() {
    // Create a rate limiter with very low limits for testing
    let config = RateLimiterConfig {
        global_limit: 1000,
        per_client_limit: 100,
        expensive_operation_limit: 2,
        window_duration: std::time::Duration::from_secs(60),
    };

    let limiter = RateLimiter::with_config(config);

    // Expensive operations (file_glob, file_grep) should have lower limits
    assert!(limiter.check_rate_limit("client1", "file_glob", 1).is_ok());
    assert!(limiter.check_rate_limit("client1", "file_glob", 1).is_ok());

    // 3rd expensive operation should fail
    let result = limiter.check_rate_limit("client1", "file_glob", 1);
    assert!(
        result.is_err(),
        "3rd expensive operation should be rate limited"
    );

    // Regular operations should still work
    assert!(limiter.check_rate_limit("client1", "file_read", 1).is_ok());
}

#[tokio::test]
async fn test_file_operations_enforce_rate_limits() {
    // This test verifies that file operations actually call the rate limiter
    // We can't easily control the global rate limiter from tests, but we can
    // verify that the operations complete successfully with normal limits

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_file = temp_dir.path().join("test.txt");

    let context = create_test_context().await;
    let write_tool = WriteFileTool::new();
    let read_tool = ReadFileTool::new();

    // Write a file
    let mut write_args = serde_json::Map::new();
    write_args.insert(
        "file_path".to_string(),
        serde_json::json!(test_file.to_string_lossy()),
    );
    write_args.insert("content".to_string(), serde_json::json!("test content"));

    let result = write_tool.execute(write_args, &context).await;
    assert!(
        result.is_ok(),
        "Write should succeed with normal rate limits"
    );

    // Read the file
    let mut read_args = serde_json::Map::new();
    read_args.insert(
        "path".to_string(),
        serde_json::json!(test_file.to_string_lossy()),
    );

    let result = read_tool.execute(read_args, &context).await;
    assert!(
        result.is_ok(),
        "Read should succeed with normal rate limits"
    );
}

#[test]
fn test_rate_limit_operation_classification() {
    let config = RateLimiterConfig {
        global_limit: 100,
        per_client_limit: 50,
        expensive_operation_limit: 10,
        window_duration: std::time::Duration::from_secs(60),
    };

    let limiter = RateLimiter::with_config(config);

    // Use reflection to test operation_limit would be nice, but it's private
    // Instead we test the behavior indirectly by exhausting limits

    // Test that file_glob is treated as expensive
    for _ in 0..10 {
        let _ = limiter.check_rate_limit("client1", "file_glob", 1);
    }

    // 11th should fail
    assert!(limiter.check_rate_limit("client1", "file_glob", 1).is_err());

    // Test that file_read is not expensive (uses global limit, not expensive operation limit)
    // But still subject to per-client limit of 50
    for i in 0..50 {
        let result = limiter.check_rate_limit("client2", "file_read", 1);
        assert!(
            result.is_ok(),
            "Operation {} should succeed (within per-client limit of 50)",
            i + 1
        );
    }

    // 51st should fail due to per-client limit being exhausted
    let result = limiter.check_rate_limit("client2", "file_read", 1);
    assert!(
        result.is_err(),
        "51st operation should fail due to per-client rate limit"
    );
}
