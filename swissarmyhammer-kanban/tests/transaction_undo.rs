//! Integration tests for transaction-level undo/redo.
//!
//! Verifies that compound operations (tag rename, add-task with auto-tag-creation)
//! can be undone and redone as a single unit via the transaction ULID returned
//! as `operation_id` in the processor result.

use swissarmyhammer_kanban::{
    board::InitBoard,
    tag::{AddTag, DeleteTag, UpdateTag},
    task::{AddTask, UpdateTask},
    KanbanContext, KanbanOperationProcessor, OperationProcessor,
};
use tempfile::TempDir;

/// Set up a board via the processor so every operation gets a transaction.
async fn setup() -> (TempDir, KanbanContext, KanbanOperationProcessor) {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);
    let processor = KanbanOperationProcessor::new();

    processor
        .process(&InitBoard::new("Test Board"), &ctx)
        .await
        .unwrap();

    (temp, ctx, processor)
}

// ---------------------------------------------------------------------------
// Tag rename — the heaviest transaction
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tag_rename_undo_restores_all_tasks() {
    let (_temp, ctx, processor) = setup().await;

    // Create a tag named "frontend"
    let tag_result = processor
        .process(&AddTag::new("frontend"), &ctx)
        .await
        .unwrap();
    let tag_id = tag_result["id"].as_str().unwrap().to_string();

    // Create 3 tasks whose body contains #frontend
    let mut task_ids = Vec::new();
    for i in 1..=3 {
        let add = AddTask::new(format!("Task {}", i))
            .with_description(format!("Work on #frontend component {}", i));
        let result = processor.process(&add, &ctx).await.unwrap();
        task_ids.push(result["id"].as_str().unwrap().to_string());
    }

    // Rename the tag from "frontend" to "fe"
    let rename_result = processor
        .process(&UpdateTag::new(tag_id.clone()).with_name("fe"), &ctx)
        .await
        .unwrap();
    let operation_id = rename_result["operation_id"]
        .as_str()
        .expect("result should contain operation_id")
        .to_string();

    // Verify all tasks now have #fe in their body
    let ectx = ctx.entity_context().await.unwrap();
    for tid in &task_ids {
        let task = ectx.read("task", tid).await.unwrap();
        let body = task.get_str("body").unwrap_or("");
        assert!(
            body.contains("#fe"),
            "Task {} should contain #fe, got: {}",
            tid,
            body
        );
        assert!(
            !body.contains("#frontend"),
            "Task {} should NOT contain #frontend, got: {}",
            tid,
            body
        );
    }

    // Verify tag name is "fe"
    let tag = ectx.read("tag", &tag_id).await.unwrap();
    assert_eq!(tag.get_str("tag_name"), Some("fe"));

    // Undo the entire transaction
    ectx.undo(&operation_id).await.unwrap();

    // Verify all 3 tasks have #frontend again
    for tid in &task_ids {
        let task = ectx.read("task", tid).await.unwrap();
        let body = task.get_str("body").unwrap_or("");
        assert!(
            body.contains("#frontend"),
            "After undo, task {} should contain #frontend, got: {}",
            tid,
            body
        );
        assert!(
            !body.contains("#fe"),
            "After undo, task {} should NOT contain #fe, got: {}",
            tid,
            body
        );
    }

    // Verify tag name is back to "frontend"
    let tag = ectx.read("tag", &tag_id).await.unwrap();
    assert_eq!(tag.get_str("tag_name"), Some("frontend"));
}

#[tokio::test]
async fn test_tag_rename_undo_then_redo() {
    let (_temp, ctx, processor) = setup().await;

    // Create tag + tasks
    let tag_result = processor
        .process(&AddTag::new("frontend"), &ctx)
        .await
        .unwrap();
    let tag_id = tag_result["id"].as_str().unwrap().to_string();

    let mut task_ids = Vec::new();
    for i in 1..=3 {
        let add = AddTask::new(format!("Task {}", i))
            .with_description(format!("Work on #frontend part {}", i));
        let result = processor.process(&add, &ctx).await.unwrap();
        task_ids.push(result["id"].as_str().unwrap().to_string());
    }

    // Rename
    let rename_result = processor
        .process(&UpdateTag::new(tag_id.clone()).with_name("fe"), &ctx)
        .await
        .unwrap();
    let operation_id = rename_result["operation_id"].as_str().unwrap().to_string();

    let ectx = ctx.entity_context().await.unwrap();

    // Undo
    ectx.undo(&operation_id).await.unwrap();

    // Verify undo worked (all tasks have #frontend, tag name is "frontend")
    for tid in &task_ids {
        let task = ectx.read("task", tid).await.unwrap();
        assert!(task.get_str("body").unwrap().contains("#frontend"));
    }
    let tag = ectx.read("tag", &tag_id).await.unwrap();
    assert_eq!(tag.get_str("tag_name"), Some("frontend"));

    // Redo
    ectx.redo(&operation_id).await.unwrap();

    // Verify redo restored the rename (all tasks have #fe, tag name is "fe")
    for tid in &task_ids {
        let task = ectx.read("task", tid).await.unwrap();
        let body = task.get_str("body").unwrap();
        assert!(
            body.contains("#fe"),
            "After redo, task {} should have #fe, got: {}",
            tid,
            body
        );
        assert!(
            !body.contains("#frontend"),
            "After redo, task {} should NOT have #frontend, got: {}",
            tid,
            body
        );
    }
    let tag = ectx.read("tag", &tag_id).await.unwrap();
    assert_eq!(tag.get_str("tag_name"), Some("fe"));
}

