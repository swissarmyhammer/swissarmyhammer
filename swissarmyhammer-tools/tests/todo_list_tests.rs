//! Unit tests for todo list functionality
//!
//! These tests verify the todo_list MCP tool including:
//! - Filtering by completion status
//! - Sorting (incomplete first, then by ULID)
//! - Edge cases (empty lists, all complete, all incomplete)
//! - Count accuracy (total, completed, pending)

use serde_json::json;
use std::sync::Arc;
use swissarmyhammer_config::agent::AgentConfig;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_issues::{FileSystemIssueStorage, IssueStorage};
use swissarmyhammer_memoranda::{MarkdownMemoStorage, MemoStorage};
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolContext};
use swissarmyhammer_tools::mcp::tools::todo::create::CreateTodoTool;
use swissarmyhammer_tools::mcp::tools::todo::list::ListTodoTool;
use swissarmyhammer_tools::mcp::tools::todo::mark_complete::MarkCompleteTodoTool;
use tempfile::TempDir;

/// Create a test context for MCP tools
fn create_test_context(temp_dir: &TempDir) -> ToolContext {
    // Set environment variable to override todo directory for isolation
    std::env::set_var("SWISSARMYHAMMER_TODO_DIR", temp_dir.path());

    let issue_storage: Arc<tokio::sync::RwLock<Box<dyn IssueStorage>>> =
        Arc::new(tokio::sync::RwLock::new(Box::new(
            FileSystemIssueStorage::new(tempfile::tempdir().unwrap().path().to_path_buf()).unwrap(),
        )));
    let git_ops: Arc<tokio::sync::Mutex<Option<GitOperations>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    let memo_temp_dir = tempfile::tempdir().unwrap().path().join("memos");
    let memo_storage: Arc<tokio::sync::RwLock<Box<dyn MemoStorage>>> = Arc::new(
        tokio::sync::RwLock::new(Box::new(MarkdownMemoStorage::new(memo_temp_dir))),
    );

    let tool_handlers = Arc::new(ToolHandlers::new(memo_storage.clone()));

    ToolContext::new(
        tool_handlers,
        issue_storage,
        git_ops,
        memo_storage,
        Arc::new(AgentConfig::default()),
    )
}

/// Helper to extract text content from CallToolResult
fn extract_text_content(result: &rmcp::model::CallToolResult) -> &str {
    let content = result.content.first().expect("Expected content");
    match &content.raw {
        rmcp::model::RawContent::Text(text_content) => &text_content.text,
        _ => panic!("Expected text content"),
    }
}

