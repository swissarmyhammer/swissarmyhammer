//! The perspective operations.
//!
//! These tests hold the add, get, list, update and delete operations, the full
//! lifecycle, and the errors for malformed `fields` and `sort` JSON.

use super::*;

// ------------------------------------------------------------------

// ── Perspective operations ─────────────────────────────────────

#[tokio::test]
async fn dispatch_add_perspective() {
    let (_temp, ctx) = setup().await;

    let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("name".into(), json!("Sprint View"));
        m.insert("view".into(), json!("board"));
        m
    });
    let result = execute_operation(&ctx, &op).await.unwrap();
    assert_eq!(result["name"], "Sprint View");
    assert_eq!(result["view"], "board");
    assert!(result["id"].as_str().is_some());
}

#[tokio::test]
async fn dispatch_get_perspective() {
    let (_temp, ctx) = setup().await;

    // Add a perspective first
    let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("name".into(), json!("My View"));
        m.insert("view".into(), json!("grid"));
        m
    });
    let added = execute_operation(&ctx, &op).await.unwrap();
    let id = added["id"].as_str().unwrap().to_string();

    // Get by ID
    let op = KanbanOperation::new(Verb::Get, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("id".into(), json!(id));
        m
    });
    let result = execute_operation(&ctx, &op).await.unwrap();
    assert_eq!(result["name"], "My View");
    assert_eq!(result["view"], "grid");
}

#[tokio::test]
async fn dispatch_list_perspectives() {
    let (_temp, ctx) = setup().await;

    // Add two perspectives
    for name in &["View A", "View B"] {
        let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
            let mut m = serde_json::Map::new();
            m.insert("name".into(), json!(name));
            m.insert("view".into(), json!("board"));
            m
        });
        execute_operation(&ctx, &op).await.unwrap();
    }

    // List all
    let op = KanbanOperation::new(Verb::List, Noun::Perspectives, serde_json::Map::new());
    let result = execute_operation(&ctx, &op).await.unwrap();
    assert_eq!(result["count"], 2);
    let perspectives = result["perspectives"].as_array().unwrap();
    assert_eq!(perspectives.len(), 2);
}

#[tokio::test]
async fn dispatch_update_perspective() {
    let (_temp, ctx) = setup().await;

    // Add a perspective
    let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("name".into(), json!("Old Name"));
        m.insert("view".into(), json!("board"));
        m
    });
    let added = execute_operation(&ctx, &op).await.unwrap();
    let id = added["id"].as_str().unwrap().to_string();

    // Update the name
    let op = KanbanOperation::new(Verb::Update, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("id".into(), json!(id));
        m.insert("name".into(), json!("New Name"));
        m.insert("view".into(), json!("grid"));
        m
    });
    let result = execute_operation(&ctx, &op).await.unwrap();
    assert_eq!(result["name"], "New Name");
    assert_eq!(result["view"], "grid");
}

#[tokio::test]
async fn dispatch_delete_perspective() {
    let (_temp, ctx) = setup().await;

    // Add a perspective
    let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("name".into(), json!("Doomed"));
        m.insert("view".into(), json!("board"));
        m
    });
    let added = execute_operation(&ctx, &op).await.unwrap();
    let id = added["id"].as_str().unwrap().to_string();

    // Delete it
    let op = KanbanOperation::new(Verb::Delete, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("id".into(), json!(id));
        m
    });
    let result = execute_operation(&ctx, &op).await.unwrap();
    assert_eq!(result["deleted"], true);

    // Verify it's gone
    let op = KanbanOperation::new(Verb::Get, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("id".into(), json!(id));
        m
    });
    let result = execute_operation(&ctx, &op).await;
    assert!(result.is_err(), "deleted perspective should not be found");
}

#[tokio::test]
async fn dispatch_perspective_full_lifecycle() {
    let (_temp, ctx) = setup().await;

    // Add
    let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("name".into(), json!("Lifecycle Test"));
        m.insert("view".into(), json!("board"));
        m.insert("filter".into(), json!("(e) => e.Status !== 'Done'"));
        m
    });
    let added = execute_operation(&ctx, &op).await.unwrap();
    let id = added["id"].as_str().unwrap().to_string();
    assert_eq!(added["name"], "Lifecycle Test");
    assert_eq!(added["filter"], "(e) => e.Status !== 'Done'");

    // Get
    let op = KanbanOperation::new(Verb::Get, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("id".into(), json!(&id));
        m
    });
    let got = execute_operation(&ctx, &op).await.unwrap();
    assert_eq!(got["name"], "Lifecycle Test");

    // Update
    let op = KanbanOperation::new(Verb::Update, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("id".into(), json!(&id));
        m.insert("name".into(), json!("Updated Lifecycle"));
        m.insert("group".into(), json!("(e) => e.Assignee"));
        m
    });
    let updated = execute_operation(&ctx, &op).await.unwrap();
    assert_eq!(updated["name"], "Updated Lifecycle");
    assert_eq!(updated["group"], "(e) => e.Assignee");
    // Filter should be preserved
    assert_eq!(updated["filter"], "(e) => e.Status !== 'Done'");

    // List
    let op = KanbanOperation::new(Verb::List, Noun::Perspectives, serde_json::Map::new());
    let listed = execute_operation(&ctx, &op).await.unwrap();
    assert_eq!(listed["count"], 1);

    // Delete
    let op = KanbanOperation::new(Verb::Delete, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("id".into(), json!(&id));
        m
    });
    let deleted = execute_operation(&ctx, &op).await.unwrap();
    assert_eq!(deleted["deleted"], true);

    // Verify empty
    let op = KanbanOperation::new(Verb::List, Noun::Perspectives, serde_json::Map::new());
    let listed = execute_operation(&ctx, &op).await.unwrap();
    assert_eq!(listed["count"], 0);
}

