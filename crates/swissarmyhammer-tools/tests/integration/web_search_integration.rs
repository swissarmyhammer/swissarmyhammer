//! Integration tests for web search functionality
//!
//! These tests verify that the unified web tool's search operation works end-to-end
//! with Brave Search via direct HTTP requests (no browser needed).

use serde_json::json;
use std::sync::Arc;
use swissarmyhammer_config::ModelConfig;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolContext};
use swissarmyhammer_tools::mcp::tools::web::WebTool;

/// Helper function to create a test context for integration tests
async fn create_test_context() -> ToolContext {
    let git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ModelConfig::default());

    ToolContext::new(tool_handlers, git_ops, agent_config)
}

/// Test web search error handling for empty query
#[tokio::test]
async fn test_web_search_error_handling() {
    let tool = WebTool::new();
    let context = create_test_context().await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("search url"));
    args.insert("query".to_string(), json!(""));

    let result = tool.execute(args, &context).await;

    assert!(result.is_err(), "Empty query should fail");

    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("cannot be empty") || err_msg.contains("empty"),
        "Error should mention empty query: {}",
        err_msg
    );
}
