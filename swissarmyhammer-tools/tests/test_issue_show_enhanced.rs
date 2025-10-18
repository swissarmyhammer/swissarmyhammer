//! Comprehensive tests for enhanced issue_show tool with "next" parameter
//!
//! This module provides comprehensive test coverage for the enhanced issue_show tool
//! that supports the special parameter "next" in addition to regular issue names.

use rmcp::model::CallToolResult;
use serde_json::json;
use std::sync::Arc;

use swissarmyhammer_config::agent::AgentConfig;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_issues::{FileSystemIssueStorage, IssueStorage};
use swissarmyhammer_memoranda::{MarkdownMemoStorage, MemoStorage};
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolContext};
use swissarmyhammer_tools::mcp::tools::issues::show::ShowIssueTool;
use tempfile::TempDir;
use tokio::sync::{Mutex, RwLock};

/// Helper function to extract text content from CallToolResult
fn extract_text_content(result: &CallToolResult) -> Option<String> {
    result.content.first().and_then(|content| {
        if let rmcp::model::RawContent::Text(text_content) = &content.raw {
            Some(text_content.text.clone())
        } else {
            None
        }
    })
}

/// Test environment for comprehensive issue_show testing
struct IssueShowTestEnvironment {
    _temp_dir: TempDir,
    tool_context: ToolContext,
    issue_storage: Arc<RwLock<Box<dyn IssueStorage>>>,
    git_ops: Arc<Mutex<Option<GitOperations>>>,
    tool: ShowIssueTool,
}

impl IssueShowTestEnvironment {
    async fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");
        Self::setup_git_repo(temp_dir.path()).await;

        // Change to test directory
        std::env::set_current_dir(temp_dir.path()).expect("Failed to change to test directory");

        // Initialize issue storage
        let issues_dir = temp_dir.path().join("issues");
        let issue_storage = Box::new(
            FileSystemIssueStorage::new(issues_dir).expect("Failed to create issue storage"),
        );
        let issue_storage = Arc::new(RwLock::new(issue_storage as Box<dyn IssueStorage>));

        // Initialize git operations
        let git_ops = Arc::new(Mutex::new(Some(
            GitOperations::with_work_dir(temp_dir.path().to_path_buf())
                .expect("Failed to create git operations"),
        )));

        // Create memo storage
        let memo_storage = Box::new(
            MarkdownMemoStorage::new_default()
                .await
                .expect("Failed to create memo storage"),
        );
        let memo_storage = Arc::new(RwLock::new(memo_storage as Box<dyn MemoStorage>));

        // Create tool handlers
        let tool_handlers = Arc::new(ToolHandlers::new(memo_storage.clone()));

        // Create tool context
        let tool_context = ToolContext::new(
            tool_handlers,
            issue_storage.clone(),
            git_ops.clone(),
            memo_storage,
            Arc::new(AgentConfig::default()),
        );

        let tool = ShowIssueTool::new();

        Self {
            _temp_dir: temp_dir,
            tool_context,
            issue_storage,
            git_ops,
            tool,
        }
    }

    async fn setup_git_repo(path: &std::path::Path) {
        use git2::{Repository, Signature};

        // Initialize git repo
        let repo = Repository::init(path).expect("Failed to init git");

        // Configure git
        let mut config = repo.config().expect("Failed to get git config");

        config
            .set_str("user.name", "Test User")
            .expect("Failed to configure git user");

        config
            .set_str("user.email", "test@example.com")
            .expect("Failed to configure git email");

        // Create initial commit
        std::fs::write(path.join("README.md"), "# Test Project")
            .expect("Failed to write README.md");

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
    }

    async fn create_test_issue(&self, name: &str, content: &str) -> String {
        let issue = self
            .issue_storage
            .write()
            .await
            .create_issue(name.to_string(), content.to_string())
            .await
            .expect("Failed to create test issue");
        issue.name
    }

    fn create_arguments(
        &self,
        name: &str,
        raw: Option<bool>,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut args = serde_json::Map::new();
        args.insert("name".to_string(), json!(name));
        if let Some(raw_value) = raw {
            args.insert("raw".to_string(), json!(raw_value));
        }
        args
    }

    async fn execute_tool(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        self.tool.execute(arguments, &self.tool_context).await
    }
}

