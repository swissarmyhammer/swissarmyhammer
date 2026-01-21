//! Integration tests for web_search functionality
//!
//! These tests verify that web_search works end-to-end with real Chrome/Chromium browser.
//! Tests are marked with #[ignore] by default since they require Chrome to be installed.
//! Run with: `cargo test --test integration web_search -- --ignored`

use serde_json::json;
use std::sync::Arc;
use swissarmyhammer_config::ModelConfig;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolContext};
use swissarmyhammer_tools::mcp::tools::web_search::{chrome_detection, search::WebSearchTool};

/// Helper function to create a test context for integration tests
async fn create_test_context() -> ToolContext {
    let git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ModelConfig::default());

    ToolContext::new(tool_handlers, git_ops, agent_config)
}

/// Test that Chrome is detected on this system
#[test]
fn test_chrome_detection_on_system() {
    let result = chrome_detection::detect_chrome();

    if !result.found {
        println!("WARNING: Chrome not found on this system");
        println!("{}", result.message);
        println!("\n{}", result.installation_instructions());
        println!("\nSkipping Chrome-dependent tests");
    } else {
        println!("Chrome found: {}", result.path.as_ref().unwrap().display());
        println!(
            "Detection method: {}",
            result.detection_method.as_ref().unwrap()
        );
    }

    // Test passes regardless of Chrome availability - just reports status
    assert!(
        !result.paths_checked.is_empty(),
        "Should have checked some paths"
    );
}

/// Test web_search with real Chrome (ignored by default)
///
/// This test requires Chrome to be installed and will launch a real browser.
/// Run with: `cargo test --test integration test_web_search_real_chrome -- --ignored --nocapture`
#[tokio::test]
#[ignore]
async fn test_web_search_real_chrome() {
    // First check if Chrome is available
    let chrome_result = chrome_detection::detect_chrome();
    if !chrome_result.found {
        println!("SKIPPED: Chrome not found on this system");
        println!("{}", chrome_result.installation_instructions());
        return;
    }

    println!(
        "Testing web search with Chrome at: {}",
        chrome_result.path.as_ref().unwrap().display()
    );

    let tool = WebSearchTool::new();
    let context = create_test_context().await;

    // Perform a simple search without content fetching for speed
    let mut args = serde_json::Map::new();
    args.insert("query".to_string(), json!("rust programming language"));
    args.insert("results_count".to_string(), json!(3));
    args.insert("fetch_content".to_string(), json!(false));

    let start = std::time::Instant::now();
    let result = tool.execute(args, &context).await;
    let duration = start.elapsed();

    println!("Search completed in {:?}", duration);

    // Verify result
    assert!(result.is_ok(), "Search should succeed: {:?}", result.err());
    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Parse response
    let response_text = match &call_result.content[0].raw {
        rmcp::model::RawContent::Text(text_content) => &text_content.text,
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(response_text).unwrap();

    // Verify we got results
    assert!(response["results"].is_array());
    let results = response["results"].as_array().unwrap();
    assert!(!results.is_empty(), "Should have search results");
    assert!(results.len() <= 3, "Should have at most 3 results");

    // Verify result structure
    for result in results {
        assert!(result["title"].is_string());
        assert!(result["url"].is_string());
        assert!(result["description"].is_string());

        println!("\nResult: {}", result["title"]);
        println!("URL: {}", result["url"]);
    }

    println!("\nTotal results: {}", results.len());
    println!("Search time: {}ms", response["metadata"]["search_time_ms"]);
}

/// Test web_search with content fetching (ignored by default - slower)
///
/// Run with: `cargo test --test integration test_web_search_with_content -- --ignored --nocapture`
#[tokio::test]
#[ignore]
async fn test_web_search_with_content() {
    // First check if Chrome is available
    let chrome_result = chrome_detection::detect_chrome();
    if !chrome_result.found {
        println!("SKIPPED: Chrome not found on this system");
        return;
    }

    let tool = WebSearchTool::new();
    let context = create_test_context().await;

    // Perform search with content fetching (slower)
    let mut args = serde_json::Map::new();
    args.insert("query".to_string(), json!("rust programming"));
    args.insert("results_count".to_string(), json!(2));
    args.insert("fetch_content".to_string(), json!(true));

    let start = std::time::Instant::now();
    let result = tool.execute(args, &context).await;
    let duration = start.elapsed();

    println!("Search with content fetching completed in {:?}", duration);

    assert!(result.is_ok());
    let call_result = result.unwrap();

    // Parse response
    let response_text = match &call_result.content[0].raw {
        rmcp::model::RawContent::Text(text_content) => &text_content.text,
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(response_text).unwrap();

    // Verify we got results with content
    let results = response["results"].as_array().unwrap();
    assert!(!results.is_empty());

    // Check if any results have content fetched
    let with_content_count = results.iter().filter(|r| !r["content"].is_null()).count();
    println!("Results with content: {}", with_content_count);

    // Print metadata about content fetching
    if let Some(stats) = response["metadata"]["content_fetch_stats"].as_object() {
        println!("Content fetch stats:");
        println!("  Attempted: {}", stats["attempted"]);
        println!("  Successful: {}", stats["successful"]);
        println!("  Failed: {}", stats["failed"]);
    }
}

/// Test web_search error handling when Chrome launches but search fails
#[tokio::test]
#[ignore]
async fn test_web_search_error_handling() {
    let chrome_result = chrome_detection::detect_chrome();
    if !chrome_result.found {
        println!("SKIPPED: Chrome not found");
        return;
    }

    let tool = WebSearchTool::new();
    let context = create_test_context().await;

    // Try an empty query (should fail validation)
    let mut args = serde_json::Map::new();
    args.insert("query".to_string(), json!(""));

    let result = tool.execute(args, &context).await;

    // Should fail with validation error
    assert!(result.is_err(), "Empty query should fail");

    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("cannot be empty") || err_msg.contains("empty"),
        "Error should mention empty query: {}",
        err_msg
    );
}