#[tokio::test]
async fn test_list_empty_todos() {
    let temp_dir = TempDir::new().unwrap();
    let context = create_test_context(&temp_dir);

    let list_tool = ListTodoTool::new();
    let args = serde_json::Map::new();

    let result = list_tool.execute(args, &context).await;

    assert!(result.is_ok(), "List should succeed on empty todo list");
    let result = result.unwrap();

    let text = extract_text_content(&result);
    let response: serde_json::Value = serde_json::from_str(text).unwrap();

    assert_eq!(response["total"], 0);
    assert_eq!(response["completed"], 0);
    assert_eq!(response["pending"], 0);
    assert_eq!(response["todos"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_list_all_incomplete_todos() {
    let temp_dir = TempDir::new().unwrap();
    let context = create_test_context(&temp_dir);

    // Create three incomplete todos
    let create_tool = CreateTodoTool::new();

    for i in 1..=3 {
        let mut args = serde_json::Map::new();
        args.insert("task".to_string(), json!(format!("Task {}", i)));
        create_tool.execute(args, &context).await.unwrap();
    }

    // List all todos
    let list_tool = ListTodoTool::new();
    let args = serde_json::Map::new();

    let result = list_tool.execute(args, &context).await.unwrap();

    let text = extract_text_content(&result);
    let response: serde_json::Value = serde_json::from_str(text).unwrap();

    assert_eq!(response["total"], 3);
    assert_eq!(response["completed"], 0);
    assert_eq!(response["pending"], 3);

    let todos = response["todos"].as_array().unwrap();
    assert_eq!(todos.len(), 3);

    // Verify all are incomplete
    for todo in todos {
        assert_eq!(todo["done"], false);
    }

    // Verify tasks are in order
    assert_eq!(todos[0]["task"], "Task 1");
    assert_eq!(todos[1]["task"], "Task 2");
    assert_eq!(todos[2]["task"], "Task 3");
}

#[tokio::test]
async fn test_list_all_complete_todos() {
    let temp_dir = TempDir::new().unwrap();
    let context = create_test_context(&temp_dir);

    // Create three todos and mark them all complete
    let create_tool = CreateTodoTool::new();
    let mark_complete_tool = MarkCompleteTodoTool::new();

    let mut todo_ids = Vec::new();

    for i in 1..=3 {
        let mut args = serde_json::Map::new();
        args.insert("task".to_string(), json!(format!("Task {}", i)));
        let result = create_tool.execute(args, &context).await.unwrap();
        let text = extract_text_content(&result);
        let response: serde_json::Value = serde_json::from_str(text).unwrap();
        todo_ids.push(response["todo_item"]["id"].as_str().unwrap().to_string());
    }

    // Mark all complete
    for id in todo_ids {
        let mut args = serde_json::Map::new();
        args.insert("id".to_string(), json!(id));
        mark_complete_tool.execute(args, &context).await.unwrap();
    }

    // List all todos - should be empty since all are complete and file gets deleted
    let list_tool = ListTodoTool::new();
    let args = serde_json::Map::new();

    let result = list_tool.execute(args, &context).await.unwrap();

    let text = extract_text_content(&result);
    let response: serde_json::Value = serde_json::from_str(text).unwrap();

    assert_eq!(response["total"], 0);
    assert_eq!(response["completed"], 0);
    assert_eq!(response["pending"], 0);
    assert_eq!(response["todos"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_list_mixed_todos() {
    let temp_dir = TempDir::new().unwrap();
    let context = create_test_context(&temp_dir);

    // Create five todos and mark some complete
    let create_tool = CreateTodoTool::new();
    let mark_complete_tool = MarkCompleteTodoTool::new();

    let mut todo_ids = Vec::new();

    for i in 1..=5 {
        let mut args = serde_json::Map::new();
        args.insert("task".to_string(), json!(format!("Task {}", i)));
        let result = create_tool.execute(args, &context).await.unwrap();
        let text = extract_text_content(&result);
        let response: serde_json::Value = serde_json::from_str(text).unwrap();
        todo_ids.push(response["todo_item"]["id"].as_str().unwrap().to_string());
    }

    // Mark tasks 2 and 4 as complete
    for i in [1, 3] {
        let mut args = serde_json::Map::new();
        args.insert("id".to_string(), json!(todo_ids[i]));
        mark_complete_tool.execute(args, &context).await.unwrap();
    }

    // List all todos
    let list_tool = ListTodoTool::new();
    let args = serde_json::Map::new();

    let result = list_tool.execute(args, &context).await.unwrap();

    let text = extract_text_content(&result);
    let response: serde_json::Value = serde_json::from_str(text).unwrap();

    assert_eq!(response["total"], 5);
    assert_eq!(response["completed"], 2);
    assert_eq!(response["pending"], 3);

    let todos = response["todos"].as_array().unwrap();
    assert_eq!(todos.len(), 5);

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
    let temp_dir = TempDir::new().unwrap();
    let context = create_test_context(&temp_dir);

    // Create mixed todos
    let create_tool = CreateTodoTool::new();
    let mark_complete_tool = MarkCompleteTodoTool::new();

    let mut todo_ids = Vec::new();

    for i in 1..=4 {
        let mut args = serde_json::Map::new();
        args.insert("task".to_string(), json!(format!("Task {}", i)));
        let result = create_tool.execute(args, &context).await.unwrap();
        let text = extract_text_content(&result);
        let response: serde_json::Value = serde_json::from_str(text).unwrap();
        todo_ids.push(response["todo_item"]["id"].as_str().unwrap().to_string());
    }

    // Mark task 2 as complete
    let mut args = serde_json::Map::new();
    args.insert("id".to_string(), json!(todo_ids[1]));
    mark_complete_tool.execute(args, &context).await.unwrap();

    // List incomplete todos only
    let list_tool = ListTodoTool::new();
    let mut args = serde_json::Map::new();
    args.insert("completed".to_string(), json!(false));

    let result = list_tool.execute(args, &context).await.unwrap();

    let text = extract_text_content(&result);
    let response: serde_json::Value = serde_json::from_str(text).unwrap();

    assert_eq!(response["total"], 3);
    assert_eq!(response["completed"], 0);
    assert_eq!(response["pending"], 3);

    let todos = response["todos"].as_array().unwrap();
    assert_eq!(todos.len(), 3);

    // Verify all are incomplete
    for todo in todos {
        assert_eq!(todo["done"], false);
    }

    // Verify task 2 is not in the list
    for todo in todos {
        assert_ne!(todo["task"], "Task 2");
    }
}

#[tokio::test]
async fn test_list_filter_completed_only() {
    let temp_dir = TempDir::new().unwrap();
    let context = create_test_context(&temp_dir);

    // Create mixed todos
    let create_tool = CreateTodoTool::new();
    let mark_complete_tool = MarkCompleteTodoTool::new();

    let mut todo_ids = Vec::new();

    for i in 1..=4 {
        let mut args = serde_json::Map::new();
        args.insert("task".to_string(), json!(format!("Task {}", i)));
        let result = create_tool.execute(args, &context).await.unwrap();
        let text = extract_text_content(&result);
        let response: serde_json::Value = serde_json::from_str(text).unwrap();
        todo_ids.push(response["todo_item"]["id"].as_str().unwrap().to_string());
    }

    // Mark tasks 1 and 3 as complete
    for i in [0, 2] {
        let mut args = serde_json::Map::new();
        args.insert("id".to_string(), json!(todo_ids[i]));
        mark_complete_tool.execute(args, &context).await.unwrap();
    }

    // List completed todos only
    let list_tool = ListTodoTool::new();
    let mut args = serde_json::Map::new();
    args.insert("completed".to_string(), json!(true));

    let result = list_tool.execute(args, &context).await.unwrap();

    let text = extract_text_content(&result);
    let response: serde_json::Value = serde_json::from_str(text).unwrap();

    assert_eq!(response["total"], 2);
    assert_eq!(response["completed"], 2);
    assert_eq!(response["pending"], 0);

    let todos = response["todos"].as_array().unwrap();
    assert_eq!(todos.len(), 2);

    // Verify all are complete
    for todo in todos {
        assert_eq!(todo["done"], true);
    }

    // Verify correct tasks
    assert_eq!(todos[0]["task"], "Task 1");
    assert_eq!(todos[1]["task"], "Task 3");
}

#[tokio::test]
async fn test_list_sorting_by_ulid() {
    let temp_dir = TempDir::new().unwrap();
    let context = create_test_context(&temp_dir);

    // Create todos with small delays to ensure different ULIDs
    let create_tool = CreateTodoTool::new();

    let mut todo_ids = Vec::new();

    for i in 1..=3 {
        let mut args = serde_json::Map::new();
        args.insert("task".to_string(), json!(format!("Task {}", i)));
        let result = create_tool.execute(args, &context).await.unwrap();
        let text = extract_text_content(&result);
        let response: serde_json::Value = serde_json::from_str(text).unwrap();
        todo_ids.push(response["todo_item"]["id"].as_str().unwrap().to_string());

        // Small delay to ensure ULID ordering
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    }

    // List all todos
    let list_tool = ListTodoTool::new();
    let args = serde_json::Map::new();

    let result = list_tool.execute(args, &context).await.unwrap();

    let text = extract_text_content(&result);
    let response: serde_json::Value = serde_json::from_str(text).unwrap();
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
    let temp_dir = TempDir::new().unwrap();
    let context = create_test_context(&temp_dir);

    // Create todos with and without context
    let create_tool = CreateTodoTool::new();

    let mut args1 = serde_json::Map::new();
    args1.insert("task".to_string(), json!("Task with context"));
    args1.insert("context".to_string(), json!("Some important context"));
    create_tool.execute(args1, &context).await.unwrap();

    let mut args2 = serde_json::Map::new();
    args2.insert("task".to_string(), json!("Task without context"));
    create_tool.execute(args2, &context).await.unwrap();

    // List all todos
    let list_tool = ListTodoTool::new();
    let args = serde_json::Map::new();

    let result = list_tool.execute(args, &context).await.unwrap();

    let text = extract_text_content(&result);
    let response: serde_json::Value = serde_json::from_str(text).unwrap();
    let todos = response["todos"].as_array().unwrap();

    assert_eq!(todos.len(), 2);

    // Verify context field is present
    assert_eq!(todos[0]["context"], "Some important context");
    assert!(todos[1]["context"].is_null());
}

#[tokio::test]
async fn test_list_includes_all_required_fields() {
    let temp_dir = TempDir::new().unwrap();
    let context = create_test_context(&temp_dir);

    // Create a todo
    let create_tool = CreateTodoTool::new();
    let mut args = serde_json::Map::new();
    args.insert("task".to_string(), json!("Test task"));
    args.insert("context".to_string(), json!("Test context"));
    create_tool.execute(args, &context).await.unwrap();

    // List todos
    let list_tool = ListTodoTool::new();
    let args = serde_json::Map::new();

    let result = list_tool.execute(args, &context).await.unwrap();

    let text = extract_text_content(&result);
    let response: serde_json::Value = serde_json::from_str(text).unwrap();
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
    let temp_dir = TempDir::new().unwrap();
    let context = create_test_context(&temp_dir);

    // Create only incomplete todos
    let create_tool = CreateTodoTool::new();

    for i in 1..=3 {
        let mut args = serde_json::Map::new();
        args.insert("task".to_string(), json!(format!("Task {}", i)));
        create_tool.execute(args, &context).await.unwrap();
    }

    // Filter for completed todos
    let list_tool = ListTodoTool::new();
    let mut args = serde_json::Map::new();
    args.insert("completed".to_string(), json!(true));

    let result = list_tool.execute(args, &context).await.unwrap();

    let text = extract_text_content(&result);
    let response: serde_json::Value = serde_json::from_str(text).unwrap();

    assert_eq!(response["total"], 0);
    assert_eq!(response["completed"], 0);
    assert_eq!(response["pending"], 0);
    assert_eq!(response["todos"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_list_edge_case_filter_incomplete_when_none_exist() {
    let temp_dir = TempDir::new().unwrap();
    let context = create_test_context(&temp_dir);

    // Create todos and mark all as complete except one
    let create_tool = CreateTodoTool::new();
    let mark_complete_tool = MarkCompleteTodoTool::new();

    let mut todo_ids = Vec::new();

    for i in 1..=3 {
        let mut args = serde_json::Map::new();
        args.insert("task".to_string(), json!(format!("Task {}", i)));
        let result = create_tool.execute(args, &context).await.unwrap();
        let text = extract_text_content(&result);
        let response: serde_json::Value = serde_json::from_str(text).unwrap();
        todo_ids.push(response["todo_item"]["id"].as_str().unwrap().to_string());
    }

    // Mark all as complete
    for id in todo_ids {
        let mut args = serde_json::Map::new();
        args.insert("id".to_string(), json!(id));
        mark_complete_tool.execute(args, &context).await.unwrap();
    }

    // Filter for incomplete todos - should return empty since file is deleted when all complete
    let list_tool = ListTodoTool::new();
    let mut args = serde_json::Map::new();
    args.insert("completed".to_string(), json!(false));

    let result = list_tool.execute(args, &context).await.unwrap();

    let text = extract_text_content(&result);
    let response: serde_json::Value = serde_json::from_str(text).unwrap();

    assert_eq!(response["total"], 0);
    assert_eq!(response["completed"], 0);
    assert_eq!(response["pending"], 0);
    assert_eq!(response["todos"].as_array().unwrap().len(), 0);
}
