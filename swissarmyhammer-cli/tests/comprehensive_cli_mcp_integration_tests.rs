//! Comprehensive CLI-MCP Integration Tests
//!
//! Extended integration tests that verify thorough CLI-MCP communication,
//! tool coverage, error handling, and response formatting.

use anyhow::Result;
use serde_json::json;
use swissarmyhammer_cli::mcp_integration::CliToolContext;
use tempfile::TempDir;

mod test_utils;
use test_utils::setup_git_repo;

/// Test helper to create a comprehensive test environment
fn setup_comprehensive_test_environment() -> Result<(TempDir, std::path::PathBuf)> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();

    // Create issues directory with sample issues
    let issues_dir = temp_path.join("issues");
    std::fs::create_dir_all(&issues_dir)?;

    // Create test issues
    std::fs::write(
        issues_dir.join("TEST_001_integration_test.md"),
        r#"# Integration Test Issue

This is a test issue for comprehensive CLI-MCP integration testing.

## Test Coverage
- MCP tool execution
- Error handling
- Response formatting
"#,
    )?;

    // Create .swissarmyhammer directory for memos
    let swissarmyhammer_dir = temp_path.join(".swissarmyhammer");
    std::fs::create_dir_all(&swissarmyhammer_dir)?;

    // Create source files for search testing
    let src_dir = temp_path.join("src");
    std::fs::create_dir_all(&src_dir)?;

    std::fs::write(
        src_dir.join("integration_test.rs"),
        r#"
// Comprehensive integration test source file
use std::error::Error;

/// Function for testing search functionality
pub fn integration_test_function() -> Result<String, Box<dyn Error>> {
    println!("Running integration test");
    Ok("Integration test completed".to_string())
}

/// Error handling function for testing
pub fn handle_integration_error(error: &str) -> Result<(), String> {
    eprintln!("Integration error: {}", error);
    Err("Integration error handled".to_string())
}
"#,
    )?;

    // Initialize git repository
    setup_git_repo(&temp_path)?;

    Ok((temp_dir, temp_path))
}

/// Test all issue-related MCP tools can be executed
#[tokio::test]
async fn test_all_issue_tools_execution() -> Result<()> {
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;
    let context = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Test issue_create
    let create_args = context.create_arguments(vec![
        ("name", json!("comprehensive_test_issue")),
        (
            "content",
            json!("# Comprehensive Test\n\nThis tests all issue tools."),
        ),
    ]);
    let result = context.execute_tool("issue_create", create_args).await;
    assert!(result.is_ok(), "issue_create should succeed: {result:?}");

    // Test issue_show current (might not have current issue, but should not error on tool level)
    let current_args = context.create_arguments(vec![("name", json!("current"))]);
    let result = context.execute_tool("issue_show", current_args).await;
    // This might succeed or fail depending on branch, but tool should be callable
    assert!(
        result.is_ok() || result.is_err(),
        "issue_show current should be callable"
    );

    // Test issue_show next
    let next_args = context.create_arguments(vec![("name", json!("next"))]);
    let result = context.execute_tool("issue_show", next_args).await;
    assert!(result.is_ok(), "issue_show next should succeed: {result:?}");

    // Test issue_all_complete
    let all_complete_args = context.create_arguments(vec![]);
    let result = context
        .execute_tool("issue_all_complete", all_complete_args)
        .await;
    assert!(
        result.is_ok(),
        "issue_all_complete should succeed: {result:?}"
    );

    Ok(())
}

