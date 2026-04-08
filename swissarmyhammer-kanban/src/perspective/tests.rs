//! Integration tests for perspective CRUD operations.

use crate::board::InitBoard;
use crate::context::KanbanContext;
use crate::perspective::{
    AddPerspective, DeletePerspective, GetPerspective, ListPerspectives, PerspectiveFieldEntry,
    RenamePerspective, SortDirection, SortEntry, UpdatePerspective,
};
use swissarmyhammer_operations::Execute;
use tempfile::TempDir;

/// Create a temp KanbanContext with an initialized board.
async fn setup() -> (TempDir, KanbanContext) {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(kanban_dir);

    InitBoard::new("Test")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    (temp, ctx)
}

/// Create a temp KanbanContext with the perspective StoreHandle wired in.
async fn setup_with_store() -> (
    TempDir,
    KanbanContext,
    std::sync::Arc<
        swissarmyhammer_store::StoreHandle<swissarmyhammer_perspectives::PerspectiveStore>,
    >,
) {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(kanban_dir.clone());

    InitBoard::new("Test")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    // Create and wire in a StoreHandle for perspectives.
    let perspectives_dir = kanban_dir.join("perspectives");
    std::fs::create_dir_all(&perspectives_dir).unwrap();
    let store = std::sync::Arc::new(swissarmyhammer_perspectives::PerspectiveStore::new(
        &perspectives_dir,
    ));
    let handle = std::sync::Arc::new(swissarmyhammer_store::StoreHandle::new(store));

    {
        let pctx = ctx.perspective_context().await.unwrap();
        pctx.write()
            .await
            .set_store_handle(std::sync::Arc::clone(&handle));
    }

    (temp, ctx, handle)
}

