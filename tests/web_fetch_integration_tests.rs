use serde_json::json;
use std::time::Duration;
use swissarmyhammer_tools::mcp::tools::web_fetch::fetch::WebFetchTool;
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolContext};
use swissarmyhammer::common::rate_limiter::RateLimiter;

/// Integration tests for the web_fetch tool based on the specification use cases
/// These tests validate the tool against real-world scenarios from the specification

/// Creates a test tool context for integration testing
fn create_test_context() -> ToolContext {
    ToolContext {
        rate_limiter: RateLimiter::new(),
    }
}

/// Test case 1: Documentation Research from the specification
/// URL: "https://docs.rust-lang.org/book/ch04-01-what-is-ownership.html"
#[tokio::test]
async fn test_specification_use_case_documentation_research() {
    let tool = WebFetchTool::new();
    let context = create_test_context();

    // Test parameters as specified in the specification
    let args = json!({
        "url": "https://docs.rust-lang.org/book/ch04-01-what-is-ownership.html",
        "timeout": 45,
        "max_content_length": 2097152 // 2MB as specified
    });

    let result = tool.execute(args.as_object().unwrap().clone(), &context).await;
    
    match result {
        Ok(call_result) => {
            // Verify the response structure
            assert!(!call_result.content.is_empty());
            assert_eq!(call_result.is_error, Some(false));
            
            // Verify the response contains the fetched markdown content directly
            let response_text = match &call_result.content[0].resource {
                swissarmyhammer_tools::rmcp::model::RawContent::Text(text_content) => &text_content.text,
                _ => panic!("Expected text content"),
            };
            
            // Verify content quality for documentation
            assert!(response_text.contains("ownership"), "Content should contain ownership-related information");
            assert!(response_text.len() > 1000, "Should have substantial content");

            
            println!("✅ Documentation research use case passed");
        }
        Err(e) => {
            // Network failures are acceptable in CI environments
            if e.to_string().contains("Connection") || e.to_string().contains("timeout") {
                println!("⚠️  Network error acceptable in CI: {}", e);
            } else {
                panic!("Unexpected error in documentation research test: {}", e);
            }
        }
    }
}

/// Test case 2: API Documentation Processing from the specification
/// URL: "https://api.github.com/docs/rest/repos"
#[tokio::test]
async fn test_specification_use_case_api_documentation() {
    let tool = WebFetchTool::new();
    let context = create_test_context();

    let args = json!({
        "url": "https://docs.github.com/en/rest/repos/repos",
        "user_agent": "SwissArmyHammer-DocProcessor/1.0"
    });

    let result = tool.execute(args.as_object().unwrap().clone(), &context).await;
    
    match result {
        Ok(call_result) => {
            assert_eq!(call_result.is_error, Some(false));
            
            let response_text = match &call_result.content[0].resource {
                swissarmyhammer_tools::rmcp::model::RawContent::Text(text_content) => &text_content.text,
                _ => panic!("Expected text content"),
            };
            
            // Verify API documentation content
            assert!(
                response_text.to_lowercase().contains("repo") || 
                response_text.to_lowercase().contains("repository"),
                "Should contain repository-related API documentation"
            );
            
            println!("✅ API documentation processing use case passed");
        }
        Err(e) => {
            if e.to_string().contains("Connection") || e.to_string().contains("timeout") {
                println!("⚠️  Network error acceptable in CI: {}", e);
            } else {
                panic!("Unexpected error in API documentation test: {}", e);
            }
        }
    }
}

/// Test case 3: Content Validation from the specification
/// Focuses on redirect handling and timeout configuration
#[tokio::test]
async fn test_specification_use_case_content_validation() {
    let tool = WebFetchTool::new();
    let context = create_test_context();

    let args = json!({
        "url": "https://httpbin.org/redirect/2",
        "follow_redirects": true,
        "timeout": 15
    });

    let result = tool.execute(args.as_object().unwrap().clone(), &context).await;
    
    match result {
        Ok(call_result) => {
            assert_eq!(call_result.is_error, Some(false));
            
            let response_text = match &call_result.content[0].resource {
                swissarmyhammer_tools::rmcp::model::RawContent::Text(text_content) => &text_content.text,
                _ => panic!("Expected text content"),
            };
            
            // Verify that we received content (redirect handling is done internally by markdowndown)
            assert!(!response_text.is_empty(), "Should have received content after redirect handling");
            
            println!("✅ Content validation use case passed");
        }
        Err(e) => {
            if e.to_string().contains("Connection") || e.to_string().contains("timeout") {
                println!("⚠️  Network error acceptable in CI: {}", e);
            } else {
                panic!("Unexpected error in content validation test: {}", e);
            }
        }
    }
}

/// Test case 4: News and Content Analysis from the specification
/// URL: "https://blog.rust-lang.org/2024/01/15/recent-updates.html"
#[tokio::test]
async fn test_specification_use_case_news_analysis() {
    let tool = WebFetchTool::new();
    let context = create_test_context();

    let args = json!({
        "url": "https://blog.rust-lang.org/",
        "max_content_length": 5242880 // 5MB as specified
    });

    let result = tool.execute(args.as_object().unwrap().clone(), &context).await;
    
    match result {
        Ok(call_result) => {
            assert_eq!(call_result.is_error, Some(false));
            
            let response_text = match &call_result.content[0].resource {
                swissarmyhammer_tools::rmcp::model::RawContent::Text(text_content) => &text_content.text,
                _ => panic!("Expected text content"),
            };
            
            // Verify blog content characteristics
            assert!(
                response_text.to_lowercase().contains("rust") ||
                response_text.to_lowercase().contains("blog"),
                "Should contain Rust blog content"
            );
            
            println!("✅ News and content analysis use case passed");
        }
        Err(e) => {
            if e.to_string().contains("Connection") || e.to_string().contains("timeout") {
                println!("⚠️  Network error acceptable in CI: {}", e);
            } else {
                panic!("Unexpected error in news analysis test: {}", e);
            }
        }
    }
}

