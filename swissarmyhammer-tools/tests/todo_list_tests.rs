//! Unit tests for todo list functionality
//!
//! These tests verify the todo_list MCP tool including:
//! - Filtering by completion status
//! - Sorting (incomplete first, then by ULID)
//! - Edge cases (empty lists, all complete, all incomplete)
//! - Count accuracy (total, completed, pending)
//! sah rule ignore test_rule_with_allow

use serde_json::json;
use std::sync::Arc;
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::ModelConfig;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolContext};
use swissarmyhammer_tools::mcp::tools::todo::create::CreateTodoTool;
use swissarmyhammer_tools::mcp::tools::todo::list::ListTodoTool;
use swissarmyhammer_tools::mcp::tools::todo::mark_complete::MarkCompleteTodoTool;

/// Create a test context for MCP tools
fn create_test_context(env: &IsolatedTestEnvironment) -> ToolContext {
    let temp_path = env.temp_dir();

    // Create a .git directory to make it a Git repository
    std::fs::create_dir_all(temp_path.join(".git")).unwrap();

    let git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>> =
        Arc::new(tokio::sync::Mutex::new(None));

    let tool_handlers = Arc::new(ToolHandlers::new());

    ToolContext::new(tool_handlers, git_ops, Arc::new(ModelConfig::default()))
        .with_working_dir(temp_path)
}

/// Helper to extract text content from CallToolResult
fn extract_text_content(result: &rmcp::model::CallToolResult) -> &str {
    let content = result.content.first().expect("Expected content");
    match &content.raw {
        rmcp::model::RawContent::Text(text_content) => &text_content.text,
        _ => panic!("Expected text content"),
    }
}

/// Helper to create todo arguments with optional context
fn create_todo_args(
    task: &str,
    context: Option<&str>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut args = serde_json::Map::new();
    args.insert("task".to_string(), json!(task));
    if let Some(ctx) = context {
        args.insert("context".to_string(), json!(ctx));
    }
    args
}

/// Helper to extract todo ID from create response
fn extract_todo_id(result: &rmcp::model::CallToolResult) -> String {
    let text = extract_text_content(result);
    let response: serde_json::Value = serde_json::from_str(text).unwrap();
    response["todo_item"]["id"].as_str().unwrap().to_string()
}

/// Helper to setup test with isolated environment and context
fn setup_test() -> (IsolatedTestEnvironment, ToolContext) {
    let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let context = create_test_context(&env);
    (env, context)
}

/// Helper to setup test with todos
async fn setup_with_todos(count: usize) -> (IsolatedTestEnvironment, ToolContext, Vec<String>) {
    let (env, context) = setup_test();
    let todo_ids = create_todos(&context, count).await;
    (env, context, todo_ids)
}

/// Helper to create multiple todos with optional delay and return their IDs
async fn create_todos_with_delay(
    context: &ToolContext,
    count: usize,
    delay_ms: Option<u64>,
) -> Vec<String> {
    let create_tool = CreateTodoTool::new();
    let mut todo_ids = Vec::new();

    for i in 1..=count {
        let args = create_todo_args(&format!("Task {}", i), None);
        let result = create_tool.execute(args, context).await.unwrap();
        todo_ids.push(extract_todo_id(&result));

        if let Some(delay) = delay_ms {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }
    }

    todo_ids
}

/// Helper to create multiple todos and return their IDs
async fn create_todos(context: &ToolContext, count: usize) -> Vec<String> {
    create_todos_with_delay(context, count, None).await
}

/// Helper to mark multiple todos as complete
async fn mark_todos_complete(context: &ToolContext, todo_ids: &[String]) {
    let mark_complete_tool = MarkCompleteTodoTool::new();

    for id in todo_ids {
        let mut args = serde_json::Map::new();
        args.insert("id".to_string(), json!(id));
        mark_complete_tool.execute(args, context).await.unwrap();
    }
}

/// Helper to mark todos complete by indices
async fn mark_todos_complete_by_indices(
    context: &ToolContext,
    todo_ids: &[String],
    indices: &[usize],
) {
    let ids_to_complete: Vec<String> = indices.iter().map(|&i| todo_ids[i].clone()).collect();
    mark_todos_complete(context, &ids_to_complete).await;
}