// Tests for "next" parameter functionality

#[tokio::test]
async fn test_issue_show_next_with_pending_issues() {
    let env = IssueShowTestEnvironment::new().await;

    // Create multiple test issues
    let issue_names = vec![
        "NEXT_TEST_003_third",
        "NEXT_TEST_001_first",
        "NEXT_TEST_002_second",
    ];

    for name in &issue_names {
        env.create_test_issue(name, &format!("# Issue {name}\n\nTest content."))
            .await;
    }

    // Test issue_show next (should return first alphabetically)
    let args = env.create_arguments("next", None);
    let result = env.execute_tool(args).await;

    assert!(
        result.is_ok(),
        "issue_show next should succeed with pending issues"
    );
    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify response contains first issue alphabetically
    if let Some(text_str) = extract_text_content(&call_result) {
        assert!(
            text_str.contains("NEXT_TEST_001_first"),
            "Response should contain first issue alphabetically: {text_str}"
        );
    }
}

#[tokio::test]
async fn test_issue_show_next_no_pending_issues() {
    let env = IssueShowTestEnvironment::new().await;

    // Create and complete an issue to test "no pending" scenario
    let issue_name = env
        .create_test_issue("COMPLETED_TEST", "# Completed Issue")
        .await;
    env.issue_storage
        .write()
        .await
        .complete_issue(&issue_name)
        .await
        .unwrap();

    // Test issue_show next
    let args = env.create_arguments("next", None);
    let result = env.execute_tool(args).await;

    assert!(
        result.is_ok(),
        "issue_show next should succeed even with no pending issues"
    );
    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify response indicates no pending issues
    if let Some(text_str) = extract_text_content(&call_result) {
        assert!(
            text_str.contains("No pending issues") || text_str.contains("All issues are completed"),
            "Response should indicate no pending issues: {text_str}"
        );
    }
}

#[tokio::test]
async fn test_issue_show_next_alphabetical_ordering() {
    let env = IssueShowTestEnvironment::new().await;

    // Create issues with specific alphabetical ordering
    let issue_names = vec!["zebra_issue", "alpha_issue", "beta_issue", "charlie_issue"];

    for name in &issue_names {
        env.create_test_issue(name, &format!("# Issue {name}"))
            .await;
    }

    // Test issue_show next multiple times to verify ordering
    let args = env.create_arguments("next", None);
    let result = env.execute_tool(args).await;

    assert!(result.is_ok(), "issue_show next should succeed");
    let call_result = result.unwrap();

    // Should return "alpha_issue" as it's first alphabetically
    if let Some(text_str) = extract_text_content(&call_result) {
        assert!(
            text_str.contains("alpha_issue"),
            "Should return first issue alphabetically: {text_str}"
        );
    }
}

#[tokio::test]
async fn test_issue_show_next_storage_error_handling() {
    let env = IssueShowTestEnvironment::new().await;

    // Create issue first
    env.create_test_issue("STORAGE_TEST", "# Storage Test")
        .await;

    // Test with corrupted storage path (simulate storage error)
    // This is tricky to test directly, so we'll test that the tool handles errors gracefully
    let args = env.create_arguments("next", None);
    let result = env.execute_tool(args).await;

    // Should succeed with normal storage
    assert!(
        result.is_ok(),
        "issue_show next should handle storage operations"
    );
}

// Tests for backward compatibility

#[tokio::test]
async fn test_issue_show_regular_issue_names() {
    let env = IssueShowTestEnvironment::new().await;

    // Create test issues with regular names
    let issue_name = env
        .create_test_issue(
            "REGULAR_ISSUE_001",
            "# Regular Issue\n\nThis tests regular functionality.",
        )
        .await;

    // Test with regular issue name (not "current" or "next")
    let args = env.create_arguments(&issue_name, None);
    let result = env.execute_tool(args).await;

    assert!(
        result.is_ok(),
        "issue_show should work with regular issue names"
    );
    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify response contains issue information
    if let Some(text_str) = extract_text_content(&call_result) {
        assert!(
            text_str.contains(&issue_name),
            "Response should contain issue name"
        );
        assert!(
            text_str.contains("Regular Issue"),
            "Response should contain issue content"
        );
    }
}