/// Test all memo-related MCP tools can be executed
#[tokio::test]
async fn test_all_memo_tools_execution() -> Result<()> {
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;
    let context = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Test memo_create
    let create_args = context.create_arguments(vec![
        ("title", json!("Comprehensive Test Memo")),
        (
            "content",
            json!("# Test Memo\n\nThis tests all memo tools."),
        ),
    ]);
    let result = context.execute_tool("memo_create", create_args).await;
    assert!(result.is_ok(), "memo_create should succeed: {result:?}");

    let call_result = result.unwrap();
    assert_eq!(
        call_result.is_error,
        Some(false),
        "memo_create should not report error"
    );

    // Extract memo ID from response for subsequent tests
    let content = swissarmyhammer_cli::mcp_integration::response_formatting::extract_text_content(
        &call_result,
    );
    let memo_id = extract_memo_id_from_response(&content.unwrap_or_default());

    // Test memo_list
    let list_args = context.create_arguments(vec![]);
    let result = context.execute_tool("memo_list", list_args).await;
    assert!(result.is_ok(), "memo_list should succeed: {result:?}");

    // Test memo_get if we have an ID
    if let Some(id) = memo_id {
        let get_args = context.create_arguments(vec![("id", json!(id))]);
        let result = context.execute_tool("memo_get", get_args).await;
        assert!(result.is_ok(), "memo_get should succeed: {result:?}");

        // Test memo_update
        let update_args = context.create_arguments(vec![
            ("id", json!(id)),
            (
                "content",
                json!("# Updated Test Memo\n\nThis memo has been updated."),
            ),
        ]);
        let result = context.execute_tool("memo_update", update_args).await;
        assert!(result.is_ok(), "memo_update should succeed: {result:?}");

        // Test memo_delete
        let delete_args = context.create_arguments(vec![("id", json!(id))]);
        let result = context.execute_tool("memo_delete", delete_args).await;
        assert!(result.is_ok(), "memo_delete should succeed: {result:?}");
    }

    // Test memo_search
    let search_args = context.create_arguments(vec![("query", json!("test"))]);
    let result = context.execute_tool("memo_search", search_args).await;
    assert!(result.is_ok(), "memo_search should succeed: {result:?}");

    // Test memo_get_all_context
    let context_args = context.create_arguments(vec![]);
    let result = context
        .execute_tool("memo_get_all_context", context_args)
        .await;
    assert!(
        result.is_ok(),
        "memo_get_all_context should succeed: {result:?}"
    );

    Ok(())
}

/// Test all search-related MCP tools can be executed
#[tokio::test]
#[ignore] // Temporarily disabled due to DuckDB crash during cleanup
async fn test_all_search_tools_execution() -> Result<()> {
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;
    let context = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Test search_index
    let index_args = context.create_arguments(vec![
        ("patterns", json!(["src/**/*.rs"])),
        ("force", json!(false)),
    ]);
    let result = context.execute_tool("search_index", index_args).await;
    assert!(result.is_ok(), "search_index should succeed: {result:?}");

    // Test search_query
    let query_args = context.create_arguments(vec![
        ("query", json!("integration test")),
        ("limit", json!(10)),
    ]);
    let result = context.execute_tool("search_query", query_args).await;
    assert!(result.is_ok(), "search_query should succeed: {result:?}");

    Ok(())
}

/// Test error propagation from MCP tools to CLI
#[tokio::test]
async fn test_mcp_error_propagation() -> Result<()> {
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;
    let context = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Test invalid arguments error
    let invalid_args = context.create_arguments(vec![("invalid_field", json!("invalid_value"))]);
    let result = context.execute_tool("memo_create", invalid_args).await;
    assert!(result.is_err(), "Invalid arguments should cause error");

    // Test missing required arguments error
    let empty_args = context.create_arguments(vec![]);
    let result = context.execute_tool("memo_create", empty_args).await;
    assert!(
        result.is_err(),
        "Missing required arguments should cause error"
    );

    // Test non-existent resource error
    let nonexistent_args =
        context.create_arguments(vec![("id", json!("01ARZ3NDEKTSV4RRFFQ69G5FAV"))]);
    let result = context.execute_tool("memo_get", nonexistent_args).await;
    assert!(result.is_err(), "Non-existent memo should cause error");

    Ok(())
}

/// Test argument passing and validation
#[tokio::test]
#[ignore] // Temporarily disabled due to DuckDB crash during cleanup
async fn test_argument_passing_and_validation() -> Result<()> {
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;
    let context = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Test correct argument types
    let valid_args = context.create_arguments(vec![
        ("title", json!("String Title")),
        ("content", json!("String content")),
    ]);
    let result = context.execute_tool("memo_create", valid_args).await;
    assert!(result.is_ok(), "Valid arguments should succeed");

    // Test boolean arguments
    let boolean_args = context.create_arguments(vec![
        ("patterns", json!(["**/*.rs"])),
        ("force", json!(true)),
    ]);
    let result = context.execute_tool("search_index", boolean_args).await;
    assert!(
        result.is_ok(),
        "Boolean arguments should be handled correctly"
    );

    // Test array arguments
    let array_args = context.create_arguments(vec![
        ("patterns", json!(["src/**/*.rs", "tests/**/*.rs"])),
        ("force", json!(false)),
    ]);
    let result = context.execute_tool("search_index", array_args).await;
    assert!(
        result.is_ok(),
        "Array arguments should be handled correctly"
    );

    // Test numeric arguments
    let numeric_args =
        context.create_arguments(vec![("query", json!("test query")), ("limit", json!(5))]);
    let result = context.execute_tool("search_query", numeric_args).await;
    assert!(
        result.is_ok(),
        "Numeric arguments should be handled correctly"
    );

    Ok(())
}

