//! Integration tests for activity logging

use swissarmyhammer_kanban::{
    board::InitBoard,
    task::{AddTask, GetTask, UpdateTask},
    KanbanContext, KanbanOperationProcessor, OperationProcessor,
};
use tempfile::TempDir;

#[tokio::test]
async fn test_activity_logging_end_to_end() {
    // Setup
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);

    let processor = KanbanOperationProcessor::with_actor("test-user[session123]");

    // Initialize board (logged)
    processor
        .process(&InitBoard::new("Test Board"), &ctx)
        .await
        .unwrap();

    // Add a task (logged)
    let result = processor
        .process(&AddTask::new("First task").with_description("Test task"), &ctx)
        .await
        .unwrap();
    let task_id = result["id"].as_str().unwrap().to_string();

    // Update the task (logged)
    processor
        .process(
            &UpdateTask::new(task_id.as_str()).with_title("Updated task"),
            &ctx,
        )
        .await
        .unwrap();

    // Get task (unlogged - should not add to activity log)
    processor
        .process(&GetTask::new(task_id.as_str()), &ctx)
        .await
        .unwrap();

    // Verify activity log
    let entries = ctx.read_activity(None).await.unwrap();
    assert_eq!(entries.len(), 3); // InitBoard, AddTask, UpdateTask (not GetTask)
    assert_eq!(entries[0].op, "update task"); // Newest first
    assert_eq!(entries[1].op, "add task");
    assert_eq!(entries[2].op, "init board"); // Oldest last

    // Verify actor attribution
    assert_eq!(entries[0].actor, Some("test-user[session123]".to_string()));
    assert_eq!(entries[1].actor, Some("test-user[session123]".to_string()));
    assert_eq!(entries[2].actor, Some("test-user[session123]".to_string()));

    // Verify per-task log
    let task_id_type = swissarmyhammer_kanban::types::TaskId::from_string(&task_id);
    let task_log_path = ctx.task_log_path(&task_id_type);
    let task_log = std::fs::read_to_string(&task_log_path).unwrap();
    let lines: Vec<&str> = task_log.lines().collect();

    assert_eq!(lines.len(), 2); // AddTask + UpdateTask (not GetTask)

    // Parse entries
    let entry1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let entry2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();

    assert_eq!(entry1["op"], "add task");
    assert_eq!(entry2["op"], "update task");
    assert_eq!(entry1["actor"], "test-user[session123]");
    assert_eq!(entry2["actor"], "test-user[session123]");

    // Verify activity log file exists
    let activity_path = ctx.activity_path();
    assert!(
        activity_path.exists(),
        "Activity log file should exist at {:?}",
        activity_path
    );

    // Verify task log file exists
    assert!(
        task_log_path.exists(),
        "Task log file should exist at {:?}",
        task_log_path
    );

}

#[tokio::test]
async fn test_unlogged_operations_dont_create_logs() {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);

    let processor = KanbanOperationProcessor::new();

    // Initialize board
    processor
        .process(&InitBoard::new("Test"), &ctx)
        .await
        .unwrap();

    // Add task
    let result = processor
        .process(&AddTask::new("Task"), &ctx)
        .await
        .unwrap();
    let task_id = result["id"].as_str().unwrap();

    // Verify 2 entries (init + add)
    let entries_before = ctx.read_activity(None).await.unwrap();
    assert_eq!(entries_before.len(), 2);

    // Perform read operations
    processor
        .process(&GetTask::new(task_id), &ctx)
        .await
        .unwrap();

    use swissarmyhammer_kanban::task::ListTasks;
    processor.process(&ListTasks::new(), &ctx).await.unwrap();

    use swissarmyhammer_kanban::board::GetBoard;
    processor.process(&GetBoard, &ctx).await.unwrap();

    // Verify still only 2 entries
    let entries_after = ctx.read_activity(None).await.unwrap();
    assert_eq!(entries_after.len(), 2);

}

#[tokio::test]
async fn test_error_logging() {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);

    let processor = KanbanOperationProcessor::new();

    // Initialize board
    processor
        .process(&InitBoard::new("Test"), &ctx)
        .await
        .unwrap();

    // Try to update a non-existent task (should fail and log)
    let result = processor
        .process(
            &UpdateTask::new("nonexistent").with_title("Updated"),
            &ctx,
        )
        .await;

    assert!(result.is_err());

    // Verify error was logged
    let entries = ctx.read_activity(None).await.unwrap();
    assert_eq!(entries.len(), 2); // InitBoard + failed UpdateTask
    assert_eq!(entries[0].op, "update task");
    assert!(entries[0].output["error"].as_str().is_some());

    // Verify error details in log file
    let activity_path = ctx.activity_path();
    let log_content = std::fs::read_to_string(activity_path).unwrap();
    let lines: Vec<&str> = log_content.lines().collect();
    assert_eq!(lines.len(), 2);

    let error_entry: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(error_entry["op"], "update task");
    assert!(error_entry["output"]["error"]
        .as_str()
        .unwrap()
        .contains("not found"));
}

#[tokio::test]
async fn test_actor_from_operation_context() {
    use swissarmyhammer_kanban::parse::parse_input;

    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);

    // Initialize without actor
    let init_op = parse_input(serde_json::json!({
        "op": "init board",
        "name": "Test"
    }))
    .unwrap();

    let processor = match &init_op[0].actor {
        Some(actor) => KanbanOperationProcessor::with_actor(actor.to_string()),
        None => KanbanOperationProcessor::new(),
    };

    processor
        .process(&InitBoard::new("Test"), &ctx)
        .await
        .unwrap();

    // Add task with actor in JSON
    let add_op = parse_input(serde_json::json!({
        "op": "add task",
        "title": "Test Task",
        "actor": "alice"
    }))
    .unwrap();

    assert_eq!(
        add_op[0].actor,
        Some(swissarmyhammer_kanban::types::ActorId::from_string("alice"))
    );

    // Create processor with actor from operation
    let processor = match &add_op[0].actor {
        Some(actor) => KanbanOperationProcessor::with_actor(actor.to_string()),
        None => KanbanOperationProcessor::new(),
    };

    processor
        .process(&AddTask::new("Test Task"), &ctx)
        .await
        .unwrap();

    // Verify actor in activity log
    let entries = ctx.read_activity(None).await.unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].op, "add task");
    assert_eq!(entries[0].actor, Some("alice".to_string()));
    assert_eq!(entries[1].op, "init board");
    assert_eq!(entries[1].actor, None);
}