#[tokio::test]
async fn test_add_perspective() {
    let (_temp, ctx) = setup().await;

    let result = AddPerspective::new("Sprint Board", "board")
        .with_filter("(e) => e.Status !== \"Done\"")
        .with_group("(e) => e.Status")
        .with_fields(vec![PerspectiveFieldEntry::new(
            "01JMTASK0000000000TITLE00",
        )
        .with_caption("Title")
        .with_width(200)])
        .with_sort(vec![SortEntry::new(
            "01JMTASK0000000000PRIORTY",
            SortDirection::Asc,
        )])
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    assert_eq!(result["name"], "Sprint Board");
    assert_eq!(result["view"], "board");
    assert!(!result["id"].as_str().unwrap().is_empty());
    assert_eq!(result["fields"].as_array().unwrap().len(), 1);
    assert_eq!(result["fields"][0]["caption"], "Title");
    assert!(result["filter"].as_str().unwrap().contains("Done"));
    assert_eq!(result["sort"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_add_perspective_minimal() {
    let (_temp, ctx) = setup().await;

    let result = AddPerspective::new("Default", "grid")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    assert_eq!(result["name"], "Default");
    assert_eq!(result["view"], "grid");
    assert!(!result["id"].as_str().unwrap().is_empty());
    // Optional fields should be empty/null
    assert!(result["fields"].as_array().is_none_or(|a| a.is_empty()));
    assert!(result["filter"].is_null());
    assert!(result["group"].is_null());
    assert!(result["sort"].as_array().is_none_or(|a| a.is_empty()));
}

#[tokio::test]
async fn test_get_by_id() {
    let (_temp, ctx) = setup().await;

    let add_result = AddPerspective::new("My View", "board")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    let id = add_result["id"].as_str().unwrap().to_string();

    let get_result = GetPerspective::new(&id)
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    assert_eq!(get_result["id"], id);
    assert_eq!(get_result["name"], "My View");
    assert_eq!(get_result["view"], "board");
}

#[tokio::test]
async fn test_get_by_name() {
    let (_temp, ctx) = setup().await;

    let add_result = AddPerspective::new("Named View", "grid")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    let id = add_result["id"].as_str().unwrap().to_string();

    let get_result = GetPerspective::new("Named View")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    assert_eq!(get_result["id"], id);
    assert_eq!(get_result["name"], "Named View");
}

#[tokio::test]
async fn test_get_not_found() {
    let (_temp, ctx) = setup().await;

    let result = GetPerspective::new("nonexistent")
        .execute(&ctx)
        .await
        .into_result();

    assert!(
        result.is_err(),
        "Expected error for nonexistent perspective"
    );
}

#[tokio::test]
async fn test_list_empty() {
    let (_temp, ctx) = setup().await;

    let result = ListPerspectives::new()
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    assert_eq!(result["count"], 0);
    assert!(result["perspectives"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_list_multiple() {
    let (_temp, ctx) = setup().await;

    AddPerspective::new("View A", "board")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    AddPerspective::new("View B", "grid")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    AddPerspective::new("View C", "list")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    let result = ListPerspectives::new()
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    assert_eq!(result["count"], 3);
    assert_eq!(result["perspectives"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn test_update_partial() {
    let (_temp, ctx) = setup().await;

    let add_result = AddPerspective::new("Original", "board")
        .with_filter("(e) => true")
        .with_group("(e) => e.Status")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    let id = add_result["id"].as_str().unwrap().to_string();

    // Update only name -- view, filter, group should be preserved
    let update_result = UpdatePerspective::new(&id)
        .with_name("Renamed")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    assert_eq!(update_result["name"], "Renamed");
    assert_eq!(update_result["view"], "board"); // preserved
    assert_eq!(update_result["filter"], "(e) => true"); // preserved
    assert_eq!(update_result["group"], "(e) => e.Status"); // preserved

    // Update only view -- name should be preserved
    let update_result = UpdatePerspective::new(&id)
        .with_view("grid")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    assert_eq!(update_result["name"], "Renamed"); // preserved
    assert_eq!(update_result["view"], "grid"); // updated

    // Clear filter by setting to None
    let update_result = UpdatePerspective::new(&id)
        .with_filter(None)
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    assert!(update_result["filter"].is_null()); // cleared
    assert_eq!(update_result["group"], "(e) => e.Status"); // preserved
}

#[tokio::test]
async fn test_delete_perspective() {
    let (_temp, ctx) = setup().await;

    let add_result = AddPerspective::new("Doomed", "board")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    let id = add_result["id"].as_str().unwrap().to_string();

    let delete_result = DeletePerspective::new(&id)
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    assert_eq!(delete_result["deleted"], true);
    assert_eq!(delete_result["id"], id);
    assert_eq!(delete_result["name"], "Doomed");

    // Verify it's gone
    let get_result = GetPerspective::new(&id).execute(&ctx).await.into_result();
    assert!(
        get_result.is_err(),
        "Deleted perspective should not be found"
    );
}

#[tokio::test]
async fn test_delete_not_found() {
    let (_temp, ctx) = setup().await;

    let result = DeletePerspective::new("01ZZZZZZZZZZZZZZZZZZZZZZZZ")
        .execute(&ctx)
        .await
        .into_result();

    assert!(
        result.is_err(),
        "Expected error for nonexistent perspective"
    );
}

#[tokio::test]
async fn test_add_duplicate_name_allowed() {
    let (_temp, ctx) = setup().await;

    AddPerspective::new("Sprint Board", "board")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    // Same name, different ID — must succeed (multiple perspectives per view)
    AddPerspective::new("Sprint Board", "grid")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    let list = ListPerspectives::new()
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    assert_eq!(list["count"], 2);
}

// =========================================================================
// Event propagation tests — prove the store → flush_all() → event pipeline
// =========================================================================

#[tokio::test]
async fn test_add_perspective_emits_item_created_event() {
    let (_temp, ctx, handle) = setup_with_store().await;

    let result = AddPerspective::new("Event Test", "board")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    let id = result["id"].as_str().unwrap();

    let events = handle.flush_changes().await;
    assert_eq!(events.len(), 1, "expected exactly one event");
    assert_eq!(events[0].event_name(), "item-created");
    assert_eq!(events[0].payload()["store"], "perspective");
    assert_eq!(events[0].payload()["id"], id);
}

#[tokio::test]
async fn test_update_perspective_emits_item_changed_event() {
    let (_temp, ctx, handle) = setup_with_store().await;

    let result = AddPerspective::new("Before Update", "board")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    let id = result["id"].as_str().unwrap().to_string();

    // Drain the create event.
    handle.flush_changes().await;

    UpdatePerspective::new(&id)
        .with_name("After Update")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    let events = handle.flush_changes().await;
    assert_eq!(events.len(), 1, "expected exactly one event");
    assert_eq!(events[0].event_name(), "item-changed");
    assert_eq!(events[0].payload()["store"], "perspective");
    assert_eq!(events[0].payload()["id"], id.as_str());
}

#[tokio::test]
async fn test_delete_perspective_emits_item_removed_event() {
    let (_temp, ctx, handle) = setup_with_store().await;

    let result = AddPerspective::new("Doomed", "board")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    let id = result["id"].as_str().unwrap().to_string();

    // Drain the create event.
    handle.flush_changes().await;

    DeletePerspective::new(&id)
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    let events = handle.flush_changes().await;
    assert_eq!(events.len(), 1, "expected exactly one event");
    assert_eq!(events[0].event_name(), "item-removed");
    assert_eq!(events[0].payload()["store"], "perspective");
    assert_eq!(events[0].payload()["id"], id.as_str());
}

// =========================================================================
// Rename tests
// =========================================================================

#[tokio::test]
async fn test_rename_perspective() {
    let (_temp, ctx) = setup().await;

    let add_result = AddPerspective::new("Original Name", "board")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    let id = add_result["id"].as_str().unwrap().to_string();

    let rename_result = RenamePerspective::new(&id, "New Name")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    assert_eq!(rename_result["id"], id);
    assert_eq!(rename_result["name"], "New Name");
    assert_eq!(rename_result["view"], "board");

    // Verify via get that the rename persisted
    let get_result = GetPerspective::new(&id)
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    assert_eq!(get_result["name"], "New Name");
}

#[tokio::test]
async fn test_rename_preserves_other_fields() {
    let (_temp, ctx) = setup().await;

    let add_result = AddPerspective::new("With Filter", "grid")
        .with_filter("(e) => e.Status !== \"Done\"")
        .with_group("(e) => e.Assignee")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    let id = add_result["id"].as_str().unwrap().to_string();

    let rename_result = RenamePerspective::new(&id, "Renamed")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    assert_eq!(rename_result["name"], "Renamed");
    assert_eq!(rename_result["view"], "grid");
    assert_eq!(rename_result["filter"], "(e) => e.Status !== \"Done\"");
    assert_eq!(rename_result["group"], "(e) => e.Assignee");
}

#[tokio::test]
async fn test_rename_nonexistent_perspective_fails() {
    let (_temp, ctx) = setup().await;

    let result = RenamePerspective::new("01ZZZZZZZZZZZZZZZZZZZZZZZZ", "New")
        .execute(&ctx)
        .await
        .into_result();

    assert!(
        result.is_err(),
        "Rename of nonexistent perspective should fail"
    );
}

#[tokio::test]
async fn test_rename_is_atomic_single_operation() {
    let (_temp, ctx) = setup().await;

    let add_result = AddPerspective::new("Before", "board")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    let id = add_result["id"].as_str().unwrap().to_string();

    RenamePerspective::new(&id, "After")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    // The perspective count should remain 1 (no duplicate created)
    let list = ListPerspectives::new()
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    assert_eq!(list["count"], 1);
    assert_eq!(list["perspectives"][0]["name"], "After");
    assert_eq!(list["perspectives"][0]["id"], id);
}