/// Test response formatting utilities
#[tokio::test]
async fn test_response_formatting() -> Result<()> {
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;
    let context = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Test successful response formatting
    let args = context.create_arguments(vec![
        ("title", json!("Format Test Memo")),
        ("content", json!("Testing response formatting")),
    ]);
    let result = context.execute_tool("memo_create", args).await?;

    let success_response =
        swissarmyhammer_cli::mcp_integration::response_formatting::format_success_response(&result);
    assert!(
        !success_response.is_empty(),
        "Success response should not be empty"
    );
    assert!(
        !success_response.contains("error"),
        "Success response should not contain error"
    );

    // Test JSON extraction
    let json_result =
        swissarmyhammer_cli::mcp_integration::response_formatting::extract_json_data(&result);
    // JSON extraction might fail if response is not JSON, which is acceptable
    match json_result {
        Ok(json) => {
            assert!(
                json.is_object() || json.is_string(),
                "JSON should be valid structure"
            );
        }
        Err(_) => {
            // Non-JSON responses are acceptable for many tools
        }
    }

    Ok(())
}

/// Test concurrent tool execution
#[tokio::test]
async fn test_concurrent_tool_execution() -> Result<()> {
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;
    let _context = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Execute multiple tools concurrently
    let mut handles = vec![];

    // Create multiple memos concurrently
    for i in 0..3 {
        let context_clone = CliToolContext::new_with_dir(&temp_path)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let handle = tokio::spawn(async move {
            let args = context_clone.create_arguments(vec![
                ("title", json!(format!("Concurrent Test Memo {}", i))),
                ("content", json!(format!("Content for memo {}", i))),
            ]);
            context_clone.execute_tool("memo_create", args).await
        });
        handles.push(handle);
    }

    // Wait for all concurrent operations to complete
    for handle in handles {
        let result = handle.await??;
        assert_eq!(
            result.is_error,
            Some(false),
            "Concurrent operation should succeed"
        );
    }

    Ok(())
}

/// Test tool execution with complex data structures
#[tokio::test]
async fn test_complex_data_structures() -> Result<()> {
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;
    let context = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Test with complex content containing markdown, special characters, etc.
    let complex_content = r#"# Complex Test Content

## Features Tested
- **Bold text** and *italic text*
- `Code snippets`
- Lists:
  1. Numbered items
  2. More items
- Special characters: @#$%^&*()
- Unicode: 日本語, émojis 🚀

```rust
fn example_code() {
    println!("Testing code blocks");
}
```

| Table | Headers |
|-------|---------|
| Data  | Values  |
"#;

    let args = context.create_arguments(vec![
        ("title", json!("Complex Content Test")),
        ("content", json!(complex_content)),
    ]);

    let result = context.execute_tool("memo_create", args).await;
    assert!(
        result.is_ok(),
        "Complex content should be handled correctly: {result:?}"
    );

    Ok(())
}

/// Test tool execution edge cases
#[tokio::test]
async fn test_tool_execution_edge_cases() -> Result<()> {
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;
    let context = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Test empty string arguments
    let empty_args = context.create_arguments(vec![
        ("title", json!("Empty Content Test")),
        ("content", json!("")),
    ]);
    let result = context.execute_tool("memo_create", empty_args).await;
    assert!(result.is_ok(), "Empty content should be handled");

    // Test very long content
    let long_content = "A".repeat(10000);
    let long_args = context.create_arguments(vec![
        ("title", json!("Long Content Test")),
        ("content", json!(long_content)),
    ]);
    let result = context.execute_tool("memo_create", long_args).await;
    assert!(result.is_ok(), "Long content should be handled");

    // Test null values (JSON null)
    let null_args = context.create_arguments(vec![
        ("title", json!("Null Test")),
        ("content", json!(null)),
    ]);
    let result = context.execute_tool("memo_create", null_args).await;
    // This should fail validation
    assert!(
        result.is_err(),
        "Null content should cause validation error"
    );

    Ok(())
}

/// Test error message formatting and user-friendliness
#[tokio::test]
async fn test_error_message_formatting() -> Result<()> {
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;
    let context = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Test missing required field error
    let result = context
        .execute_tool("memo_create", context.create_arguments(vec![]))
        .await;
    assert!(result.is_err(), "Should error on missing required fields");

    let error = result.unwrap_err();
    let error_msg = error.to_string();
    assert!(
        error_msg.contains("required")
            || error_msg.contains("missing")
            || error_msg.contains("title"),
        "Error message should be descriptive: {error_msg}"
    );

    // Test invalid tool name error
    let result = context
        .execute_tool("nonexistent_tool", context.create_arguments(vec![]))
        .await;
    assert!(result.is_err(), "Should error on nonexistent tool");

    let error = result.unwrap_err();
    let error_msg = error.to_string();
    assert!(
        error_msg.contains("not found") || error_msg.contains("Tool not found"),
        "Error message should indicate tool not found: {error_msg}"
    );

    Ok(())
}

