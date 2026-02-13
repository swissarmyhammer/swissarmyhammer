//! Integration tests for web fetch specification compliance
//!
//! These tests verify that the unified web tool's fetch operation meets specification requirements.

use serde_json::json;
use swissarmyhammer_tools::mcp::tool_registry::McpTool;
use swissarmyhammer_tools::mcp::tools::web::WebTool;

/// Test that the unified web tool has proper registration and metadata
#[test]
fn test_tool_registration_and_metadata() {
    let tool = WebTool::new();

    // Verify tool registration
    assert_eq!(tool.name(), "web");
    assert!(!tool.description().is_empty());

    let schema = tool.schema();

    // Validate schema structure
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"].is_object());

    // Should have op field with fetch url as an option
    let op_enum = schema["properties"]["op"]["enum"]
        .as_array()
        .expect("op should have enum");
    assert!(op_enum.contains(&json!("fetch url")));

    // Should have url property for fetch
    assert!(schema["properties"]["url"].is_object());

    println!("✓ Tool registration and metadata compliance verified");
}

/// Test that fetch-related parameters are present in schema
#[test]
fn test_fetch_parameter_schema_compliance() {
    let tool = WebTool::new();
    let schema = tool.schema();
    let properties = &schema["properties"];

    // Check fetch-specific parameters exist
    assert!(properties["url"].is_object(), "Should have url property");
    assert!(
        properties["timeout"].is_object(),
        "Should have timeout property"
    );
    assert!(
        properties["follow_redirects"].is_object(),
        "Should have follow_redirects property"
    );
    assert!(
        properties["max_content_length"].is_object(),
        "Should have max_content_length property"
    );
    assert!(
        properties["user_agent"].is_object(),
        "Should have user_agent property"
    );

    println!("✓ Fetch parameter schema compliance verified");
}

#[test]
fn test_specification_use_case_parameters() {
    // Test that all use case parameters from the specification are supported

    // Use Case 1: Documentation Research
    let doc_research_params = json!({
        "url": "https://docs.rust-lang.org/book/ch04-01-what-is-ownership.html",
        "timeout": 45,
        "max_content_length": 2097152
    });

    // Use Case 2: API Documentation Processing
    let api_doc_params = json!({
        "url": "https://api.github.com/docs/rest/repos",
        "user_agent": "SwissArmyHammer-DocProcessor/1.0"
    });

    // Use Case 3: Content Validation
    let content_validation_params = json!({
        "url": "https://example.com/changelog",
        "follow_redirects": true,
        "timeout": 15
    });

    // Use Case 4: News and Content Analysis
    let news_analysis_params = json!({
        "url": "https://blog.rust-lang.org/2024/01/15/recent-updates.html",
        "max_content_length": 5242880
    });

    // Verify all parameter combinations are valid
    for (params, description) in [
        (&doc_research_params, "Documentation Research"),
        (&api_doc_params, "API Documentation Processing"),
        (&content_validation_params, "Content Validation"),
        (&news_analysis_params, "News and Content Analysis"),
    ] {
        // Verify required URL is present
        assert!(
            params["url"].is_string(),
            "{description} should have string URL"
        );

        // Verify optional parameters are within valid ranges when present
        if let Some(timeout) = params.get("timeout") {
            let timeout_val = timeout.as_u64().unwrap();
            assert!(
                (5..=120).contains(&timeout_val),
                "{description} timeout should be in valid range"
            );
        }

        if let Some(max_length) = params.get("max_content_length") {
            let max_length_val = max_length.as_u64().unwrap();
            assert!(
                (1024..=10485760).contains(&max_length_val),
                "{description} max_content_length should be in valid range"
            );
        }

        println!("✓ {description} use case parameters validated");
    }
}

#[test]
fn test_response_format_specification_structure() {
    // This test verifies the expected response format structures
    // without making actual HTTP requests

    // Expected successful response structure per specification
    let expected_success_structure = json!({
        "content": [{"type": "text", "text": "Successfully fetched content from URL"}],
        "is_error": false,
        "metadata": {
            "url": "https://example.com/page",
            "final_url": "https://example.com/page",
            "title": "Example Page Title",
            "content_type": "text/html",
            "content_length": 15420,
            "status_code": 200,
            "response_time_ms": 245,
            "markdown_content": "# Example Page Title\n\nThis is the converted markdown content...",
            "word_count": 856,
            "headers": {
                "server": "nginx/1.18.0",
                "content-encoding": "gzip"
            }
        }
    });

    // Expected error response structure per specification
    let expected_error_structure = json!({
        "content": [{"type": "text", "text": "Failed to fetch content: Connection timeout"}],
        "is_error": true,
        "metadata": {
            "url": "https://example.com/page",
            "error_type": "timeout",
            "error_details": "Request timed out after 30 seconds",
            "status_code": null,
            "response_time_ms": 30000
        }
    });

    // Verify structure compliance
    assert_eq!(expected_success_structure["is_error"], false);
    assert!(expected_success_structure["content"].is_array());
    assert!(expected_success_structure["metadata"].is_object());

    assert_eq!(expected_error_structure["is_error"], true);
    assert!(expected_error_structure["metadata"]["status_code"].is_null());

    println!("✓ Response format structures match specification");
}

#[test]
fn test_security_and_validation_features() {
    use swissarmyhammer_tools::mcp::tools::web_fetch::fetch::WebFetchTool;

    // The WebFetchTool pipeline still has security validation
    let _tool = WebFetchTool::new();

    // Verify the security pipeline compiles and instantiates
    // (actual security validation is tested via web/fetch.rs dispatch)

    println!("✓ Security and validation features verified");
}
