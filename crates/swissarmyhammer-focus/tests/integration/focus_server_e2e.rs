//! End-to-end tests for the `focus` MCP server.
//!
//! Stands up a real [`FocusServer`] and exercises every verb the `_meta` tree
//! advertises, driving each through the real `ServerHandler` / `call_tool`
//! path. Each test asserts the structured response (`event` / `next_fq`) and,
//! where it matters, the resulting focus state read back through a follow-up
//! op — so the behavior is pinned to match the original `spatial_*` Tauri
//! commands.

use serde_json::{json, Value};
use swissarmyhammer_focus::FocusServer;

use super::common::call_tool;

/// Build a single-scope snapshot under `layer_fq` containing `fq` at the
/// zero rect (no parent zone / overrides) — the minimal shape `focus`
/// needs to commit.
fn snapshot_one(layer_fq: &str, fq: &str) -> Value {
    json!({
        "layer_fq": layer_fq,
        "scopes": [
            { "fq": fq, "rect": { "x": 0.0, "y": 0.0, "width": 10.0, "height": 10.0 },
              "parent_zone": null, "nav_override": {} }
        ]
    })
}

/// Push a window-root layer at `layer_fq` owned by `window`.
async fn push_root_layer(server: &FocusServer, layer_fq: &str, window: &str) {
    let res = call_tool(
        server,
        "push layer",
        json!({
            "op": "push layer",
            "fq": layer_fq,
            "segment": "window",
            "name": "window",
            "parent": null,
            "window": window,
        }),
    )
    .await
    .expect("push layer should succeed");
    assert_eq!(res["ok"], json!(true));
}

/// `set focus` (the `ui.setFocus` routing target) commits focus and returns
/// a `FocusChangedEvent` with the resolved window / segment — mirroring
/// `spatial_focus`.
#[tokio::test]
async fn set_focus_routes_and_emits_event() {
    let server = FocusServer::new();
    push_root_layer(&server, "/L", "main").await;

    let res = call_tool(
        &server,
        "set focus",
        json!({ "op": "set focus", "fq": "/L/k1", "snapshot": snapshot_one("/L", "/L/k1") }),
    )
    .await
    .expect("set focus should succeed");

    assert_eq!(res["ok"], json!(true));
    let event = &res["event"];
    assert_eq!(event["window_label"], json!("main"));
    assert_eq!(event["prev_fq"], Value::Null);
    assert_eq!(event["next_fq"], json!("/L/k1"));
    assert_eq!(event["next_segment"], json!("k1"));
}

