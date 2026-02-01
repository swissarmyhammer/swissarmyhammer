//! Comprehensive in-process CLI integration tests for all kanban operations
//!
//! These tests verify the CLI interface for all kanban operations without
//! spawning external processes, making them fast and isolated.

use swissarmyhammer_cli::cli_executor::CliExecutor;
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;

/// Helper to extract ID from YAML output
fn extract_id(output: &str) -> String {
    output
        .lines()
        .find(|line| line.trim().starts_with("id:"))
        .map(|line| {
            line.split(':')
                .nth(1)
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .to_string()
        })
        .unwrap_or_default()
}

// ============================================
// BOARD OPERATIONS (3 tests)
// ============================================

#[tokio::test]
async fn test_kanban_board_init() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let executor = CliExecutor::new(&env.temp_dir()).await.unwrap();

    let result = executor
        .execute(&[
            "tool",
            "kanban",
            "board",
            "init",
            "--name",
            "Test Board",
            "--description",
            "A test board",
        ])
        .await;

    assert_eq!(
        result.exit_code, 0,
        "Expected success, got stderr: {}",
        result.stderr
    );
    assert!(
        result.stdout.contains("Test Board"),
        "Output should contain board name"
    );
}

#[tokio::test]
async fn test_kanban_board_get() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let executor = CliExecutor::new(&env.temp_dir()).await.unwrap();

    // Setup
    executor
        .execute(&["tool", "kanban", "board", "init", "--name", "My Board"])
        .await;

    // Get board
    let result = executor.execute(&["tool", "kanban", "board", "get"]).await;

    assert_eq!(
        result.exit_code, 0,
        "Expected success, got stderr: {}",
        result.stderr
    );
    assert!(
        result.stdout.contains("My Board"),
        "Output should contain board name"
    );
}

#[tokio::test]
async fn test_kanban_board_update() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let executor = CliExecutor::new(&env.temp_dir()).await.unwrap();

    // Setup
    executor
        .execute(&["tool", "kanban", "board", "init", "--name", "Original"])
        .await;

    // Update board
    let result = executor
        .execute(&[
            "tool",
            "kanban",
            "board",
            "update",
            "--name",
            "Updated Board",
            "--description",
            "New description",
        ])
        .await;

    assert_eq!(
        result.exit_code, 0,
        "Expected success, got stderr: {}",
        result.stderr
    );
    assert!(
        result.stdout.contains("Updated Board"),
        "Output should contain updated name"
    );
}

// ============================================
// COLUMN OPERATIONS (5 tests)
// ============================================

#[tokio::test]
async fn test_kanban_column_add() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let executor = CliExecutor::new(&env.temp_dir()).await.unwrap();

    // Setup
    executor
        .execute(&["tool", "kanban", "board", "init", "--name", "Test"])
        .await;

    // Add a new column (use unique ID to avoid conflict with defaults)
    let result = executor
        .execute(&[
            "tool",
            "kanban",
            "column",
            "add",
            "--id",
            "testing",
            "--name",
            "Testing",
        ])
        .await;

    assert_eq!(
        result.exit_code, 0,
        "Expected success, got stderr: {}",
        result.stderr
    );
    assert!(
        result.stdout.contains("testing"),
        "Output should contain column id"
    );
}

#[tokio::test]
async fn test_kanban_column_get() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let executor = CliExecutor::new(&env.temp_dir()).await.unwrap();

    // Setup
    executor
        .execute(&["tool", "kanban", "board", "init", "--name", "Test"])
        .await;
    executor
        .execute(&[
            "tool",
            "kanban",
            "column",
            "add",
            "--id",
            "testing",
            "--name",
            "Testing",
        ])
        .await;

    // Get column
    let result = executor
        .execute(&["tool", "kanban", "column", "get", "--id", "testing"])
        .await;

    assert_eq!(
        result.exit_code, 0,
        "Expected success, got stderr: {}",
        result.stderr
    );
    assert!(
        result.stdout.contains("Testing"),
        "Output should contain column name"
    );
}

#[tokio::test]
async fn test_kanban_column_update() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let executor = CliExecutor::new(&env.temp_dir()).await.unwrap();

    // Setup
    executor
        .execute(&["tool", "kanban", "board", "init", "--name", "Test"])
        .await;
    executor
        .execute(&[
            "tool",
            "kanban",
            "column",
            "add",
            "--id",
            "testing",
            "--name",
            "Testing",
        ])
        .await;

    // Update column
    let result = executor
        .execute(&[
            "tool",
            "kanban",
            "column",
            "update",
            "--id",
            "testing",
            "--name",
            "QA Testing",
        ])
        .await;

    assert_eq!(
        result.exit_code, 0,
        "Expected success, got stderr: {}",
        result.stderr
    );
    assert!(
        result.stdout.contains("QA Testing"),
        "Output should contain updated name"
    );
}

