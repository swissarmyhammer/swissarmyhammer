//! Integration tests for the entity bag round-trip:
//! add task → list_entities shape → update field → list again → verify change persisted.
//!
//! These tests exercise the same code paths the Tauri commands use,
//! proving that field updates via `UpdateEntityField` are visible
//! in subsequent `EntityContext::list()` calls and that `Entity::to_json()`
//! produces the expected flat bag format.

use serde_json::json;
use swissarmyhammer_kanban::{
    board::InitBoard, entity::UpdateEntityField, task::AddTask, task_helpers::enrich_task_entity,
    KanbanContext, KanbanOperationProcessor, OperationProcessor,
};
use tempfile::TempDir;

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

#[tokio::test]
async fn entity_to_json_has_flat_fields() {
    let (_temp, ctx, processor) = setup().await;

    let result = processor
        .process(&AddTask::new("My Task"), &ctx)
        .await
        .unwrap();
    let task_id = result["id"].as_str().unwrap().to_string();

    // Read the entity back and check to_json() shape
    let ectx = ctx.entity_context().await.unwrap();
    let entity = ectx.read("task", &task_id).await.unwrap();
    let bag = entity.to_json();

    // Flat keys at top level — no nesting
    assert_eq!(bag["entity_type"], "task");
    assert_eq!(bag["id"], task_id);
    assert_eq!(bag["title"], "My Task");
    assert!(bag.get("position_column").is_some());
    // No "fields" wrapper — everything is flat
    assert!(bag.get("fields").is_none());
}

#[tokio::test]
async fn update_field_persists_title_change() {
    let (_temp, ctx, processor) = setup().await;

    // Add a task
    let result = processor
        .process(&AddTask::new("Original"), &ctx)
        .await
        .unwrap();
    let task_id = result["id"].as_str().unwrap().to_string();

    // Update the title via UpdateEntityField (same path as Tauri command)
    let update = UpdateEntityField::new("task", &task_id, "title", json!("Changed"));
    processor.process(&update, &ctx).await.unwrap();

    // Read back via list and verify
    let ectx = ctx.entity_context().await.unwrap();
    let tasks = ectx.list("task").await.unwrap();
    let task = tasks.iter().find(|t| t.id == task_id).unwrap();
    assert_eq!(task.get_str("title"), Some("Changed"));

    // Also verify via to_json() (the wire format)
    let bag = task.to_json();
    assert_eq!(bag["title"], "Changed");
}

#[tokio::test]
async fn update_field_persists_body_change() {
    let (_temp, ctx, processor) = setup().await;

    let result = processor
        .process(&AddTask::new("Task").with_description("old body"), &ctx)
        .await
        .unwrap();
    let task_id = result["id"].as_str().unwrap().to_string();

    // Update body
    let update = UpdateEntityField::new("task", &task_id, "body", json!("new body with #tag"));
    processor.process(&update, &ctx).await.unwrap();

    // Verify body persisted
    let ectx = ctx.entity_context().await.unwrap();
    let entity = ectx.read("task", &task_id).await.unwrap();
    assert_eq!(entity.get_str("body"), Some("new body with #tag"));

    // Verify computed tags got re-derived
    let tags = entity.get_string_list("tags");
    assert!(
        tags.contains(&"tag".to_string()),
        "tags should contain 'tag' after body update"
    );
}

#[tokio::test]
async fn enriched_entity_has_computed_fields() {
    let (_temp, ctx, processor) = setup().await;

    // Add two tasks, one depending on the other
    let r1 = processor
        .process(&AddTask::new("Blocker"), &ctx)
        .await
        .unwrap();
    let blocker_id = r1["id"].as_str().unwrap().to_string();

    let r2 = processor
        .process(&AddTask::new("Blocked"), &ctx)
        .await
        .unwrap();
    let blocked_id = r2["id"].as_str().unwrap().to_string();

    // Set dependency
    let update = UpdateEntityField::new("task", &blocked_id, "depends_on", json!([blocker_id]));
    processor.process(&update, &ctx).await.unwrap();

    // Read all tasks and enrich
    let ectx = ctx.entity_context().await.unwrap();
    let mut tasks = ectx.list("task").await.unwrap();
    let all_tasks = tasks.clone();
    for t in &mut tasks {
        enrich_task_entity(t, &all_tasks, "done");
    }

    // Verify the blocked task has ready=false
    let blocked = tasks.iter().find(|t| t.id == blocked_id).unwrap();
    let bag = blocked.to_json();
    assert_eq!(bag["ready"], false);
    assert_eq!(bag["blocked_by"], json!([blocker_id]));

    // Verify the blocker has ready=true and blocks the other
    let blocker = tasks.iter().find(|t| t.id == blocker_id).unwrap();
    let bbag = blocker.to_json();
    assert_eq!(bbag["ready"], true);
    assert_eq!(bbag["blocks"], json!([blocked_id]));
}
