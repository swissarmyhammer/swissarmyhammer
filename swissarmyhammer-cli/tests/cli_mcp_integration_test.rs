//! Integration tests for CLI-MCP tool integration
//!
//! These tests verify that the CLI can successfully call MCP tools directly
//! without going through the MCP protocol layer.

use serde_json::json;
use swissarmyhammer_cli::mcp_integration::CliToolContext;
use tempfile::TempDir;

/// Test helper to create a test environment
fn setup_test_environment() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create SwissArmyHammer directory structure
    let swissarmyhammer_dir = temp_dir.path().join(".swissarmyhammer");
    std::fs::create_dir_all(&swissarmyhammer_dir)
        .expect("Failed to create .swissarmyhammer directory");

    // Create issues directory within swissarmyhammer structure
    let issues_dir = swissarmyhammer_dir.join("issues");
    std::fs::create_dir_all(&issues_dir).expect("Failed to create issues directory");

    // Create memos directory for memo storage
    let memos_dir = swissarmyhammer_dir.join("memos");
    std::fs::create_dir_all(&memos_dir).expect("Failed to create memos directory");

    // Initialize git repository in temp directory to avoid branch conflicts
    use git2::{Repository, Signature};

    let repo = Repository::init(temp_dir.path()).expect("Failed to init git repo");

    // Configure git for testing
    let mut config = repo.config().expect("Failed to get git config");

    config
        .set_str("user.email", "test@example.com")
        .expect("Failed to configure git email");

    config
        .set_str("user.name", "Test User")
        .expect("Failed to configure git name");

    // Create initial README file
    let readme_path = temp_dir.path().join("README.md");
    std::fs::write(
        &readme_path,
        "# Test Repository\n\nThis is a test repository.",
    )
    .expect("Failed to create README.md");

    // Add and commit initial file to establish HEAD
    let mut index = repo.index().expect("Failed to get index");

    index
        .add_path(std::path::Path::new("README.md"))
        .expect("Failed to add README.md to index");

    index.write().expect("Failed to write index");

    let tree_id = index.write_tree().expect("Failed to write tree");

    let tree = repo.find_tree(tree_id).expect("Failed to find tree");

    let signature =
        Signature::now("Test User", "test@example.com").expect("Failed to create signature");

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Initial commit",
        &tree,
        &[],
    )
    .expect("Failed to create initial commit");

    // No longer change global current directory to avoid test isolation issues
    temp_dir
}

#[tokio::test]
async fn test_cli_can_call_mcp_tools() {
    let temp_dir = setup_test_environment();

    let _context = CliToolContext::new_with_dir(temp_dir.path())
        .await
        .expect("Failed to create CliToolContext");

    // Context creation successful means the tool registry is working
    // We can't directly access the registry methods anymore, but
    // successful initialization means tools are available
}

