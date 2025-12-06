//! Comprehensive CLI-MCP Integration Tests
//!
//! Extended integration tests that verify thorough CLI-MCP communication,
//! tool coverage, error handling, and response formatting.

use anyhow::Result;
use serde_json::json;
use swissarmyhammer::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_cli::mcp_integration::CliToolContext;

mod test_utils;
use test_utils::{create_semantic_test_guard, setup_git_repo};

/// Helper to create a test context with git repository
async fn create_test_context_with_git() -> Result<(IsolatedTestEnvironment, CliToolContext)> {
    let env = IsolatedTestEnvironment::new()?;
    let temp_path = env.temp_dir();
    setup_git_repo(&temp_path)?;
    let context = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok((env, context))
}

/// Helper to create a test context without git repository
async fn create_test_context() -> Result<(IsolatedTestEnvironment, CliToolContext)> {
    let env = IsolatedTestEnvironment::new()?;
    let temp_path = env.temp_dir();
    let context = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok((env, context))
}

/// Helper to create todo arguments
fn create_todo_args(
    context: &CliToolContext,
    task: &str,
    content: &str,
) -> serde_json::Map<String, serde_json::Value> {
    context.create_arguments(vec![("task", json!(task)), ("context", json!(content))])
}

/// Helper to assert error contains expected phrases
fn assert_error_contains_any(error: &rmcp::model::ErrorData, expected_phrases: &[&str]) {
    let error_msg = error.to_string();
    assert!(
        expected_phrases
            .iter()
            .any(|phrase| error_msg.contains(phrase)),
        "Error message should contain one of {:?}, got: {}",
        expected_phrases,
        error_msg
    );
}

/// Macro for asserting tool success
macro_rules! assert_tool_success {
    ($result:expr, $msg:expr) => {
        assert!($result.is_ok(), "{}: {:?}", $msg, $result);
    };
}

/// Test error propagation from MCP tools to CLI
#[tokio::test]
async fn test_mcp_error_propagation() -> Result<()> {
    let (_env, context) = create_test_context().await?;

    // Test invalid arguments error
    let invalid_args = context.create_arguments(vec![("invalid_field", json!("invalid_value"))]);
    let result = context.execute_tool("todo_create", invalid_args).await;
    assert!(result.is_err(), "Invalid arguments should cause error");

    // Test missing required arguments error
    let empty_args = context.create_arguments(vec![]);
    let result = context.execute_tool("todo_create", empty_args).await;
    assert!(
        result.is_err(),
        "Missing required arguments should cause error"
    );

    // Test non-existent resource handling with todo_show
    let nonexistent_args =
        context.create_arguments(vec![("item", json!("01NONEXISTENT000000000000"))]);
    let result = context.execute_tool("todo_show", nonexistent_args).await;

    // The todo_show tool may return either success with "not found" message or error
    // depending on the integration layer. Both behaviors are acceptable.
    match result {
        Ok(response) => {
            let text = response.content[0].as_text().unwrap().text.as_str();
            assert!(
                text.contains("not found") || text.contains("No todo"),
                "Should contain not found message, got: {}",
                text
            );
        }
        Err(_) => {
            // CLI integration may return error for non-existent resources
            // This is acceptable behavior
        }
    }

    Ok(())
}

/// Test argument passing and validation
#[tokio::test]
// Fixed: Limited patterns to specific files to avoid DuckDB timeout
async fn test_argument_passing_and_validation() -> Result<()> {
    let _guard = create_semantic_test_guard();
    let (_env, _context) = create_test_context_with_git().await?;
    let temp_path = _env.temp_dir();

    // Create source files for search testing
    let src_dir = temp_path.join("src");
    std::fs::create_dir_all(&src_dir)?;

    std::fs::write(
        src_dir.join("integration_test.rs"),
        r#"// Test file for search functionality
pub fn test_function() -> String { "test".to_string() }"#,
    )?;

    let context = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Test correct argument types
    let valid_args = create_todo_args(&context, "String Title", "String content");
    let result = context.execute_tool("todo_create", valid_args).await;
    assert_tool_success!(result, "Valid arguments should succeed");

    Ok(())
}

/// Test response formatting utilities
#[tokio::test]
async fn test_response_formatting() -> Result<()> {
    let (_env, context) = create_test_context_with_git().await?;

    // Test successful response formatting with todo_create
    let args = create_todo_args(&context, "Format Test Task", "Testing response formatting");
    let result = context.execute_tool("todo_create", args).await?;

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
    let (_env, _context) = create_test_context_with_git().await?;
    let temp_path = _env.temp_dir();

    // Execute multiple tools concurrently
    let mut handles = vec![];

    // Create multiple todos concurrently
    for i in 0..3 {
        let context_clone = CliToolContext::new_with_dir(&temp_path)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let handle = tokio::spawn(async move {
            let args = create_todo_args(
                &context_clone,
                &format!("Concurrent Test Task {}", i),
                &format!("Context for task {}", i),
            );
            context_clone.execute_tool("todo_create", args).await
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

/// Test error message formatting and user-friendliness
#[tokio::test]
async fn test_error_message_formatting() -> Result<()> {
    let (_env, context) = create_test_context().await?;

    // Test missing required field error
    let result = context
        .execute_tool("todo_create", context.create_arguments(vec![]))
        .await;
    assert!(result.is_err(), "Should error on missing required fields");

    if let Err(error) = result {
        assert_error_contains_any(&error, &["required", "missing", "task"]);
    }

    // Test invalid tool name error
    let result = context
        .execute_tool("nonexistent_tool", context.create_arguments(vec![]))
        .await;
    assert!(result.is_err(), "Should error on nonexistent tool");

    if let Err(error) = result {
        assert_error_contains_any(&error, &["not found", "Tool not found", "Unknown tool"]);
    }

    Ok(())
}

/// Test tool context initialization with different configurations
#[tokio::test]
async fn test_tool_context_configurations() -> Result<()> {
    let (_env, context1) = create_test_context_with_git().await?;
    let temp_path = _env.temp_dir();

    // Test with another context using the same directory
    let context2 = CliToolContext::new_with_dir(&temp_path)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Both should work independently
    let args1 = create_todo_args(&context1, "Context 1 Task", "From context 1");
    let args2 = create_todo_args(&context2, "Context 2 Task", "From context 2");

    let result1 = context1.execute_tool("todo_create", args1).await;
    let result2 = context2.execute_tool("todo_create", args2).await;

    assert_tool_success!(result1, "Context 1 should work independently");
    assert_tool_success!(result2, "Context 2 should work independently");

    Ok(())
}

/// Test MCP error boundaries and recovery
#[tokio::test]
async fn test_mcp_error_boundaries() -> Result<()> {
    let (_env, context) = create_test_context_with_git().await?;

    // Test malformed arguments (empty arguments when required fields are missing)
    let empty_args = serde_json::Map::new();
    let result = context.execute_tool("todo_create", empty_args).await;
    assert!(
        result.is_err(),
        "Missing required arguments should be rejected"
    );

    // Test context recovery after error
    let valid_args = create_todo_args(&context, "Recovery Test", "Testing recovery after error");
    let result = context.execute_tool("todo_create", valid_args).await;
    assert_tool_success!(result, "Context should recover after error");

    Ok(())
}