// ---------------------------------------------------------------------------
// DeleteTag — compound operation (remove #tag from bodies + delete tag entity)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_delete_tag_undo_restores_tag_and_task_bodies() {
    let (_temp, ctx, processor) = setup().await;

    // Create a tag named "bug"
    let tag_result = processor.process(&AddTag::new("bug"), &ctx).await.unwrap();
    let tag_id = tag_result["id"].as_str().unwrap().to_string();

    // Create 2 tasks whose body contains #bug
    let mut task_ids = Vec::new();
    for i in 1..=2 {
        let add =
            AddTask::new(format!("Task {}", i)).with_description(format!("Fix #bug number {}", i));
        let result = processor.process(&add, &ctx).await.unwrap();
        task_ids.push(result["id"].as_str().unwrap().to_string());
    }

    // Verify tasks have #bug in their body before delete
    let ectx = ctx.entity_context().await.unwrap();
    for tid in &task_ids {
        let task = ectx.read("task", tid).await.unwrap();
        assert!(task.get_str("body").unwrap().contains("#bug"));
    }

    // Delete the tag through the processor
    let delete_result = processor
        .process(&DeleteTag::new(tag_id.clone()), &ctx)
        .await
        .unwrap();
    let operation_id = delete_result["operation_id"]
        .as_str()
        .expect("result should contain operation_id")
        .to_string();

    // Verify tag is gone and #bug removed from bodies
    assert!(ectx.read("tag", &tag_id).await.is_err());
    for tid in &task_ids {
        let task = ectx.read("task", tid).await.unwrap();
        assert!(
            !task.get_str("body").unwrap().contains("#bug"),
            "After delete, body should not contain #bug"
        );
    }

    // Undo the entire delete transaction
    ectx.undo(&operation_id).await.unwrap();

    // Verify tag is restored
    let tag = ectx.read("tag", &tag_id).await.unwrap();
    assert_eq!(tag.get_str("tag_name"), Some("bug"));

    // Verify task bodies have #bug back
    for tid in &task_ids {
        let task = ectx.read("task", tid).await.unwrap();
        let body = task.get_str("body").unwrap();
        assert!(
            body.contains("#bug"),
            "After undo, task {} body should contain #bug, got: {}",
            tid,
            body
        );
    }
}

// ---------------------------------------------------------------------------
// Simple entity update through processor
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_simple_update_undo_via_operation_id() {
    let (_temp, ctx, processor) = setup().await;

    // Create a task
    let add_result = processor
        .process(&AddTask::new("Original title"), &ctx)
        .await
        .unwrap();
    let task_id = add_result["id"].as_str().unwrap().to_string();

    // Update the task title through the processor
    let update_result = processor
        .process(
            &UpdateTask::new(task_id.as_str()).with_title("Updated title"),
            &ctx,
        )
        .await
        .unwrap();
    let operation_id = update_result["operation_id"]
        .as_str()
        .expect("result should contain operation_id")
        .to_string();

    // Verify the title changed
    let ectx = ctx.entity_context().await.unwrap();
    let task = ectx.read("task", &task_id).await.unwrap();
    assert_eq!(task.get_str("title"), Some("Updated title"));

    // Undo the update via the operation_id (transaction ULID)
    ectx.undo(&operation_id).await.unwrap();

    // Verify the title is restored
    let task = ectx.read("task", &task_id).await.unwrap();
    assert_eq!(task.get_str("title"), Some("Original title"));
}

#[tokio::test]
async fn test_operation_id_present_in_all_processor_results() {
    let (_temp, ctx, processor) = setup().await;

    // AddTask should have operation_id
    let add_result = processor
        .process(&AddTask::new("Task"), &ctx)
        .await
        .unwrap();
    assert!(
        add_result["operation_id"].is_string(),
        "AddTask result should have operation_id"
    );

    // AddTag should have operation_id
    let tag_result = processor
        .process(&AddTag::new("label"), &ctx)
        .await
        .unwrap();
    assert!(
        tag_result["operation_id"].is_string(),
        "AddTag result should have operation_id"
    );

    // InitBoard result also gets it (from the setup)
    // We just verify the other operations since InitBoard was already called
}