/// Test case 5: GitHub Issue Processing
/// URL: GitHub issue for testing structured content extraction
#[tokio::test]
async fn test_specification_use_case_github_issue() {
    let tool = WebFetchTool::new();
    let context = create_test_context();

    let args = json!({
        "url": "https://github.com/rust-lang/rust/issues/1",
        "timeout": 30,
        "user_agent": "SwissArmyHammer-IssueTracker/1.0"
    });

    let result = tool.execute(args.as_object().unwrap().clone(), &context).await;
    
    match result {
        Ok(call_result) => {
            assert_eq!(call_result.is_error, Some(false));
            
            let response_text = match &call_result.content[0].resource {
                swissarmyhammer_tools::rmcp::model::RawContent::Text(text_content) => &text_content.text,
                _ => panic!("Expected text content"),
            };
            
            // Verify GitHub-specific content characteristics
            assert!(
                response_text.to_lowercase().contains("issue") || 
                response_text.to_lowercase().contains("github") ||
                response_text.to_lowercase().contains("rust"),
                "Should contain GitHub issue-related content"
            );
            
            // Verify structured content extraction
            assert!(!response_text.is_empty(), "Should have extracted text content");
            
            println!("✅ GitHub issue processing use case passed");
        }
        Err(e) => {
            if e.to_string().contains("Connection") || e.to_string().contains("timeout") {
                println!("⚠️  Network error acceptable in CI: {}", e);
            } else {
                panic!("Unexpected error in GitHub issue test: {}", e);
            }
        }
    }
}

/// Test response format compliance with specification
#[tokio::test]
async fn test_response_format_specification_compliance() {
    let tool = WebFetchTool::new();
    let context = create_test_context();

    // Use a reliable test endpoint
    let args = json!({
        "url": "https://httpbin.org/html",
        "timeout": 30
    });

    let result = tool.execute(args.as_object().unwrap().clone(), &context).await;
    
    match result {
        Ok(call_result) => {
            // Verify basic structure
            assert!(!call_result.content.is_empty());
            assert_eq!(call_result.is_error, Some(false));
            
            let response_text = match &call_result.content[0].resource {
                swissarmyhammer_tools::rmcp::model::RawContent::Text(text_content) => &text_content.text,
                _ => panic!("Expected text content"),
            };
            
            // Verify simple response format - should contain the actual fetched content
            assert!(!response_text.is_empty(), "Should contain fetched content");
            assert!(response_text.contains("html") || response_text.contains("HTML"), "Should contain HTML content converted to markdown");
            
            println!("✅ Response format specification compliance passed");
        }
        Err(e) => {
            if e.to_string().contains("Connection") || e.to_string().contains("timeout") {
                println!("⚠️  Network error acceptable in CI: {}", e);
            } else {
                panic!("Unexpected error in response format test: {}", e);
            }
        }
    }
}

/// Test error response format compliance with specification
#[tokio::test]
async fn test_error_response_specification_compliance() {
    let tool = WebFetchTool::new();
    let context = create_test_context();

    // Test with invalid URL to trigger error
    let args = json!({
        "url": "https://invalid-domain-that-does-not-exist-12345.com"
    });

    let result = tool.execute(args.as_object().unwrap().clone(), &context).await;
    
    match result {
        Ok(call_result) => {
            // Should be an error response
            assert_eq!(call_result.is_error, Some(true));
            
            let response_text = match &call_result.content[0].resource {
                swissarmyhammer_tools::rmcp::model::RawContent::Text(text_content) => &text_content.text,
                _ => panic!("Expected text content"),
            };
            
            // For error responses, we still return detailed error information
            let response: serde_json::Value = serde_json::from_str(response_text)
                .expect("Response should be valid JSON");
            
            // Verify error response structure is maintained for debugging
            assert!(response["content"].is_array(), "content should be array");
            let content_item = &response["content"][0];
            assert!(content_item["text"].as_str().unwrap().starts_with("Failed to fetch content:"));
            
            println!("✅ Error response specification compliance passed");
        }
        Err(_) => {
            // If the tool throws an MCP error instead of returning error response,
            // that's also acceptable behavior
            println!("✅ Tool properly rejected invalid request with MCP error");
        }
    }
}

/// Test redirect response format compliance
#[tokio::test]
async fn test_redirect_response_specification_compliance() {
    let tool = WebFetchTool::new();
    let context = create_test_context();

    // Test with a URL that redirects
    let args = json!({
        "url": "https://httpbin.org/redirect/1",
        "follow_redirects": true
    });

    let result = tool.execute(args.as_object().unwrap().clone(), &context).await;
    
    match result {
        Ok(call_result) => {
            assert_eq!(call_result.is_error, Some(false));
            
            let response_text = match &call_result.content[0].resource {
                swissarmyhammer_tools::rmcp::model::RawContent::Text(text_content) => &text_content.text,
                _ => panic!("Expected text content"),
            };
            
            // For redirects, we should still get content (markdowndown handles redirects internally)
            assert!(!response_text.is_empty(), "Should have content after redirect handling");
            
            println!("✅ Redirect response specification compliance passed");
        }
        Err(e) => {
            if e.to_string().contains("Connection") || e.to_string().contains("timeout") {
                println!("⚠️  Network error acceptable in CI: {}", e);
            } else {
                panic!("Unexpected error in redirect test: {}", e);
            }
        }
    }
}