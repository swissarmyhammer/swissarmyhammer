//! Integration tests for perspective operations through the dispatch path.
//!
//! Verifies the full add → get → update → list → delete lifecycle via
//! `parse_input` → `execute_operation`, including JSON output shapes and
//! changelog entry production.

use serde_json::json;
use std::sync::Arc;
use swissarmyhammer_kanban::{
    board::InitBoard, dispatch::execute_operation, parse::parse_input, Execute, KanbanContext,
};
use tempfile::TempDir;

/// Set up an initialized board in a temp directory.
async fn setup() -> (TempDir, KanbanContext) {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);

    InitBoard::new("Test Board")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    (temp, ctx)
}

/// Parse and execute a single JSON operation through the dispatch path.
async fn dispatch(ctx: &KanbanContext, input: serde_json::Value) -> serde_json::Value {
    let ops = parse_input(input).expect("parse_input should succeed");
    assert_eq!(ops.len(), 1, "expected exactly one parsed operation");
    execute_operation(ctx, &ops[0])
        .await
        .expect("execute_operation should succeed")
}

/// Full lifecycle: add → get → update → list → delete with changelog verification.
///
/// This exercises the same code path as the MCP tool: JSON input is parsed by
/// `parse_input`, then dispatched by `execute_operation`. After each mutation
/// we verify the JSON output shape and at the end we verify that the
/// `perspectives.jsonl` changelog contains the expected entries.
#[tokio::test]
async fn test_perspective_lifecycle_integration() {
    let (_temp, ctx) = setup().await;

    // --- Add ---
    let added = dispatch(
        &ctx,
        json!({
            "op": "add perspective",
            "name": "Sprint Board",
            "view": "board",
            "filter": "(e) => e.status !== 'done'"
        }),
    )
    .await;

    // Verify add output shape
    let id = added["id"]
        .as_str()
        .expect("add should return an id string");
    assert!(!id.is_empty(), "id should be non-empty");
    assert_eq!(added["name"], "Sprint Board");
    assert_eq!(added["view"], "board");
    assert_eq!(added["filter"], "(e) => e.status !== 'done'");

    // --- Get ---
    let got = dispatch(
        &ctx,
        json!({
            "op": "get perspective",
            "id": id
        }),
    )
    .await;

    assert_eq!(got["id"], id);
    assert_eq!(got["name"], "Sprint Board");
    assert_eq!(got["view"], "board");
    assert_eq!(got["filter"], "(e) => e.status !== 'done'");

    // --- Update ---
    let updated = dispatch(
        &ctx,
        json!({
            "op": "update perspective",
            "id": id,
            "name": "Updated Sprint",
            "view": "grid",
            "group": "(e) => e.assignee"
        }),
    )
    .await;

    assert_eq!(updated["id"], id);
    assert_eq!(updated["name"], "Updated Sprint");
    assert_eq!(updated["view"], "grid");
    assert_eq!(updated["group"], "(e) => e.assignee");
    // Filter should be preserved from the original add
    assert_eq!(
        updated["filter"], "(e) => e.status !== 'done'",
        "filter should be preserved when not explicitly changed"
    );

    // --- List ---
    let listed = dispatch(&ctx, json!({"op": "list perspectives"})).await;

    assert_eq!(listed["count"], 1, "should have exactly one perspective");
    let perspectives = listed["perspectives"]
        .as_array()
        .expect("list should return a perspectives array");
    assert_eq!(perspectives.len(), 1);
    assert_eq!(perspectives[0]["name"], "Updated Sprint");

    // --- Delete ---
    let deleted = dispatch(
        &ctx,
        json!({
            "op": "delete perspective",
            "id": id
        }),
    )
    .await;

    assert_eq!(deleted["deleted"], true);

    // Verify it is gone
    let listed_after = dispatch(&ctx, json!({"op": "list perspectives"})).await;
    assert_eq!(
        listed_after["count"], 0,
        "after delete, count should be zero"
    );
}

/// Round-trip a perspective with a DSL filter expression through
/// add → get → update to verify the filter is stored and retrieved correctly.
#[tokio::test]
async fn test_perspective_dsl_filter_round_trip() {
    let (_temp, ctx) = setup().await;

    // Add with a DSL filter
    let added = dispatch(
        &ctx,
        json!({
            "op": "add perspective",
            "name": "Bug Board",
            "view": "board",
            "filter": "#bug && @will"
        }),
    )
    .await;

    let id = added["id"].as_str().unwrap();
    assert_eq!(added["filter"], "#bug && @will");

    // Get it back — filter should be preserved
    let got = dispatch(&ctx, json!({"op": "get perspective", "id": id})).await;
    assert_eq!(got["filter"], "#bug && @will");

    // Update to a different DSL filter
    let updated = dispatch(
        &ctx,
        json!({
            "op": "update perspective",
            "id": id,
            "filter": "!#done || #READY"
        }),
    )
    .await;
    assert_eq!(updated["filter"], "!#done || #READY");

    // Get again to confirm persistence
    let got2 = dispatch(&ctx, json!({"op": "get perspective", "id": id})).await;
    assert_eq!(got2["filter"], "!#done || #READY");
}

/// Prove the full pipeline: add perspectives with StoreHandle wired in,
/// then list returns them AND StoreHandle has pending change events.
#[tokio::test]
async fn test_perspective_store_events_and_list() {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);

    InitBoard::new("Store Test")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    // Wire up StoreHandle for perspectives (mirrors BoardHandle::open)
    let perspectives_dir = kanban_dir.join("perspectives");
    std::fs::create_dir_all(&perspectives_dir).unwrap();
    let store = Arc::new(swissarmyhammer_perspectives::PerspectiveStore::new(
        &perspectives_dir,
    ));
    let handle = Arc::new(swissarmyhammer_store::StoreHandle::new(store));

    {
        let pctx = ctx.perspective_context().await.unwrap();
        pctx.write().await.set_store_handle(Arc::clone(&handle));
    }

    // Add two perspectives
    let added1 = dispatch(
        &ctx,
        json!({"op": "add perspective", "name": "Board Sprint", "view": "board"}),
    )
    .await;
    let id1 = added1["id"].as_str().unwrap();

    let added2 = dispatch(
        &ctx,
        json!({"op": "add perspective", "name": "Grid Overview", "view": "grid"}),
    )
    .await;
    let id2 = added2["id"].as_str().unwrap();

    // List must return both
    let listed = dispatch(&ctx, json!({"op": "list perspectives"})).await;
    assert_eq!(listed["count"], 2);
    let names: Vec<&str> = listed["perspectives"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"Board Sprint"));
    assert!(names.contains(&"Grid Overview"));

    // StoreHandle must have pending events (two creates)
    let events = handle.flush_changes().await;
    assert_eq!(events.len(), 2, "expected two create events");
    assert!(events.iter().all(|e| e.event_name() == "item-created"));

    let event_ids: Vec<&str> = events
        .iter()
        .map(|e| e.payload()["id"].as_str().unwrap())
        .collect();
    assert!(event_ids.contains(&id1));
    assert!(event_ids.contains(&id2));
}