/// Helper to list todos and parse the JSON response
async fn list_todos_and_parse(
    list_tool: &ListTodoTool,
    args: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> serde_json::Value {
    let result = list_tool.execute(args, context).await.unwrap();
    let text = extract_text_content(&result);
    serde_json::from_str(text).unwrap()
}

/// Helper to list todos with optional filter
async fn list_todos_with_optional_filter(
    context: &ToolContext,
    completed: Option<bool>,
) -> serde_json::Value {
    let list_tool = ListTodoTool::new();
    let mut args = serde_json::Map::new();
    if let Some(status) = completed {
        args.insert("completed".to_string(), json!(status));
    }
    list_todos_and_parse(&list_tool, args, context).await
}

/// Helper to assert todo counts in response
fn assert_todo_counts(response: &serde_json::Value, total: i64, completed: i64, pending: i64) {
    assert_eq!(response["total"], total);
    assert_eq!(response["completed"], completed);
    assert_eq!(response["pending"], pending);
    assert_eq!(response["todos"].as_array().unwrap().len(), total as usize);
}

/// Helper to assert todos completion status
fn assert_todos_completion_status(todos: &[serde_json::Value], expected_status: bool) {
    for todo in todos {
        assert_eq!(todo["done"], expected_status);
    }
}

#[tokio::test]
async fn test_list_empty_todos() {
    let (_temp_dir, context) = setup_test();

    let list_tool = ListTodoTool::new();
    let args = serde_json::Map::new();

    let result = list_tool.execute(args.clone(), &context).await;

    assert!(result.is_ok(), "List should succeed on empty todo list");

    let response = list_todos_with_optional_filter(&context, None).await;

    assert_todo_counts(&response, 0, 0, 0);
}

#[tokio::test]
async fn test_list_all_incomplete_todos() {
    let (_temp_dir, context, _todo_ids) = setup_with_todos(3).await;

    // List all todos
    let response = list_todos_with_optional_filter(&context, None).await;

    assert_todo_counts(&response, 3, 0, 3);

    let todos = response["todos"].as_array().unwrap();

    // Verify all are incomplete
    assert_todos_completion_status(todos, false);

    // Verify tasks are in order
    assert_eq!(todos[0]["task"], "Task 1");
    assert_eq!(todos[1]["task"], "Task 2");
    assert_eq!(todos[2]["task"], "Task 3");
}

#[tokio::test]
async fn test_list_all_complete_todos() {
    let (_temp_dir, context, todo_ids) = setup_with_todos(3).await;

    // Mark them all complete
    mark_todos_complete(&context, &todo_ids).await;

    // List all todos - should be empty since all are complete and file gets deleted
    let response = list_todos_with_optional_filter(&context, None).await;

    assert_todo_counts(&response, 0, 0, 0);
}

#[tokio::test]
async fn test_list_mixed_todos() {
    let (_temp_dir, context, todo_ids) = setup_with_todos(5).await;

    // Mark tasks 2 and 4 as complete (indices 1 and 3)
    mark_todos_complete_by_indices(&context, &todo_ids, &[1, 3]).await;

    // List all todos
    let response = list_todos_with_optional_filter(&context, None).await;

    assert_todo_counts(&response, 5, 2, 3);

    let todos = response["todos"].as_array().unwrap();

    // Verify sorting: incomplete first, then complete
    assert_eq!(todos[0]["done"], false);
    assert_eq!(todos[0]["task"], "Task 1");
    assert_eq!(todos[1]["done"], false);
    assert_eq!(todos[1]["task"], "Task 3");
    assert_eq!(todos[2]["done"], false);
    assert_eq!(todos[2]["task"], "Task 5");
    assert_eq!(todos[3]["done"], true);
    assert_eq!(todos[3]["task"], "Task 2");
    assert_eq!(todos[4]["done"], true);
    assert_eq!(todos[4]["task"], "Task 4");
}

#[tokio::test]
async fn test_list_filter_incomplete_only() {
    let (_temp_dir, context, todo_ids) = setup_with_todos(4).await;

    // Mark task 2 as complete (index 1)
    mark_todos_complete_by_indices(&context, &todo_ids, &[1]).await;

    // List incomplete todos only
    let response = list_todos_with_optional_filter(&context, Some(false)).await;

    assert_todo_counts(&response, 3, 0, 3);

    let todos = response["todos"].as_array().unwrap();

    // Verify all are incomplete
    assert_todos_completion_status(todos, false);

    // Verify task 2 is not in the list
    for todo in todos {
        assert_ne!(todo["task"], "Task 2");
    }
}

#[tokio::test]
async fn test_list_filter_completed_only() {
    let (_temp_dir, context, todo_ids) = setup_with_todos(4).await;

    // Mark tasks 1 and 3 as complete (indices 0 and 2)
    mark_todos_complete_by_indices(&context, &todo_ids, &[0, 2]).await;

    // List completed todos only
    let response = list_todos_with_optional_filter(&context, Some(true)).await;

    assert_todo_counts(&response, 2, 2, 0);

    let todos = response["todos"].as_array().unwrap();

    // Verify all are complete
    assert_todos_completion_status(todos, true);

    // Verify correct tasks
    assert_eq!(todos[0]["task"], "Task 1");
    assert_eq!(todos[1]["task"], "Task 3");
}

#[tokio::test]
async fn test_list_sorting_by_ulid() {
    let (_temp_dir, context) = setup_test();

    // Create todos with small delays to ensure different ULIDs
    let todo_ids = create_todos_with_delay(&context, 3, Some(5)).await;

    // List all todos
    let response = list_todos_with_optional_filter(&context, None).await;
    let todos = response["todos"].as_array().unwrap();

    // Verify IDs are in ascending order (ULIDs are time-ordered)
    assert_eq!(todos[0]["id"], todo_ids[0]);
    assert_eq!(todos[1]["id"], todo_ids[1]);
    assert_eq!(todos[2]["id"], todo_ids[2]);

    // Verify ULIDs are in lexicographic order
    let id1 = todos[0]["id"].as_str().unwrap();
    let id2 = todos[1]["id"].as_str().unwrap();
    let id3 = todos[2]["id"].as_str().unwrap();

    assert!(id1 < id2);
    assert!(id2 < id3);
}

#[tokio::test]
async fn test_list_with_context_field() {
    let (_temp_dir, context) = setup_test();

    // Create todos with and without context
    let create_tool = CreateTodoTool::new();

    let args1 = create_todo_args("Task with context", Some("Some important context"));
    create_tool.execute(args1, &context).await.unwrap();

    let args2 = create_todo_args("Task without context", None);
    create_tool.execute(args2, &context).await.unwrap();

    // List all todos
    let response = list_todos_with_optional_filter(&context, None).await;
    let todos = response["todos"].as_array().unwrap();

    assert_eq!(todos.len(), 2);

    // Verify context field is present
    assert_eq!(todos[0]["context"], "Some important context");
    assert!(todos[1]["context"].is_null());
}

#[tokio::test]
async fn test_list_includes_all_required_fields() {
    let (_temp_dir, context) = setup_test();

    // Create a todo
    let create_tool = CreateTodoTool::new();
    let args = create_todo_args("Test task", Some("Test context"));
    create_tool.execute(args, &context).await.unwrap();

    // List todos
    let response = list_todos_with_optional_filter(&context, None).await;
    let todos = response["todos"].as_array().unwrap();

    assert_eq!(todos.len(), 1);

    let todo = &todos[0];

    // Verify all required fields are present
    assert!(todo["id"].is_string());
    assert_eq!(todo["task"], "Test task");
    assert_eq!(todo["context"], "Test context");
    assert_eq!(todo["done"], false);
}

#[tokio::test]
async fn test_list_edge_case_filter_completed_when_none_exist() {
    let (_temp_dir, context, _todo_ids) = setup_with_todos(3).await;

    // Filter for completed todos
    let response = list_todos_with_optional_filter(&context, Some(true)).await;

    assert_todo_counts(&response, 0, 0, 0);
}

#[tokio::test]
async fn test_list_edge_case_filter_incomplete_when_none_exist() {
    let (_temp_dir, context, todo_ids) = setup_with_todos(3).await;

    // Mark all as complete
    mark_todos_complete(&context, &todo_ids).await;

    // Filter for incomplete todos - should return empty since file is deleted when all complete
    let response = list_todos_with_optional_filter(&context, Some(false)).await;

    assert_todo_counts(&response, 0, 0, 0);
}