#[tokio::test]
async fn test_kanban_column_delete() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let executor = CliExecutor::new(&env.temp_dir()).await.unwrap();

    // Setup
    executor
        .execute(&["tool", "kanban", "board", "init", "--name", "Test"])
        .await;
    executor
        .execute(&[
            "tool",
            "kanban",
            "column",
            "add",
            "--id",
            "testing",
            "--name",
            "Testing",
        ])
        .await;

    // Delete column
    let result = executor
        .execute(&["tool", "kanban", "column", "delete", "--id", "testing"])
        .await;

    assert_eq!(
        result.exit_code, 0,
        "Expected success, got stderr: {}",
        result.stderr
    );

    // Verify column is gone
    let list_result = executor.execute(&["tool", "kanban", "columns", "list"]).await;
    assert!(
        !list_result.stdout.contains("testing"),
        "Deleted column should not appear in list"
    );
}

#[tokio::test]
async fn test_kanban_columns_list() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let executor = CliExecutor::new(&env.temp_dir()).await.unwrap();

    // Setup - board init creates default columns (todo, in_progress, done)
    executor
        .execute(&["tool", "kanban", "board", "init", "--name", "Test"])
        .await;

    // List columns
    let result = executor.execute(&["tool", "kanban", "columns", "list"]).await;

    assert_eq!(
        result.exit_code, 0,
        "Expected success, got stderr: {}",
        result.stderr
    );
    assert!(
        result.stdout.contains("todo") || result.stdout.contains("To Do"),
        "Output should contain todo column"
    );
    assert!(
        result.stdout.contains("done") || result.stdout.contains("Done"),
        "Output should contain done column"
    );
}

// ============================================
// TASK OPERATIONS (7 tests)
// ============================================

#[tokio::test]
async fn test_kanban_task_add() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let executor = CliExecutor::new(&env.temp_dir()).await.unwrap();

    // Setup
    executor
        .execute(&["tool", "kanban", "board", "init", "--name", "Test"])
        .await;

    // Add task
    let result = executor
        .execute(&[
            "tool",
            "kanban",
            "task",
            "add",
            "--title",
            "Implement feature",
            "--description",
            "Add the new feature",
        ])
        .await;

    assert_eq!(
        result.exit_code, 0,
        "Expected success, got stderr: {}",
        result.stderr
    );
    assert!(
        result.stdout.contains("Implement feature"),
        "Output should contain task title"
    );
}

#[tokio::test]
async fn test_kanban_task_get() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let executor = CliExecutor::new(&env.temp_dir()).await.unwrap();

    // Setup
    executor
        .execute(&["tool", "kanban", "board", "init", "--name", "Test"])
        .await;
    let add_result = executor
        .execute(&["tool", "kanban", "task", "add", "--title", "My Task"])
        .await;
    let task_id = extract_id(&add_result.stdout);
    assert!(!task_id.is_empty(), "Should extract task ID from output");

    // Get task
    let result = executor
        .execute(&["tool", "kanban", "task", "get", "--id", &task_id])
        .await;

    assert_eq!(
        result.exit_code, 0,
        "Expected success, got stderr: {}",
        result.stderr
    );
    assert!(
        result.stdout.contains("My Task"),
        "Output should contain task title"
    );
}

#[tokio::test]
async fn test_kanban_task_update() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let executor = CliExecutor::new(&env.temp_dir()).await.unwrap();

    // Setup
    executor
        .execute(&["tool", "kanban", "board", "init", "--name", "Test"])
        .await;
    let add_result = executor
        .execute(&["tool", "kanban", "task", "add", "--title", "Original Title"])
        .await;
    let task_id = extract_id(&add_result.stdout);

    // Update task
    let result = executor
        .execute(&[
            "tool",
            "kanban",
            "task",
            "update",
            "--id",
            &task_id,
            "--title",
            "Updated Title",
            "--description",
            "New description",
        ])
        .await;

    assert_eq!(
        result.exit_code, 0,
        "Expected success, got stderr: {}",
        result.stderr
    );
    assert!(
        result.stdout.contains("Updated Title"),
        "Output should contain updated title"
    );
}