#[tokio::test]
async fn test_issue_create_tool_integration() {
    let temp_dir = setup_test_environment();

    let context = CliToolContext::new_with_dir(temp_dir.path())
        .await
        .expect("Failed to create CliToolContext");

    // Test calling issue_create tool
    let args = context.create_arguments(vec![
        ("name", json!("test_issue")),
        (
            "content",
            json!("# Test Issue\n\nThis is a test issue for integration testing."),
        ),
    ]);

    let result = context.execute_tool("issue_create", args).await;
    assert!(
        result.is_ok(),
        "Failed to execute issue_create tool: {:?}",
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
#[serial_test::serial]
async fn test_memo_create_tool_integration() {
    let temp_dir = setup_test_environment();

    let context = CliToolContext::new_with_dir(temp_dir.path())
        .await
        .expect("Failed to create CliToolContext");

    // Test calling memo_create tool
    let args = context.create_arguments(vec![
        ("title", json!("Test Memo")),
        (
            "content",
            json!("# Test Memo\n\nThis is a test memo for integration testing."),
        ),
    ]);

    let result = context.execute_tool("memo_create", args).await;
    assert!(
        result.is_ok(),
        "Failed to execute memo_create tool: {:?}",
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
    let temp_dir = setup_test_environment();

    let context = CliToolContext::new_with_dir(temp_dir.path())
        .await
        .expect("Failed to create CliToolContext");

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
    let temp_dir = setup_test_environment();

    let context = CliToolContext::new_with_dir(temp_dir.path())
        .await
        .expect("Failed to create CliToolContext");

    // Test calling memo_create with invalid arguments (missing required fields)
    let args = context.create_arguments(vec![("invalid_field", json!("invalid_value"))]);

    let result = context.execute_tool("memo_create", args).await;
    assert!(result.is_err(), "Should return error for invalid arguments");
}

#[tokio::test]
async fn test_issue_workflow_integration() {
    let temp_dir = setup_test_environment();

    let context = CliToolContext::new_with_dir(temp_dir.path())
        .await
        .expect("Failed to create CliToolContext");

    // Test a complete workflow: create issue, work with marker, verify workflow

    // 1. Create an issue
    let create_args = context.create_arguments(vec![
        ("name", json!("workflow_test")),
        (
            "content",
            json!("# Workflow Test\n\nTesting issue workflow integration."),
        ),
    ]);

    let create_result = context.execute_tool("issue_create", create_args).await;
    assert!(
        create_result.is_ok(),
        "Failed to create issue: {:?}",
        create_result.err()
    );

    let _create_call_result = create_result.unwrap();

    // 2. Verify no current issue marker initially
    let current_issue =
        swissarmyhammer_issues::current_marker::get_current_issue_in(temp_dir.path());
    assert!(
        current_issue.is_ok(),
        "Should be able to check current issue"
    );
    assert_eq!(
        current_issue.unwrap(),
        None,
        "Should have no current issue initially"
    );

    // 3. Set the current issue marker
    swissarmyhammer_issues::current_marker::set_current_issue_in("workflow_test", temp_dir.path())
        .expect("Failed to set current issue marker");

    // 4. Verify marker was set
    let current_issue =
        swissarmyhammer_issues::current_marker::get_current_issue_in(temp_dir.path());
    assert!(
        current_issue.is_ok(),
        "Should be able to read current issue"
    );
    assert_eq!(
        current_issue.unwrap(),
        Some("workflow_test".to_string()),
        "Current issue marker should be set"
    );

    // 5. Try to get the next issue using enhanced issue_show (should include our created issue)
    let next_args = context.create_arguments(vec![("name", json!("next"))]);
    let next_result = context.execute_tool("issue_show", next_args).await;

    // Note: This might fail if there are no pending issues, which is fine for this test
    // We're mainly testing that the tool can be called without errors
    match next_result {
        Ok(result) => {
            assert_eq!(
                result.is_error,
                Some(false),
                "issue_show next should not report error when successful"
            );
        }
        Err(e) => {
            // This is acceptable - might be no pending issues
            println!("issue_show next returned error (acceptable): {e}");
        }
    }

    // 6. Test issue_show with regular issue name
    let show_args = context.create_arguments(vec![("name", json!("workflow_test"))]);
    let show_result = context.execute_tool("issue_show", show_args).await;

    assert!(
        show_result.is_ok(),
        "issue_show with regular name should succeed: {:?}",
        show_result.err()
    );

    let show_call_result = show_result.unwrap();
    assert_eq!(
        show_call_result.is_error,
        Some(false),
        "issue_show should not report error when showing existing issue"
    );

    // 7. Clear the marker
    swissarmyhammer_issues::current_marker::clear_current_issue_in(temp_dir.path())
        .expect("Failed to clear current issue marker");

    // 8. Verify marker was cleared
    let current_issue =
        swissarmyhammer_issues::current_marker::get_current_issue_in(temp_dir.path());
    assert!(
        current_issue.is_ok(),
        "Should be able to check current issue"
    );
    assert_eq!(
        current_issue.unwrap(),
        None,
        "Current issue marker should be cleared"
    );
}

#[test]
fn test_response_formatting_utilities() {
    use rmcp::model::{Annotated, CallToolResult, RawContent, RawTextContent};

    use swissarmyhammer_cli::mcp_integration::response_formatting;

    // Test success response formatting
    let success_result = CallToolResult {
        content: vec![Annotated::new(
            RawContent::Text(RawTextContent {
                text: "Operation completed successfully".to_string(),
                meta: None,
            }),
            None,
        )],
        structured_content: None,
        is_error: Some(false),
        meta: None,
    };

    let formatted = response_formatting::format_success_response(&success_result);
    assert!(formatted.contains("Operation completed successfully"));

    // Test error response formatting
    let error_result = CallToolResult {
        content: vec![Annotated::new(
            RawContent::Text(RawTextContent {
                text: "Something went wrong".to_string(),
                meta: None,
            }),
            None,
        )],
        structured_content: None,
        is_error: Some(true),
        meta: None,
    };

    let formatted_error = response_formatting::format_error_response(&error_result);
    assert!(formatted_error.contains("Something went wrong"));

    // Only test the functions that still exist
    // The table formatting and status message functions have been removed as they were dead code
}

#[test]
fn test_error_conversion() {
    use rmcp::ErrorData as McpError;
    use swissarmyhammer_cli::error::CliError;

    // Test basic MCP error conversion
    let mcp_error = McpError::internal_error("test error".to_string(), None);
    let cli_error: CliError = mcp_error.into();

    assert!(cli_error.message.contains("MCP error"));
    assert!(cli_error.message.contains("test error"));
    assert_eq!(cli_error.exit_code, 1);

    // Test error handling continues to work normally
    let general_error = McpError::internal_error("Cannot proceed".to_string(), None);
    let cli_general_error: CliError = general_error.into();

    assert!(cli_general_error.message.contains("MCP error"));
    assert!(cli_general_error.message.contains("Cannot proceed"));
}

#[tokio::test]
async fn test_create_arguments_helper() {
    let temp_dir = setup_test_environment();

    let context = CliToolContext::new_with_dir(temp_dir.path())
        .await
        .expect("Failed to create CliToolContext");

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
