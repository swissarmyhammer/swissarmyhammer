//! End-to-end integration tests for the filter DSL pipeline.
//!
//! Exercises DSL expression → Rust parser → evaluator → filtered results
//! through the dispatch path (`parse_input` → `execute_operation`), verifying
//! basic filtering, boolean logic, virtual tags, keyword operators, edge cases,
//! and cross-surface consistency.

use serde_json::json;
use swissarmyhammer_kanban::{
    board::InitBoard,
    dispatch::execute_operation,
    parse::parse_input,
    task::ListTasks,
    Execute, KanbanContext,
};
use tempfile::TempDir;

/// Initialize a board with default columns (todo, doing, done).
async fn setup() -> (TempDir, KanbanContext) {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);
    InitBoard::new("Filter Test").execute(&ctx).await.into_result().unwrap();
    (temp, ctx)
}

/// Parse and execute a JSON operation through the dispatch path.
async fn dispatch(ctx: &KanbanContext, input: serde_json::Value) -> serde_json::Value {
    let ops = parse_input(input).expect("parse_input should succeed");
    assert_eq!(ops.len(), 1);
    execute_operation(ctx, &ops[0]).await.expect("execute_operation should succeed")
}

/// Collect task titles from a list result.
fn titles(result: &serde_json::Value) -> Vec<String> {
    result["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap().to_string())
        .collect()
}

// =========================================================================
// Basic filtering (scenarios 1-3)
// =========================================================================

#[tokio::test]
async fn s01_filter_by_tag() {
    let (_tmp, ctx) = setup().await;
    dispatch(&ctx, json!({"op": "add task", "title": "Bug task", "description": "#bug report"})).await;
    dispatch(&ctx, json!({"op": "add task", "title": "Feature task", "description": "#feature request"})).await;
    dispatch(&ctx, json!({"op": "add task", "title": "Docs task", "description": "#docs update"})).await;

    let result = dispatch(&ctx, json!({"op": "list tasks", "filter": "#bug"})).await;
    assert_eq!(result["count"], 1);
    assert_eq!(titles(&result), vec!["Bug task"]);
}

#[tokio::test]
async fn s02_filter_by_assignee() {
    let (_tmp, ctx) = setup().await;
    dispatch(&ctx, json!({"op": "add actor", "id": "alice", "name": "Alice", "type": "human"})).await;
    dispatch(&ctx, json!({"op": "add actor", "id": "bob", "name": "Bob", "type": "human"})).await;

    let r1 = dispatch(&ctx, json!({"op": "add task", "title": "Alice task"})).await;
    let r2 = dispatch(&ctx, json!({"op": "add task", "title": "Bob task"})).await;
    dispatch(&ctx, json!({"op": "add task", "title": "Unassigned"})).await;

    dispatch(&ctx, json!({"op": "assign task", "id": r1["id"], "assignee": "alice"})).await;
    dispatch(&ctx, json!({"op": "assign task", "id": r2["id"], "assignee": "bob"})).await;

    let result = dispatch(&ctx, json!({"op": "list tasks", "filter": "@alice"})).await;
    assert_eq!(result["count"], 1);
    assert_eq!(titles(&result), vec!["Alice task"]);
}

#[tokio::test]
async fn s03_filter_by_ref() {
    let (_tmp, ctx) = setup().await;
    let r1 = dispatch(&ctx, json!({"op": "add task", "title": "Dependency"})).await;
    let dep_id = r1["id"].as_str().unwrap();

    dispatch(&ctx, json!({"op": "add task", "title": "Depends on dep", "depends_on": [dep_id]})).await;
    dispatch(&ctx, json!({"op": "add task", "title": "Independent"})).await;

    let result = dispatch(&ctx, json!({"op": "list tasks", "filter": format!("^{dep_id}")})).await;
    let t = titles(&result);
    // Both the dependency itself (id match) and the task that depends on it should match
    assert!(t.contains(&"Dependency".to_string()), "ref should match entity's own id");
    assert!(t.contains(&"Depends on dep".to_string()), "ref should match depends_on");
    assert!(!t.contains(&"Independent".to_string()));
}

// =========================================================================
// Boolean logic (scenarios 4-8)
// =========================================================================

#[tokio::test]
async fn s04_and_operator() {
    let (_tmp, ctx) = setup().await;
    dispatch(&ctx, json!({"op": "add actor", "id": "alice", "name": "Alice", "type": "human"})).await;

    let r1 = dispatch(&ctx, json!({"op": "add task", "title": "Bug by Alice", "description": "#bug"})).await;
    dispatch(&ctx, json!({"op": "assign task", "id": r1["id"], "assignee": "alice"})).await;
    dispatch(&ctx, json!({"op": "add task", "title": "Bug unassigned", "description": "#bug"})).await;

    let r3 = dispatch(&ctx, json!({"op": "add task", "title": "Feature by Alice", "description": "#feature"})).await;
    dispatch(&ctx, json!({"op": "assign task", "id": r3["id"], "assignee": "alice"})).await;

    let result = dispatch(&ctx, json!({"op": "list tasks", "filter": "#bug && @alice"})).await;
    assert_eq!(result["count"], 1);
    assert_eq!(titles(&result), vec!["Bug by Alice"]);
}

#[tokio::test]
async fn s05_or_operator() {
    let (_tmp, ctx) = setup().await;
    dispatch(&ctx, json!({"op": "add task", "title": "Bug", "description": "#bug"})).await;
    dispatch(&ctx, json!({"op": "add task", "title": "Feature", "description": "#feature"})).await;
    dispatch(&ctx, json!({"op": "add task", "title": "Docs", "description": "#docs"})).await;

    let result = dispatch(&ctx, json!({"op": "list tasks", "filter": "#bug || #feature"})).await;
    assert_eq!(result["count"], 2);
    let t = titles(&result);
    assert!(t.contains(&"Bug".to_string()));
    assert!(t.contains(&"Feature".to_string()));
    assert!(!t.contains(&"Docs".to_string()));
}

#[tokio::test]
async fn s06_not_operator() {
    let (_tmp, ctx) = setup().await;
    dispatch(&ctx, json!({"op": "add task", "title": "Has done tag", "description": "#done"})).await;
    dispatch(&ctx, json!({"op": "add task", "title": "No done tag", "description": "#bug"})).await;

    let result = dispatch(&ctx, json!({"op": "list tasks", "filter": "!#done"})).await;
    assert_eq!(titles(&result), vec!["No done tag"]);
}

#[tokio::test]
async fn s07_grouping() {
    let (_tmp, ctx) = setup().await;
    dispatch(&ctx, json!({"op": "add actor", "id": "alice", "name": "Alice", "type": "human"})).await;

    let r1 = dispatch(&ctx, json!({"op": "add task", "title": "Bug by Alice", "description": "#bug"})).await;
    dispatch(&ctx, json!({"op": "assign task", "id": r1["id"], "assignee": "alice"})).await;

    let r2 = dispatch(&ctx, json!({"op": "add task", "title": "Feature by Alice", "description": "#feature"})).await;
    dispatch(&ctx, json!({"op": "assign task", "id": r2["id"], "assignee": "alice"})).await;

    dispatch(&ctx, json!({"op": "add task", "title": "Docs unassigned", "description": "#docs"})).await;

    let result = dispatch(&ctx, json!({"op": "list tasks", "filter": "(#bug || #feature) && @alice"})).await;
    assert_eq!(result["count"], 2);
    let t = titles(&result);
    assert!(t.contains(&"Bug by Alice".to_string()));
    assert!(t.contains(&"Feature by Alice".to_string()));
}

#[tokio::test]
async fn s08_implicit_and() {
    let (_tmp, ctx) = setup().await;
    dispatch(&ctx, json!({"op": "add actor", "id": "alice", "name": "Alice", "type": "human"})).await;

    let r1 = dispatch(&ctx, json!({"op": "add task", "title": "Bug by Alice", "description": "#bug"})).await;
    dispatch(&ctx, json!({"op": "assign task", "id": r1["id"], "assignee": "alice"})).await;
    dispatch(&ctx, json!({"op": "add task", "title": "Bug unassigned", "description": "#bug"})).await;

    // Implicit AND: "#bug @alice" should behave like "#bug && @alice"
    let result = dispatch(&ctx, json!({"op": "list tasks", "filter": "#bug @alice"})).await;
    assert_eq!(result["count"], 1);
    assert_eq!(titles(&result), vec!["Bug by Alice"]);
}

// =========================================================================
// Virtual tags (scenarios 9-11)
// =========================================================================

#[tokio::test]
async fn s09_virtual_tag_ready() {
    let (_tmp, ctx) = setup().await;
    let r1 = dispatch(&ctx, json!({"op": "add task", "title": "Blocker"})).await;
    let id1 = r1["id"].as_str().unwrap();
    dispatch(&ctx, json!({"op": "add task", "title": "Blocked", "depends_on": [id1]})).await;

    let result = dispatch(&ctx, json!({"op": "list tasks", "filter": "#READY"})).await;
    assert_eq!(result["count"], 1);
    assert_eq!(titles(&result), vec!["Blocker"]);
}

#[tokio::test]
async fn s10_virtual_tag_blocked() {
    let (_tmp, ctx) = setup().await;
    let r1 = dispatch(&ctx, json!({"op": "add task", "title": "Blocker"})).await;
    let id1 = r1["id"].as_str().unwrap();
    dispatch(&ctx, json!({"op": "add task", "title": "Blocked", "depends_on": [id1]})).await;

    let result = dispatch(&ctx, json!({"op": "list tasks", "filter": "#BLOCKED"})).await;
    assert_eq!(result["count"], 1);
    assert_eq!(titles(&result), vec!["Blocked"]);
}

#[tokio::test]
async fn s11_virtual_tag_combined_with_real_tag() {
    let (_tmp, ctx) = setup().await;
    let r1 = dispatch(&ctx, json!({"op": "add task", "title": "Ready bug", "description": "#bug"})).await;
    let _id1 = r1["id"].as_str().unwrap();
    dispatch(&ctx, json!({"op": "add task", "title": "Ready feature", "description": "#feature"})).await;

    let result = dispatch(&ctx, json!({"op": "list tasks", "filter": "#READY && #bug"})).await;
    assert_eq!(result["count"], 1);
    assert_eq!(titles(&result), vec!["Ready bug"]);
}

// =========================================================================
// Keyword operators (scenarios 12-13)
// =========================================================================

#[tokio::test]
async fn s12_keyword_operators_lowercase() {
    let (_tmp, ctx) = setup().await;
    dispatch(&ctx, json!({"op": "add actor", "id": "alice", "name": "Alice", "type": "human"})).await;

    let r1 = dispatch(&ctx, json!({"op": "add task", "title": "Alice urgent", "description": "#urgent"})).await;
    dispatch(&ctx, json!({"op": "assign task", "id": r1["id"], "assignee": "alice"})).await;

    dispatch(&ctx, json!({"op": "add task", "title": "Done tagged", "description": "#done"})).await;
    dispatch(&ctx, json!({"op": "add task", "title": "Plain task"})).await;

    // "not #done and @alice or #urgent" → ((!#done) && @alice) || #urgent
    let result = dispatch(&ctx, json!({"op": "list tasks", "filter": "not #done and @alice or #urgent"})).await;
    let t = titles(&result);
    assert!(t.contains(&"Alice urgent".to_string()), "matches both @alice and #urgent");
    assert!(!t.contains(&"Done tagged".to_string()), "#done excluded by not");
}

#[tokio::test]
async fn s13_keyword_operators_uppercase() {
    let (_tmp, ctx) = setup().await;
    dispatch(&ctx, json!({"op": "add actor", "id": "alice", "name": "Alice", "type": "human"})).await;

    let r1 = dispatch(&ctx, json!({"op": "add task", "title": "Alice no-done", "description": "#bug"})).await;
    dispatch(&ctx, json!({"op": "assign task", "id": r1["id"], "assignee": "alice"})).await;

    dispatch(&ctx, json!({"op": "add task", "title": "Done task", "description": "#done"})).await;

    let result = dispatch(&ctx, json!({"op": "list tasks", "filter": "NOT #done AND @alice"})).await;
    assert_eq!(result["count"], 1);
    assert_eq!(titles(&result), vec!["Alice no-done"]);
}

// =========================================================================
// Edge cases (scenarios 14-17)
// =========================================================================

#[tokio::test]
async fn s14_empty_filter_returns_all() {
    let (_tmp, ctx) = setup().await;
    dispatch(&ctx, json!({"op": "add task", "title": "T1"})).await;
    dispatch(&ctx, json!({"op": "add task", "title": "T2"})).await;

    let result = dispatch(&ctx, json!({"op": "list tasks", "filter": ""})).await;
    assert_eq!(result["count"], 2);
}

#[tokio::test]
async fn s15_invalid_filter_returns_error() {
    let (_tmp, ctx) = setup().await;
    dispatch(&ctx, json!({"op": "add task", "title": "T1"})).await;

    let ops = parse_input(json!({"op": "list tasks", "filter": "$$garbage"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await;
    assert!(result.is_err(), "invalid filter should return an error");
}

#[tokio::test]
async fn s16_no_matches_returns_empty() {
    let (_tmp, ctx) = setup().await;
    dispatch(&ctx, json!({"op": "add task", "title": "T1", "description": "#bug"})).await;

    let result = dispatch(&ctx, json!({"op": "list tasks", "filter": "#nonexistent-tag"})).await;
    assert_eq!(result["count"], 0);
    assert!(result["tasks"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn s17_tag_names_with_special_chars() {
    let (_tmp, ctx) = setup().await;
    dispatch(&ctx, json!({"op": "add task", "title": "Version task", "description": "#v2.0 release"})).await;
    dispatch(&ctx, json!({"op": "add task", "title": "Bugfix task", "description": "#bug-fix found"})).await;
    dispatch(&ctx, json!({"op": "add task", "title": "Plain"})).await;

    let result = dispatch(&ctx, json!({"op": "list tasks", "filter": "#v2.0"})).await;
    assert_eq!(result["count"], 1);
    assert_eq!(titles(&result), vec!["Version task"]);

    let result = dispatch(&ctx, json!({"op": "list tasks", "filter": "#bug-fix"})).await;
    assert_eq!(result["count"], 1);
    assert_eq!(titles(&result), vec!["Bugfix task"]);
}

// =========================================================================
// Cross-surface consistency (scenarios 18-19)
// =========================================================================

#[tokio::test]
async fn s18_dispatch_and_direct_api_return_same_results() {
    let (_tmp, ctx) = setup().await;
    dispatch(&ctx, json!({"op": "add task", "title": "Bug", "description": "#bug"})).await;
    dispatch(&ctx, json!({"op": "add task", "title": "Feature", "description": "#feature"})).await;

    // Via dispatch (MCP path)
    let dispatch_result = dispatch(&ctx, json!({"op": "list tasks", "filter": "#bug"})).await;
    let dispatch_titles = titles(&dispatch_result);

    // Via direct operation API
    let direct_result = ListTasks::new()
        .with_filter("#bug")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    let direct_titles: Vec<String> = direct_result["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap().to_string())
        .collect();

    assert_eq!(dispatch_titles, direct_titles, "dispatch and direct API should return identical results");
    assert_eq!(dispatch_titles, vec!["Bug"]);
}

#[tokio::test]
async fn s19_perspective_filter_round_trip() {
    let (_tmp, ctx) = setup().await;

    // Save a perspective with a DSL filter
    let added = dispatch(&ctx, json!({
        "op": "add perspective",
        "name": "Bug Board",
        "view": "board",
        "filter": "#bug && @alice"
    })).await;
    let id = added["id"].as_str().unwrap();

    // Get it back — filter should be preserved
    let got = dispatch(&ctx, json!({"op": "get perspective", "id": id})).await;
    assert_eq!(got["filter"], "#bug && @alice");

    // Update the filter
    let updated = dispatch(&ctx, json!({
        "op": "update perspective",
        "id": id,
        "filter": "!#done || #READY"
    })).await;
    assert_eq!(updated["filter"], "!#done || #READY");

    // Re-read to confirm persistence
    let got2 = dispatch(&ctx, json!({"op": "get perspective", "id": id})).await;
    assert_eq!(got2["filter"], "!#done || #READY");
}