#[tokio::test]
async fn test_kanban_task_delete() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let executor = CliExecutor::new(&env.temp_dir()).await.unwrap();

    // Setup
    executor
        .execute(&["tool", "kanban", "board", "init", "--name", "Test"])
        .await;
    let add_result = executor
        .execute(&["tool", "kanban", "task", "add", "--title", "To Delete"])
        .await;
    let task_id = extract_id(&add_result.stdout);

    // Delete task
    let result = executor
        .execute(&["tool", "kanban", "task", "delete", "--id", &task_id])
        .await;

    assert_eq!(
        result.exit_code, 0,
        "Expected success, got stderr: {}",
        result.stderr
    );

    // Verify task is gone
    let get_result = executor
        .execute(&["tool", "kanban", "task", "get", "--id", &task_id])
        .await;
    assert_ne!(
        get_result.exit_code, 0,
        "Should fail - task doesn't exist"
    );
}

#[tokio::test]
async fn test_kanban_task_move() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let executor = CliExecutor::new(&env.temp_dir()).await.unwrap();

    // Setup
    executor
        .execute(&["tool", "kanban", "board", "init", "--name", "Test"])
        .await;
    let add_result = executor
        .execute(&["tool", "kanban", "task", "add", "--title", "Task to Move"])
        .await;
    let task_id = extract_id(&add_result.stdout);

    // Move task to done
    let result = executor
        .execute(&[
            "tool",
            "kanban",
            "task",
            "move",
            "--id",
            &task_id,
            "--column",
            "done",
        ])
        .await;

    assert_eq!(
        result.exit_code, 0,
        "Expected success, got stderr: {}",
        result.stderr
    );

    // Verify task is in done column
    let get_result = executor
        .execute(&["tool", "kanban", "task", "get", "--id", &task_id])
        .await;
    assert!(
        get_result.stdout.contains("done"),
        "Task should be in done column"
    );
}

#[tokio::test]
async fn test_kanban_task_next() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let executor = CliExecutor::new(&env.temp_dir()).await.unwrap();

    // Setup - create multiple tasks
    executor
        .execute(&["tool", "kanban", "board", "init", "--name", "Test"])
        .await;
    executor
        .execute(&["tool", "kanban", "task", "add", "--title", "First Task"])
        .await;
    executor
        .execute(&["tool", "kanban", "task", "add", "--title", "Second Task"])
        .await;

    // Get next task
    let result = executor.execute(&["tool", "kanban", "task", "next"]).await;

    assert_eq!(
        result.exit_code, 0,
        "Expected success, got stderr: {}",
        result.stderr
    );
    // Should return one of the tasks
    assert!(
        result.stdout.contains("Task") || result.stdout.contains("id"),
        "Output should contain a task"
    );
}

#[tokio::test]
async fn test_kanban_tasks_list() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let executor = CliExecutor::new(&env.temp_dir()).await.unwrap();

    // Setup
    executor
        .execute(&["tool", "kanban", "board", "init", "--name", "Test"])
        .await;
    executor
        .execute(&["tool", "kanban", "task", "add", "--title", "Task One"])
        .await;
    executor
        .execute(&["tool", "kanban", "task", "add", "--title", "Task Two"])
        .await;

    // List all tasks
    let result = executor.execute(&["tool", "kanban", "tasks", "list"]).await;

    assert_eq!(
        result.exit_code, 0,
        "Expected success, got stderr: {}",
        result.stderr
    );
    assert!(
        result.stdout.contains("Task One"),
        "Output should contain first task"
    );
    assert!(
        result.stdout.contains("Task Two"),
        "Output should contain second task"
    );
}

#[tokio::test]
async fn test_kanban_tasks_list_with_column_filter() {
    let env = IsolatedTestEnvironment::new().unwrap();
    let executor = CliExecutor::new(&env.temp_dir()).await.unwrap();

    // Setup
    executor
        .execute(&["tool", "kanban", "board", "init", "--name", "Test"])
        .await;
    let add_result = executor
        .execute(&["tool", "kanban", "task", "add", "--title", "Done Task"])
        .await;
    let task_id = extract_id(&add_result.stdout);
    executor
        .execute(&[
            "tool",
            "kanban",
            "task",
            "move",
            "--id",
            &task_id,
            "--column",
            "done",
        ])
        .await;
    executor
        .execute(&["tool", "kanban", "task", "add", "--title", "Todo Task"])
        .await;

    // List only done tasks
    let result = executor
        .execute(&["tool", "kanban", "tasks", "list", "--column", "done"])
        .await;

    assert_eq!(
        result.exit_code, 0,
        "Expected success, got stderr: {}",
        result.stderr
    );
    assert!(
        result.stdout.contains("Done Task"),
        "Output should contain done task"
    );
    assert!(
        !result.stdout.contains("Todo Task"),
        "Output should NOT contain todo task"
    );
}
