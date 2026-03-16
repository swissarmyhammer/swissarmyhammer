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

/// Test web search with Brave Search
#[tokio::test]
async fn test_web_search_brave() {
    let tool = WebTool::new();
    let context = create_test_context().await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("search url"));
    args.insert("query".to_string(), json!("rust programming language"));
    args.insert("results_count".to_string(), json!(3));
    args.insert("fetch_content".to_string(), json!(false));

    let start = std::time::Instant::now();
    let result = tool.execute(args, &context).await;
    let duration = start.elapsed();

    println!("Search completed in {:?}", duration);

    // Skip gracefully on rate limiting (HTTP 429) — common when running alongside
    // other tests that also hit Brave Search.
    if let Err(ref e) = result {
        if e.message.contains("429") || e.message.contains("Too Many Requests") || e.message.contains("rate") {
            eprintln!("SKIP: Brave Search rate-limited (429), skipping test");
            return;
        }
    }

    assert!(result.is_ok(), "Search should succeed: {:?}", result.err());
    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    let response_text = match &call_result.content[0].raw {
        rmcp::model::RawContent::Text(text_content) => &text_content.text,
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(response_text).unwrap();

    assert!(response["results"].is_array());
    let results = response["results"].as_array().unwrap();
    assert!(!results.is_empty(), "Should have search results");
    assert!(results.len() <= 3, "Should have at most 3 results");

    for result in results {
        assert!(result["title"].is_string());
        assert!(result["url"].is_string());

        println!("\nResult: {}", result["title"]);
        println!("URL: {}", result["url"]);
    }

    println!("\nTotal results: {}", results.len());
    println!("Search time: {}ms", response["metadata"]["search_time_ms"]);
}

/// Test web search with content fetching
#[tokio::test]
async fn test_web_search_with_content() {
    let tool = WebTool::new();
    let context = create_test_context().await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("search url"));
    args.insert("query".to_string(), json!("rust programming"));
    args.insert("results_count".to_string(), json!(2));
    args.insert("fetch_content".to_string(), json!(true));

    let start = std::time::Instant::now();
    let result = tool.execute(args, &context).await;
    let duration = start.elapsed();

    println!("Search with content fetching completed in {:?}", duration);

    // Skip gracefully on rate limiting (HTTP 429) — common when running alongside
    // other tests that also hit Brave Search.
    if let Err(ref e) = result {
        if e.message.contains("429") || e.message.contains("Too Many Requests") || e.message.contains("rate") {
            eprintln!("SKIP: Brave Search rate-limited (429), skipping test");
            return;
        }
    }

    assert!(result.is_ok());
    let call_result = result.unwrap();

    let response_text = match &call_result.content[0].raw {
        rmcp::model::RawContent::Text(text_content) => &text_content.text,
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(response_text).unwrap();

    let results = response["results"].as_array().unwrap();
    assert!(!results.is_empty());

    let with_content_count = results.iter().filter(|r| !r["content"].is_null()).count();
    println!("Results with content: {}", with_content_count);

    if let Some(stats) = response["metadata"]["content_fetch_stats"].as_object() {
        println!("Content fetch stats:");
        println!("  Attempted: {}", stats["attempted"]);
        println!("  Successful: {}", stats["successful"]);
        println!("  Failed: {}", stats["failed"]);
    }
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