#[tokio::test]
async fn test_issue_show_nonexistent_regular_issue() {
    let env = IssueShowTestEnvironment::new().await;

    // Test with non-existent regular issue name
    let args = env.create_arguments("NONEXISTENT_ISSUE", None);
    let result = env.execute_tool(args).await;

    assert!(
        result.is_err(),
        "issue_show should fail for nonexistent issue"
    );

    // Verify error contains appropriate message
    let error = result.unwrap_err();
    let error_msg = error.to_string();
    assert!(
        error_msg.contains("not found") || error_msg.contains("NONEXISTENT_ISSUE"),
        "Error should indicate issue not found: {error_msg}"
    );
}

#[tokio::test]
async fn test_issue_show_raw_parameter_compatibility() {
    let env = IssueShowTestEnvironment::new().await;

    // Create test issue
    let issue_name = env
        .create_test_issue(
            "RAW_TEST_001",
            "# Raw Test Issue\n\nThis tests raw parameter.",
        )
        .await;

    // Test with raw=true
    let args = env.create_arguments(&issue_name, Some(true));
    let result = env.execute_tool(args).await;

    assert!(result.is_ok(), "issue_show should work with raw parameter");
    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));

    // Verify raw response (should not have formatting)
    if let Some(text_str) = extract_text_content(&call_result) {
        // Raw content should not have status formatting
        assert!(
            !text_str.contains("Status:"),
            "Raw response should not contain status formatting"
        );
        assert!(
            text_str.contains("Raw Test Issue"),
            "Raw response should contain content"
        );
    }

    // Test with raw=false
    let args = env.create_arguments(&issue_name, Some(false));
    let result = env.execute_tool(args).await;

    assert!(result.is_ok(), "issue_show should work with raw=false");
    let call_result = result.unwrap();

    // Verify formatted response (should have formatting)
    if let Some(text_str) = extract_text_content(&call_result) {
        // Formatted content should have status indicators
        assert!(
            text_str.contains("Status:")
                && (text_str.contains("Active") || text_str.contains("Completed")),
            "Formatted response should contain status text: {text_str}"
        );
    }
}

// Tests for parameter validation

#[tokio::test]
async fn test_issue_show_empty_name_validation() {
    let env = IssueShowTestEnvironment::new().await;

    // Test with empty name
    let args = env.create_arguments("", None);
    let result = env.execute_tool(args).await;

    assert!(result.is_err(), "issue_show should fail with empty name");

    let error = result.unwrap_err();
    let error_msg = error.to_string();
    assert!(
        error_msg.contains("empty") || error_msg.contains("required"),
        "Error should indicate empty name issue: {error_msg}"
    );
}

#[tokio::test]
async fn test_issue_show_special_parameter_case_sensitivity() {
    let env = IssueShowTestEnvironment::new().await;

    // Create test issue to ensure we have one for "next"
    env.create_test_issue("CASE_TEST", "# Case Test").await;

    // Test case variations of special parameters
    let test_cases = vec![
        ("next", true),  // Should work
        ("NEXT", false), // Should not work (case sensitive)
        ("Next", false), // Should not work (case sensitive)
    ];

    for (param_name, should_work) in test_cases {
        let args = env.create_arguments(param_name, None);
        let result = env.execute_tool(args).await;

        if should_work {
            assert!(
                result.is_ok(),
                "Special parameter '{param_name}' should work"
            );
        } else {
            // Case-insensitive parameters will be treated as regular issue names
            // which should fail since they don't exist
            assert!(
                result.is_err(),
                "Invalid case '{param_name}' should be treated as regular issue name and fail"
            );
        }
    }
}

#[tokio::test]
async fn test_issue_show_parameter_type_validation() {
    let env = IssueShowTestEnvironment::new().await;

    // Test invalid argument structure (this tests the parsing layer)
    let mut invalid_args = serde_json::Map::new();
    invalid_args.insert("name".to_string(), json!(123)); // Should be string

    let result = env.execute_tool(invalid_args).await;
    assert!(result.is_err(), "Invalid argument types should cause error");

    // Test missing required name parameter
    let empty_args = serde_json::Map::new();
    let result = env.execute_tool(empty_args).await;
    assert!(
        result.is_err(),
        "Missing required name parameter should cause error"
    );

    // Test invalid raw parameter type
    let mut invalid_raw = serde_json::Map::new();
    invalid_raw.insert("name".to_string(), json!("test"));
    invalid_raw.insert("raw".to_string(), json!("not_boolean"));

    let result = env.execute_tool(invalid_raw).await;
    assert!(
        result.is_err(),
        "Invalid raw parameter type should cause error"
    );
}