/// `set focus` with no snapshot drops the commit silently (transient unmount
/// race) and returns a null event — matching `spatial_focus`'s early return.
#[tokio::test]
async fn set_focus_without_snapshot_is_noop() {
    let server = FocusServer::new();
    push_root_layer(&server, "/L", "main").await;

    let res = call_tool(
        &server,
        "set focus",
        json!({ "op": "set focus", "fq": "/L/k1" }),
    )
    .await
    .expect("set focus should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["event"], Value::Null);
}

/// Focusing the already-focused FQM emits no second event — the kernel's
/// "already focused" short-circuit, preserved verbatim.
#[tokio::test]
async fn set_focus_same_fq_twice_emits_once() {
    let server = FocusServer::new();
    push_root_layer(&server, "/L", "main").await;
    let args = json!({ "op": "set focus", "fq": "/L/k1", "snapshot": snapshot_one("/L", "/L/k1") });

    let first = call_tool(&server, "set focus", args.clone()).await.unwrap();
    assert_eq!(first["event"]["next_fq"], json!("/L/k1"));

    let second = call_tool(&server, "set focus", args).await.unwrap();
    assert_eq!(second["event"], Value::Null, "second focus is a no-op");
}

/// `navigate focus` lands on the snapshot-determined target and emits an
/// event for the move — mirroring `spatial_navigate`.
#[tokio::test]
async fn navigate_focus_moves_in_direction() {
    let server = FocusServer::new();
    push_root_layer(&server, "/L", "main").await;

    // Two leaves stacked vertically: k1 on top, k2 below it.
    let snapshot = json!({
        "layer_fq": "/L",
        "scopes": [
            { "fq": "/L/k1", "rect": { "x": 0.0, "y": 0.0, "width": 10.0, "height": 10.0 },
              "parent_zone": null, "nav_override": {} },
            { "fq": "/L/k2", "rect": { "x": 0.0, "y": 20.0, "width": 10.0, "height": 10.0 },
              "parent_zone": null, "nav_override": {} }
        ]
    });

    // Focus k1 first.
    call_tool(
        &server,
        "set focus",
        json!({ "op": "set focus", "fq": "/L/k1", "snapshot": snapshot }),
    )
    .await
    .unwrap();

    // Navigate down → k2.
    let snapshot = json!({
        "layer_fq": "/L",
        "scopes": [
            { "fq": "/L/k1", "rect": { "x": 0.0, "y": 0.0, "width": 10.0, "height": 10.0 },
              "parent_zone": null, "nav_override": {} },
            { "fq": "/L/k2", "rect": { "x": 0.0, "y": 20.0, "width": 10.0, "height": 10.0 },
              "parent_zone": null, "nav_override": {} }
        ]
    });
    let res = call_tool(
        &server,
        "navigate focus",
        json!({
            "op": "navigate focus",
            "focused_fq": "/L/k1",
            "direction": "down",
            "snapshot": snapshot,
        }),
    )
    .await
    .expect("navigate focus should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["event"]["prev_fq"], json!("/L/k1"));
    assert_eq!(res["event"]["next_fq"], json!("/L/k2"));
}

/// `clear focus` drops the per-window slot and emits a `Some(prev) → None`
/// event — mirroring `spatial_clear_focus`.
#[tokio::test]
async fn clear_focus_emits_clearing_event() {
    let server = FocusServer::new();
    push_root_layer(&server, "/L", "main").await;
    call_tool(
        &server,
        "set focus",
        json!({ "op": "set focus", "fq": "/L/k1", "snapshot": snapshot_one("/L", "/L/k1") }),
    )
    .await
    .unwrap();

    let res = call_tool(
        &server,
        "clear focus",
        json!({ "op": "clear focus", "window": "main" }),
    )
    .await
    .expect("clear focus should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["event"]["prev_fq"], json!("/L/k1"));
    assert_eq!(res["event"]["next_fq"], Value::Null);

    // Idempotent: a second clear is a no-op (null event).
    let again = call_tool(
        &server,
        "clear focus",
        json!({ "op": "clear focus", "window": "main" }),
    )
    .await
    .unwrap();
    assert_eq!(again["event"], Value::Null);
}

/// `lose focus` computes a sibling-in-zone fallback when the focused scope
/// unmounts — mirroring `spatial_focus_lost`.
#[tokio::test]
async fn lose_focus_falls_back_to_sibling() {
    let server = FocusServer::new();
    push_root_layer(&server, "/L", "main").await;

    // Focus k1 with both siblings present.
    let full = json!({
        "layer_fq": "/L",
        "scopes": [
            { "fq": "/L/k1", "rect": { "x": 0.0, "y": 0.0, "width": 10.0, "height": 10.0 },
              "parent_zone": null, "nav_override": {} },
            { "fq": "/L/k2", "rect": { "x": 0.0, "y": 20.0, "width": 10.0, "height": 10.0 },
              "parent_zone": null, "nav_override": {} }
        ]
    });
    call_tool(
        &server,
        "set focus",
        json!({ "op": "set focus", "fq": "/L/k1", "snapshot": full }),
    )
    .await
    .unwrap();

    // k1 unmounts: snapshot now only carries k2; the lost FQM's metadata
    // rides alongside.
    let remaining = json!({
        "layer_fq": "/L",
        "scopes": [
            { "fq": "/L/k2", "rect": { "x": 0.0, "y": 20.0, "width": 10.0, "height": 10.0 },
              "parent_zone": null, "nav_override": {} }
        ]
    });
    let res = call_tool(
        &server,
        "lose focus",
        json!({
            "op": "lose focus",
            "focused_fq": "/L/k1",
            "lost_parent_zone": null,
            "lost_layer_fq": "/L",
            "lost_rect": { "x": 0.0, "y": 0.0, "width": 10.0, "height": 10.0 },
            "snapshot": remaining,
        }),
    )
    .await
    .expect("lose focus should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["event"]["prev_fq"], json!("/L/k1"));
    assert_eq!(res["event"]["next_fq"], json!("/L/k2"));
}

/// `push layer` then `pop layer` round-trips: popping returns the layer's
/// recorded `last_focused` restoration target — mirroring `spatial_pop_layer`.
#[tokio::test]
async fn push_then_pop_layer_returns_restoration_target() {
    let server = FocusServer::new();
    push_root_layer(&server, "/L", "main").await;
    // Push a child palette layer.
    call_tool(
        &server,
        "push layer",
        json!({
            "op": "push layer",
            "fq": "/L/palette",
            "segment": "palette",
            "name": "palette",
            "parent": "/L",
            "window": "main",
        }),
    )
    .await
    .unwrap();

    // Focus a scope inside the root layer so the root records last_focused.
    call_tool(
        &server,
        "set focus",
        json!({ "op": "set focus", "fq": "/L/k1", "snapshot": snapshot_one("/L", "/L/k1") }),
    )
    .await
    .unwrap();

    // Popping the root layer surfaces its recorded last_focused.
    let res = call_tool(
        &server,
        "pop layer",
        json!({ "op": "pop layer", "fq": "/L" }),
    )
    .await
    .expect("pop layer should succeed");
    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["next_fq"], json!("/L/k1"));

    // Popping an unknown layer yields a null restoration target.
    let unknown = call_tool(
        &server,
        "pop layer",
        json!({ "op": "pop layer", "fq": "/ghost" }),
    )
    .await
    .unwrap();
    assert_eq!(unknown["next_fq"], Value::Null);
}

/// `drill_in layer` returns the topmost-then-leftmost child of a focused
/// zone — mirroring `spatial_drill_in`.
#[tokio::test]
async fn drill_in_returns_first_child() {
    let server = FocusServer::new();
    push_root_layer(&server, "/L", "main").await;

    // A zone with one child leaf.
    let snapshot = json!({
        "layer_fq": "/L",
        "scopes": [
            { "fq": "/L/zone", "rect": { "x": 0.0, "y": 0.0, "width": 100.0, "height": 100.0 },
              "parent_zone": null, "nav_override": {} },
            { "fq": "/L/zone/leaf", "rect": { "x": 0.0, "y": 0.0, "width": 10.0, "height": 10.0 },
              "parent_zone": "/L/zone", "nav_override": {} }
        ]
    });
    let res = call_tool(
        &server,
        "drill_in layer",
        json!({
            "op": "drill_in layer",
            "fq": "/L/zone",
            "focused_fq": "/L/zone",
            "snapshot": snapshot,
        }),
    )
    .await
    .expect("drill_in layer should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["next_fq"], json!("/L/zone/leaf"));
}

/// `drill_out layer` returns the focused scope's `parent_zone` — mirroring
/// `spatial_drill_out`.
#[tokio::test]
async fn drill_out_returns_parent_zone() {
    let server = FocusServer::new();
    push_root_layer(&server, "/L", "main").await;

    let snapshot = json!({
        "layer_fq": "/L",
        "scopes": [
            { "fq": "/L/zone", "rect": { "x": 0.0, "y": 0.0, "width": 100.0, "height": 100.0 },
              "parent_zone": null, "nav_override": {} },
            { "fq": "/L/zone/leaf", "rect": { "x": 0.0, "y": 0.0, "width": 10.0, "height": 10.0 },
              "parent_zone": "/L/zone", "nav_override": {} }
        ]
    });
    let res = call_tool(
        &server,
        "drill_out layer",
        json!({
            "op": "drill_out layer",
            "fq": "/L/zone/leaf",
            "focused_fq": "/L/zone/leaf",
            "snapshot": snapshot,
        }),
    )
    .await
    .expect("drill_out layer should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["next_fq"], json!("/L/zone"));
}

/// `drill_in` / `drill_out` with no snapshot echo `focused_fq` (transient
/// unmount window) — the Tauri commands' early return.
#[tokio::test]
async fn drill_without_snapshot_echoes_focused() {
    let server = FocusServer::new();

    let din = call_tool(
        &server,
        "drill_in layer",
        json!({ "op": "drill_in layer", "fq": "/L/zone", "focused_fq": "/L/cur" }),
    )
    .await
    .unwrap();
    assert_eq!(din["next_fq"], json!("/L/cur"));

    let dout = call_tool(
        &server,
        "drill_out layer",
        json!({ "op": "drill_out layer", "fq": "/L/zone", "focused_fq": "/L/cur" }),
    )
    .await
    .unwrap();
    assert_eq!(dout["next_fq"], json!("/L/cur"));
}

/// An unknown op surfaces a structured `invalid_params` error.
#[tokio::test]
async fn unknown_op_errors() {
    let server = FocusServer::new();
    let err = call_tool(
        &server,
        "frobnicate focus",
        json!({ "op": "frobnicate focus" }),
    )
    .await
    .expect_err("unknown op should error");
    assert!(
        err.message.contains("frobnicate focus"),
        "error should name the unknown op: {}",
        err.message
    );
}

/// Calling the server with the wrong tool name is rejected.
#[tokio::test]
async fn wrong_tool_name_is_rejected() {
    use rmcp::model::CallToolRequestParams;
    use rmcp::ServerHandler;
    use std::borrow::Cow;

    use super::common::request_context;

    let server = FocusServer::new();
    let request = CallToolRequestParams::new(Cow::Borrowed("not-focus"));
    let err = server
        .call_tool(request, request_context())
        .await
        .expect_err("wrong tool name should error");
    assert!(err.message.contains("not-focus"), "{}", err.message);
}