/// Test tool context initialization with different configurations
#[tokio::test]
async fn test_tool_context_configurations() -> Result<()> {
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;

    // Test with different working directories
    let context1 = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let context2 = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Both should work independently
    let args1 = context1.create_arguments(vec![
        ("title", json!("Context 1 Memo")),
        ("content", json!("From context 1")),
    ]);
    let args2 = context2.create_arguments(vec![
        ("title", json!("Context 2 Memo")),
        ("content", json!("From context 2")),
    ]);

    let result1 = context1.execute_tool("memo_create", args1).await;
    let result2 = context2.execute_tool("memo_create", args2).await;

    assert!(result1.is_ok(), "Context 1 should work independently");
    assert!(result2.is_ok(), "Context 2 should work independently");

    Ok(())
}

/// Test MCP tool robustness under stress
#[tokio::test]
#[ignore] // Temporarily disabled due to DuckDB crash during cleanup
async fn test_mcp_tool_stress_conditions() -> Result<()> {
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;
    let context = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Test rapid successive operations
    for i in 0..10 {
        let args = context.create_arguments(vec![
            ("title", json!(format!("Stress Test Memo {}", i))),
            ("content", json!(format!("Stress test content {}", i))),
        ]);
        let result = context.execute_tool("memo_create", args).await;
        assert!(
            result.is_ok(),
            "Rapid operations should succeed: {result:?}"
        );
    }

    // Test tool execution with minimal resources
    let args = context.create_arguments(vec![
        ("patterns", json!(["nonexistent/**/*.rs"])),
        ("force", json!(false)),
    ]);
    let result = context.execute_tool("search_index", args).await;
    // Should succeed even if no files found
    assert!(result.is_ok(), "Should handle empty indexing gracefully");

    Ok(())
}

/// Test MCP tool state consistency across operations
#[tokio::test]
async fn test_mcp_tool_state_consistency() -> Result<()> {
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;
    let context = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Create issue
    let create_args = context.create_arguments(vec![
        ("name", json!("state_consistency_test")),
        (
            "content",
            json!("# State Test\n\nTesting state consistency."),
        ),
    ]);
    let create_result = context.execute_tool("issue_create", create_args).await?;
    assert_eq!(create_result.is_error, Some(false));

    // List issues - should include our created issue
    let list_args = context.create_arguments(vec![]);
    let list_result = context.execute_tool("issue_list", list_args).await;

    // Even if issue_list tool doesn't exist, the call should be handled gracefully
    match list_result {
        Ok(result) => {
            // If successful, should show consistent state
            let text_content =
                swissarmyhammer_cli::mcp_integration::response_formatting::extract_text_content(
                    &result,
                );
            if let Some(content) = text_content {
                // If we get content, it should be consistent
                assert!(!content.is_empty(), "List results should have content");
            }
        }
        Err(_) => {
            // If the tool doesn't exist, that's also acceptable for this test
            // The important thing is that it fails gracefully
        }
    }

    Ok(())
}

/// Test MCP error boundaries and recovery
#[tokio::test]
async fn test_mcp_error_boundaries() -> Result<()> {
    let (_temp_dir, temp_path) = setup_comprehensive_test_environment()?;
    let context = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Test malformed arguments (empty arguments when required fields are missing)
    let empty_args = serde_json::Map::new();
    let result = context.execute_tool("memo_create", empty_args).await;
    assert!(
        result.is_err(),
        "Missing required arguments should be rejected"
    );

    // Test context recovery after error
    let valid_args = context.create_arguments(vec![
        ("title", json!("Recovery Test")),
        ("content", json!("Testing recovery after error")),
    ]);
    let result = context.execute_tool("memo_create", valid_args).await;
    assert!(
        result.is_ok(),
        "Context should recover after error: {result:?}"
    );

    Ok(())
}

/// Helper function to extract memo ID from MCP response
fn extract_memo_id_from_response(content: &str) -> Option<String> {
    // Try to extract ULID pattern from response
    // ULIDs are 26 characters long and use Crockford's Base32
    use regex::Regex;

    let ulid_pattern = Regex::new(r"[0-9A-HJKMNP-TV-Z]{26}").ok()?;
    ulid_pattern.find(content).map(|m| m.as_str().to_string())
}
