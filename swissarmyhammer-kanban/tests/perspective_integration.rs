//! Integration tests for perspective operations through the dispatch path.
//!
//! Verifies the full add → get → update → list → delete lifecycle via
//! `parse_input` → `execute_operation`, including JSON output shapes and
//! changelog entry production.

use serde_json::json;
use swissarmyhammer_kanban::{
    board::InitBoard,
    dispatch::execute_operation,
    parse::parse_input,
    perspective::{PerspectiveChangeOp, PerspectiveChangelog},
    Execute, KanbanContext,
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
    let (temp, ctx) = setup().await;

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

    // --- Changelog verification ---
    let changelog_path = temp.path().join(".kanban").join("perspectives.jsonl");
    let changelog = PerspectiveChangelog::new(changelog_path);
    let entries = changelog.read_all().await.unwrap();

    // We expect 3 changelog entries: create, update, delete
    assert_eq!(
        entries.len(),
        3,
        "changelog should have exactly 3 entries (create, update, delete), got {}",
        entries.len()
    );

    assert_eq!(entries[0].op, PerspectiveChangeOp::Create);
    assert_eq!(entries[0].perspective_id, id);
    assert!(
        entries[0].previous.is_none(),
        "create entry should have no previous"
    );
    assert!(
        entries[0].current.is_some(),
        "create entry should have current"
    );
    assert_eq!(entries[0].current.as_ref().unwrap()["name"], "Sprint Board");

    assert_eq!(entries[1].op, PerspectiveChangeOp::Update);
    assert_eq!(entries[1].perspective_id, id);
    assert!(
        entries[1].previous.is_some(),
        "update entry should have previous"
    );
    assert!(
        entries[1].current.is_some(),
        "update entry should have current"
    );
    assert_eq!(
        entries[1].previous.as_ref().unwrap()["name"],
        "Sprint Board"
    );
    assert_eq!(
        entries[1].current.as_ref().unwrap()["name"],
        "Updated Sprint"
    );

    assert_eq!(entries[2].op, PerspectiveChangeOp::Delete);
    assert_eq!(entries[2].perspective_id, id);
    assert!(
        entries[2].previous.is_some(),
        "delete entry should have previous"
    );
    assert!(
        entries[2].current.is_none(),
        "delete entry should have no current"
    );
    assert_eq!(
        entries[2].previous.as_ref().unwrap()["name"],
        "Updated Sprint"
    );
}