#[tokio::test]
async fn dispatch_update_perspective_clear_filter_and_group_via_null() {
    let (_temp, ctx) = setup().await;

    // Add a perspective with filter and group set
    let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("name".into(), json!("Null Clear Test"));
        m.insert("view".into(), json!("board"));
        m.insert("filter".into(), json!("(e) => e.Status !== 'Done'"));
        m.insert("group".into(), json!("(e) => e.Assignee"));
        m
    });
    let added = execute_operation(&ctx, &op).await.unwrap();
    let id = added["id"].as_str().unwrap().to_string();
    assert_eq!(added["filter"], "(e) => e.Status !== 'Done'");
    assert_eq!(added["group"], "(e) => e.Assignee");

    // Update with filter: null and group: null to clear them
    let op = KanbanOperation::new(Verb::Update, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("id".into(), json!(&id));
        m.insert("filter".into(), Value::Null);
        m.insert("group".into(), Value::Null);
        m
    });
    let updated = execute_operation(&ctx, &op).await.unwrap();
    assert!(
        updated.get("filter").is_none() || updated["filter"].is_null(),
        "filter should be cleared (null or absent), got: {:?}",
        updated.get("filter")
    );
    assert!(
        updated.get("group").is_none() || updated["group"].is_null(),
        "group should be cleared (null or absent), got: {:?}",
        updated.get("group")
    );

    // Verify via get that the clear persisted
    let op = KanbanOperation::new(Verb::Get, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("id".into(), json!(&id));
        m
    });
    let got = execute_operation(&ctx, &op).await.unwrap();
    assert!(
        got.get("filter").is_none() || got["filter"].is_null(),
        "filter should remain cleared after re-fetch, got: {:?}",
        got.get("filter")
    );
    assert!(
        got.get("group").is_none() || got["group"].is_null(),
        "group should remain cleared after re-fetch, got: {:?}",
        got.get("group")
    );
}

/// Passing malformed `fields` JSON to `add perspective` should return a parse error
/// instead of silently dropping the value.
#[tokio::test]
async fn dispatch_add_perspective_malformed_fields_returns_error() {
    let (_temp, ctx) = setup().await;

    let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("name".into(), json!("Bad Fields"));
        m.insert("view".into(), json!("board"));
        // fields should be an array of PerspectiveFieldEntry, not a string
        m.insert("fields".into(), json!("not-an-array"));
        m
    });
    let result = execute_operation(&ctx, &op).await;
    assert!(
        result.is_err(),
        "malformed fields should produce an error, not be silently dropped"
    );
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("invalid fields"),
        "error should mention 'invalid fields', got: {err_msg}"
    );
}

/// Passing malformed `sort` JSON to `add perspective` should return a parse error.
#[tokio::test]
async fn dispatch_add_perspective_malformed_sort_returns_error() {
    let (_temp, ctx) = setup().await;

    let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("name".into(), json!("Bad Sort"));
        m.insert("view".into(), json!("board"));
        // sort should be an array of SortEntry, not a number
        m.insert("sort".into(), json!(42));
        m
    });
    let result = execute_operation(&ctx, &op).await;
    assert!(
        result.is_err(),
        "malformed sort should produce an error, not be silently dropped"
    );
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("invalid sort"),
        "error should mention 'invalid sort', got: {err_msg}"
    );
}

/// Passing malformed `fields` JSON to `update perspective` should return a parse error.
#[tokio::test]
async fn dispatch_update_perspective_malformed_fields_returns_error() {
    let (_temp, ctx) = setup().await;

    // Create a valid perspective first
    let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("name".into(), json!("Valid"));
        m.insert("view".into(), json!("board"));
        m
    });
    let added = execute_operation(&ctx, &op).await.unwrap();
    let id = added["id"].as_str().unwrap().to_string();

    // Update with malformed fields
    let op = KanbanOperation::new(Verb::Update, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("id".into(), json!(id));
        m.insert("fields".into(), json!({"wrong": "shape"}));
        m
    });
    let result = execute_operation(&ctx, &op).await;
    assert!(
        result.is_err(),
        "malformed fields on update should produce an error"
    );
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("invalid fields"),
        "error should mention 'invalid fields', got: {err_msg}"
    );
}

/// Passing malformed `sort` JSON to `update perspective` should return a parse error.
#[tokio::test]
async fn dispatch_update_perspective_malformed_sort_returns_error() {
    let (_temp, ctx) = setup().await;

    // Create a valid perspective first
    let op = KanbanOperation::new(Verb::Add, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("name".into(), json!("Valid"));
        m.insert("view".into(), json!("board"));
        m
    });
    let added = execute_operation(&ctx, &op).await.unwrap();
    let id = added["id"].as_str().unwrap().to_string();

    // Update with malformed sort
    let op = KanbanOperation::new(Verb::Update, Noun::Perspective, {
        let mut m = serde_json::Map::new();
        m.insert("id".into(), json!(id));
        m.insert("sort".into(), json!("not-an-array"));
        m
    });
    let result = execute_operation(&ctx, &op).await;
    assert!(
        result.is_err(),
        "malformed sort on update should produce an error"
    );
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("invalid sort"),
        "error should mention 'invalid sort', got: {err_msg}"
    );
}
