//! End-to-end coverage for the `entity` server's `filter perspective` verb.
//!
//! Editing a perspective's filter must, when that perspective is the
//! dispatching window's active selection, recompute the window's
//! `filtered_task_ids` and return a `{ ok, change }` envelope carrying a
//! `PerspectiveSwitch` change — the event-driven refresh the host's
//! `ui-state-changed` emit rides. This is the fix for the "filter change
//! doesn't refresh until click-away/back" live bug (card
//! 01KV0MJYA58GW5PRXGVXWHQK32): the `perspective.filter` command used to route
//! to the `views` server's storage-only `set filter` op, which never wrote
//! `UiState`, so no event ever fired.
//!
//! These tests drive the verb through the real `ServerHandler` surface against
//! a full board substrate + `UiState`, reusing the shared
//! `SetFilterAndRefreshCmd` (no duplicate filter-eval logic in this crate).

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{json, Value};
use swissarmyhammer_kanban::commands::perspective_commands::SavePerspectiveCmd;
use swissarmyhammer_kanban::commands_core::{Command, CommandContext};
use swissarmyhammer_kanban::task::AddTask;
use swissarmyhammer_kanban::{KanbanOperationProcessor, OperationProcessor};

use super::common::{call_tool, ClipboardHarness};

/// Add a task with a body (the body carries `#tag` markers the enrich
/// pipeline lifts into `filter_tags`) and return its id.
async fn add_task_with_body(h: &ClipboardHarness, title: &str, body: &str) -> String {
    let result = KanbanOperationProcessor::new()
        .process(
            &AddTask::new(title).with_description(body),
            h.kanban.as_ref(),
        )
        .await
        .expect("add task");
    result["id"].as_str().expect("task id").to_string()
}

/// Save a no-filter board perspective via the shared `SavePerspectiveCmd`
/// against the harness's `KanbanContext`, returning its id.
async fn save_board_perspective(h: &ClipboardHarness, name: &str) -> String {
    let mut args = HashMap::new();
    args.insert("name".to_string(), Value::String(name.to_string()));
    args.insert("view".to_string(), Value::String("board".to_string()));
    let mut ctx = CommandContext::new("perspective.save", vec![], None, args);
    ctx.set_extension(Arc::clone(&h.kanban));
    SavePerspectiveCmd
        .execute(&ctx)
        .await
        .expect("save perspective")["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn filter_perspective_on_active_recomputes_filtered_ids_and_returns_change() {
    let h = ClipboardHarness::new().await;
    let server = h.server().await;

    let t_bug = add_task_with_body(&h, "Bug", "#bug top").await;
    let _t_feat = add_task_with_body(&h, "Feature", "#feature pretty").await;

    // Create a no-filter perspective and make it the window's active selection
    // via the real `switch perspective` op.
    let pid = save_board_perspective(&h, "All").await;

    call_tool(
        &server,
        "switch perspective",
        json!({
            "op": "switch perspective",
            "perspective_id": pid,
            "scope": ["window:main"],
        }),
    )
    .await
    .expect("switch perspective");

    assert_eq!(
        h.ui_state.filtered_task_ids("main").len(),
        2,
        "precondition: no-filter perspective shows every task"
    );

    // Edit the active perspective's filter to `#bug` through the new verb.
    let resp = call_tool(
        &server,
        "filter perspective",
        json!({
            "op": "filter perspective",
            "perspective_id": pid,
            "filter": "#bug",
            "scope": ["window:main"],
        }),
    )
    .await
    .expect("filter perspective");

    assert_eq!(resp.get("ok"), Some(&Value::Bool(true)));
    let change = resp.get("change").expect("envelope carries change");
    let change: swissarmyhammer_ui_state::UiStateChange =
        serde_json::from_value(change.clone()).expect("change is a UiStateChange");
    match change {
        swissarmyhammer_ui_state::UiStateChange::PerspectiveSwitch {
            perspective_id,
            filtered_task_ids,
        } => {
            assert_eq!(perspective_id, pid);
            assert_eq!(filtered_task_ids, vec![t_bug.clone()]);
        }
        other => panic!("expected PerspectiveSwitch, got: {other:?}"),
    }

    assert_eq!(h.ui_state.filtered_task_ids("main"), vec![t_bug]);
}

#[tokio::test]
async fn filter_perspective_requires_window_moniker() {
    let h = ClipboardHarness::new().await;
    let server = h.server().await;

    let pid = save_board_perspective(&h, "All").await;

    // No `window:<label>` moniker — per-window op must reject (no silent main
    // fallback), same hardening as switch/next/prev/delete.
    let err = call_tool(
        &server,
        "filter perspective",
        json!({
            "op": "filter perspective",
            "perspective_id": pid,
            "filter": "#bug",
            "scope": [],
        }),
    )
    .await
    .expect_err("missing window moniker must error");
    assert!(
        err.message.contains("window:"),
        "error should mention the required window moniker, got: {}",
        err.message
    );
}
