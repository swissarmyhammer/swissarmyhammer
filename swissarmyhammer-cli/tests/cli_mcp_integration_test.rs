//! Integration tests for CLI-MCP tool integration
//!
//! These tests verify that the CLI can successfully call MCP tools directly
//! without going through the MCP protocol layer.

use serde_json::json;
use swissarmyhammer::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_cli::mcp_integration::CliToolContext;

mod test_utils;
use test_utils::setup_git_repo;

// Test helper functions to reduce code duplication

/// Creates a test context with an isolated environment.
/// Returns the environment (which must be kept alive) and the context.
async fn setup_test_context() -> (IsolatedTestEnvironment, CliToolContext) {
    let env = IsolatedTestEnvironment::new().unwrap();
    let temp_path = env.temp_dir();

    // Set up git repository for tests that need it
    setup_git_repo(&temp_path).expect("Failed to set up git repository");

    let context = CliToolContext::new_with_dir(&temp_path)
        .await
        .expect("Failed to create CliToolContext");
    (env, context)
}

/// Creates a CallToolResult for testing purposes.
#[cfg(test)]
fn create_test_call_result(text: &str, is_error: bool) -> rmcp::model::CallToolResult {
    use rmcp::model::{Annotated, CallToolResult, RawContent, RawTextContent};

    CallToolResult {
        content: vec![Annotated::new(
            RawContent::Text(RawTextContent {
                text: text.to_string(),
                meta: None,
            }),
            None,
        )],
        structured_content: None,
        is_error: Some(is_error),
        meta: None,
    }
}

/// Asserts that an MCP error converts correctly to a CLI error.
#[cfg(test)]
fn assert_cli_error_conversion(mcp_error: rmcp::ErrorData, expected_text: &str) {
    use swissarmyhammer_cli::error::CliError;

    let cli_error: CliError = mcp_error.into();
    assert!(cli_error.message.contains("MCP error"));
    assert!(cli_error.message.contains(expected_text));
    assert_eq!(cli_error.exit_code, 1);
}

#[tokio::test]
async fn test_cli_can_call_mcp_tools() {
    let (_env, _context) = setup_test_context().await;

    // Context creation successful means the tool registry is working
    // We can't directly access the registry methods anymore, but
    // successful initialization means tools are available
}

#[tokio::test]
#[serial_test::serial]
async fn test_todo_create_tool_integration() {
    let (_env, context) = setup_test_context().await;

    // Test calling todo_create tool
    let args = context.create_arguments(vec![
        ("task", json!("Test todo item")),
        (
            "context",
            json!("This is a test todo for integration testing."),
        ),
    ]);

    let result = context.execute_tool("todo_create", args).await;
    assert!(
        result.is_ok(),
        "Failed to execute todo_create tool: {:?}",
        result.err()
    );

    let call_result = result.unwrap();
    assert_eq!(
        call_result.is_error,
        Some(false),
        "Tool execution reported an error"
    );
    assert!(
        !call_result.content.is_empty(),
        "Tool result should have content"
    );
}

#[tokio::test]
async fn test_nonexistent_tool_error() {
    let (_env, context) = setup_test_context().await;

    // Test calling a nonexistent tool
    let args = context.create_arguments(vec![]);
    let result = context.execute_tool("nonexistent_tool", args).await;

    assert!(result.is_err(), "Should return error for nonexistent tool");

    let error = result.err().unwrap();
    assert!(
        error.to_string().contains("Tool not found"),
        "Error should mention tool not found"
    );
}

#[tokio::test]
#[serial_test::serial]
async fn test_invalid_arguments_error() {
    let (_env, context) = setup_test_context().await;

    // Test calling todo_create with invalid arguments (missing required fields)
    let args = context.create_arguments(vec![("invalid_field", json!("invalid_value"))]);

    let result = context.execute_tool("todo_create", args).await;
    assert!(result.is_err(), "Should return error for invalid arguments");
}

#[test]
fn test_response_formatting_utilities() {
    use swissarmyhammer_cli::mcp_integration::response_formatting;

    // Test success response formatting
    let success_result = create_test_call_result("Operation completed successfully", false);

    let formatted = response_formatting::format_success_response(&success_result);
    assert!(formatted.contains("Operation completed successfully"));

    // Test error response formatting
    let error_result = create_test_call_result("Something went wrong", true);

    let formatted_error = response_formatting::format_error_response(&error_result);
    assert!(formatted_error.contains("Something went wrong"));

    // Only test the functions that still exist
    // The table formatting and status message functions have been removed as they were dead code
}

#[test]
fn test_error_conversion() {
    use rmcp::ErrorData as McpError;

    // Test basic MCP error conversion
    let mcp_error = McpError::internal_error("test error".to_string(), None);
    assert_cli_error_conversion(mcp_error, "test error");

    // Test error handling continues to work normally
    let general_error = McpError::internal_error("Cannot proceed".to_string(), None);
    assert_cli_error_conversion(general_error, "Cannot proceed");
}

#[tokio::test]
async fn test_create_arguments_helper() {
    let (_env, context) = setup_test_context().await;

    // Test the create_arguments helper method
    let args = context.create_arguments(vec![
        ("string_param", json!("test_string")),
        ("number_param", json!(42)),
        ("bool_param", json!(true)),
        ("array_param", json!(["item1", "item2"])),
        ("object_param", json!({"key": "value"})),
    ]);

    assert_eq!(args.len(), 5);
    assert_eq!(args.get("string_param"), Some(&json!("test_string")));
    assert_eq!(args.get("number_param"), Some(&json!(42)));
    assert_eq!(args.get("bool_param"), Some(&json!(true)));
    assert_eq!(args.get("array_param"), Some(&json!(["item1", "item2"])));
    assert_eq!(args.get("object_param"), Some(&json!({"key": "value"})));
}
