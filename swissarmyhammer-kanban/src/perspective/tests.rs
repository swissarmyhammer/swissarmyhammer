//! Integration tests for perspective CRUD operations.

use crate::board::InitBoard;
use crate::context::KanbanContext;
use crate::perspective::{
    AddPerspective, DeletePerspective, GetPerspective, ListPerspectives, PerspectiveFieldEntry,
    SortDirection, SortEntry, UpdatePerspective,
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

#[tokio::test]
async fn test_add_perspective() {
    let (_temp, ctx) = setup().await;

    let result = AddPerspective::new("Sprint Board", "board")
        .with_filter("(e) => e.Status !== \"Done\"")
        .with_group("(e) => e.Status")
        .with_fields(vec![PerspectiveFieldEntry {
            field: "01JMTASK0000000000TITLE00".to_string(),
            caption: Some("Title".to_string()),
            width: Some(200),
            editor: None,
            display: None,
            sort_comparator: None,
        }])
        .with_sort(vec![SortEntry {
            field: "01JMTASK0000000000PRIORTY".to_string(),
            direction: SortDirection::Asc,
        }])
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    assert_eq!(result["name"], "Sprint Board");
    assert_eq!(result["view"], "board");
    assert!(result["id"].as_str().unwrap().len() > 0);
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
    assert!(result["id"].as_str().unwrap().len() > 0);
    // Optional fields should be empty/null
    assert!(
        result["fields"].as_array().map_or(true, |a| a.is_empty())
    );
    assert!(result["filter"].is_null());
    assert!(result["group"].is_null());
    assert!(
        result["sort"].as_array().map_or(true, |a| a.is_empty())
    );
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

    assert!(result.is_err(), "Expected error for nonexistent perspective");
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
    let get_result = GetPerspective::new(&id)
        .execute(&ctx)
        .await
        .into_result();
    assert!(get_result.is_err(), "Deleted perspective should not be found");
}

#[tokio::test]
async fn test_delete_not_found() {
    let (_temp, ctx) = setup().await;

    let result = DeletePerspective::new("01ZZZZZZZZZZZZZZZZZZZZZZZZ")
        .execute(&ctx)
        .await
        .into_result();

    assert!(result.is_err(), "Expected error for nonexistent perspective");
}

#[tokio::test]
async fn test_add_logs_to_changelog() {
    let (_temp, ctx) = setup().await;

    AddPerspective::new("Logged View", "board")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    let entries = ctx.perspective_changelog().read_all().await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].op,
        crate::perspective::PerspectiveChangeOp::Create
    );
}

#[tokio::test]
async fn test_update_logs_to_changelog() {
    let (_temp, ctx) = setup().await;

    let add_result = AddPerspective::new("Before", "board")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    let id = add_result["id"].as_str().unwrap().to_string();

    UpdatePerspective::new(&id)
        .with_name("After")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    let entries = ctx.perspective_changelog().read_all().await.unwrap();
    assert_eq!(entries.len(), 2); // create + update
    assert_eq!(
        entries[1].op,
        crate::perspective::PerspectiveChangeOp::Update
    );
    assert_eq!(entries[1].previous.as_ref().unwrap()["name"], "Before");
    assert_eq!(entries[1].current.as_ref().unwrap()["name"], "After");
}

#[tokio::test]
async fn test_delete_logs_to_changelog() {
    let (_temp, ctx) = setup().await;

    let add_result = AddPerspective::new("Will Delete", "board")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    let id = add_result["id"].as_str().unwrap().to_string();

    DeletePerspective::new(&id)
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    let entries = ctx.perspective_changelog().read_all().await.unwrap();
    assert_eq!(entries.len(), 2); // create + delete
    assert_eq!(
        entries[1].op,
        crate::perspective::PerspectiveChangeOp::Delete
    );
    assert_eq!(entries[1].previous.as_ref().unwrap()["name"], "Will Delete");
    assert!(entries[1].current.is_none());
}

#[tokio::test]
async fn test_add_duplicate_name_rejected() {
    let (_temp, ctx) = setup().await;

    AddPerspective::new("Sprint Board", "board")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    // Adding a second perspective with the same name should fail
    let result = AddPerspective::new("Sprint Board", "grid")
        .execute(&ctx)
        .await
        .into_result();

    assert!(result.is_err(), "Expected error for duplicate perspective name");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("Sprint Board") && err_msg.contains("already exists"),
        "Error should mention the duplicate name, got: {err_msg}"
    );

    // Only one perspective should exist
    let list = ListPerspectives::new()
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    assert_eq!(list["count"], 1);
}

#[tokio::test]
async fn test_update_rename_to_duplicate_rejected() {
    let (_temp, ctx) = setup().await;

    AddPerspective::new("View A", "board")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    let b_result = AddPerspective::new("View B", "grid")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    let b_id = b_result["id"].as_str().unwrap().to_string();

    // Renaming View B to View A should fail
    let result = UpdatePerspective::new(&b_id)
        .with_name("View A")
        .execute(&ctx)
        .await
        .into_result();

    assert!(result.is_err(), "Expected error when renaming to an existing name");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("View A") && err_msg.contains("already exists"),
        "Error should mention the duplicate name, got: {err_msg}"
    );

    // View B should still have its original name
    let get_result = GetPerspective::new(&b_id)
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    assert_eq!(get_result["name"], "View B");
}