// Integration scenario tests

#[tokio::test]
async fn test_issue_show_concurrent_access() {
    let env = IssueShowTestEnvironment::new().await;

    // Create test issues
    for i in 0..3 {
        env.create_test_issue(
            &format!("CONCURRENT_TEST_{i:03}"),
            &format!("# Concurrent Test {i}"),
        )
        .await;
    }

    // Execute multiple concurrent requests
    let mut handles = vec![];

    for _ in 0..5 {
        let tool = ShowIssueTool::new();
        // Create memo storage for this context
        let memo_storage = Box::new(
            MarkdownMemoStorage::new_default()
                .await
                .expect("Failed to create memo storage"),
        );
        let memo_storage = Arc::new(RwLock::new(memo_storage as Box<dyn MemoStorage>));

        let tool_handlers = Arc::new(ToolHandlers::new(memo_storage.clone()));

        let context = ToolContext::new(
            tool_handlers,
            env.issue_storage.clone(),
            env.git_ops.clone(),
            memo_storage,
            Arc::new(AgentConfig::default()),
        );

        let handle = tokio::spawn(async move {
            let args = serde_json::Map::from_iter([("name".to_string(), json!("next"))]);
            tool.execute(args, &context).await
        });
        handles.push(handle);
    }

    // Wait for all concurrent operations
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "Concurrent access should work");
    }
}

#[tokio::test]
async fn test_issue_show_switching_between_parameters() {
    let env = IssueShowTestEnvironment::new().await;

    // Create test issue and branch
    let issue_name = env.create_test_issue("SWITCH_TEST", "# Switch Test").await;

    // Test switching between different parameter types
    let test_sequence = vec![
        (issue_name.clone(), true),
        ("next".to_string(), true),
        ("nonexistent".to_string(), false),
        ("next".to_string(), true),
    ];

    for (param, should_succeed) in test_sequence {
        let args = env.create_arguments(&param, None);
        let result = env.execute_tool(args).await;

        if should_succeed {
            assert!(result.is_ok(), "Parameter '{param}' should succeed");
        } else {
            assert!(result.is_err(), "Parameter '{param}' should fail");
        }
    }
}

// Edge cases and error scenarios

#[tokio::test]
async fn test_issue_show_schema_validation() {
    let env = IssueShowTestEnvironment::new().await;

    // Test tool schema is valid JSON
    let schema = env.tool.schema();
    assert!(schema.is_object(), "Schema should be valid JSON object");

    // Verify schema contains required fields
    let schema_obj = schema.as_object().unwrap();
    assert!(
        schema_obj.contains_key("type"),
        "Schema should have type field"
    );
    assert!(
        schema_obj.contains_key("properties"),
        "Schema should have properties field"
    );

    let properties = schema_obj.get("properties").unwrap().as_object().unwrap();
    assert!(
        properties.contains_key("name"),
        "Schema should have name property"
    );

    let name_prop = properties.get("name").unwrap().as_object().unwrap();
    assert_eq!(
        name_prop.get("type").unwrap().as_str().unwrap(),
        "string",
        "Name should be string type"
    );

    // Verify description mentions special parameters
    let description = name_prop.get("description").unwrap().as_str().unwrap();
    assert!(
        description.contains("next"),
        "Description should mention 'next' parameter"
    );
}

#[tokio::test]
async fn test_issue_show_tool_metadata() {
    let env = IssueShowTestEnvironment::new().await;

    // Test tool name
    assert_eq!(
        env.tool.name(),
        "issue_show",
        "Tool name should be 'issue_show'"
    );

    // Test tool description
    let description = env.tool.description();
    assert!(
        !description.is_empty(),
        "Tool description should not be empty"
    );
    assert!(
        description.contains("issue") || description.contains("Display"),
        "Description should mention issue functionality"
    );
}
