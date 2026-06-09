//! Two-window isolation tests for the `focus` MCP server.
//!
//! These reproduce the FRONTEND sequence that produced the cross-window
//! focus/nav contamination bug: two windows open on the SAME board, each
//! pushing its own window-root layer, then focusing / navigating a card.
//!
//! The contamination happened because both windows used the LITERAL `/window`
//! root, so the FQM-keyed registry held only the last-pushed window's layer,
//! and the focus op resolved the target window from that clobbered layer's
//! `window_label` side field. The structural fix is a UNIQUE window root
//! (`/<label>/window`) per window so the registry never collides, plus an
//! explicit `window` on the `set focus` op so the kernel derives the window
//! from the path / explicit arg, not the side field.
//!
//! Each test drives the real [`FocusServer`] end-to-end through `call_tool`,
//! exactly as the React `focus-mcp.ts` client does.

use std::sync::Arc;

use serde_json::{json, Value};
use swissarmyhammer_focus::{FocusServer, FullyQualifiedMoniker, RecordingSink, WindowLabel};

use super::common::call_tool;

/// Push a window-root layer at `layer_fq` owned by `window`. Mirrors the
/// React `pushLayer` call for the window-root `<FocusLayer>`.
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

/// Build a single-scope snapshot under `layer_fq` containing `fq`.
fn snapshot_one(layer_fq: &str, fq: &str) -> Value {
    json!({
        "layer_fq": layer_fq,
        "scopes": [
            { "fq": fq, "rect": { "x": 0.0, "y": 0.0, "width": 10.0, "height": 10.0 },
              "parent_zone": null, "nav_override": {} }
        ]
    })
}

/// Build a zone+leaf snapshot under `layer_fq`: `zone` (no parent) and `leaf`
/// (a child of `zone`). Used to exercise `drill_in` (zone → leaf) and
/// `drill_out` (leaf → zone).
fn snapshot_zone_leaf(layer_fq: &str, zone: &str, leaf: &str) -> Value {
    json!({
        "layer_fq": layer_fq,
        "scopes": [
            { "fq": zone, "rect": { "x": 0.0, "y": 0.0, "width": 100.0, "height": 100.0 },
              "parent_zone": null, "nav_override": {} },
            { "fq": leaf, "rect": { "x": 0.0, "y": 0.0, "width": 10.0, "height": 10.0 },
              "parent_zone": zone, "nav_override": {} }
        ]
    })
}

/// Read the focused FQM for `window` directly from the kernel's per-window
/// focus slot.
///
/// Reads `SpatialState::focus_by_window` through the server's shared state
/// arc — NOT the `query focus` op, which pulls from the (Noop in tests) UI
/// geometry provider rather than the kernel slot.
async fn focused_in(server: &FocusServer, window: &str) -> Option<FullyQualifiedMoniker> {
    let state = server.state();
    let guard = state.lock().await;
    guard.focused_in(&WindowLabel::from_string(window)).cloned()
}

/// Two windows on the SAME board, each rooted at its UNIQUE window label
/// (`/winA/window` and `/winB/window`) with the same board sub-path beneath.
/// Focusing a card inside winA's layer must emit an event labelled `winA`,
/// commit focus only in winA, and leave winB's focus slot untouched.
///
/// This is the kernel-side proof that unique roots register without collision
/// and resolve the right window — the prior breakage was a FRONTEND push-fq vs
/// snapshot-layer_fq mismatch, not a kernel limitation.
#[tokio::test]
async fn unique_window_roots_isolate_focus_across_windows() {
    let server = FocusServer::new();
    // Same board (`board:b`) open in two windows, each with a unique root.
    push_root_layer(&server, "/winA/window", "winA").await;
    push_root_layer(&server, "/winB/window", "winB").await;

    let card = "/winA/window/board:b/task:t";
    let res = call_tool(
        &server,
        "set focus",
        json!({
            "op": "set focus",
            "fq": card,
            "snapshot": snapshot_one("/winA/window", card),
        }),
    )
    .await
    .expect("set focus should succeed");

    // (a) the commit is NOT dropped.
    assert_eq!(res["ok"], json!(true));
    assert_ne!(
        res["event"],
        Value::Null,
        "focus commit must not be dropped — the layer must resolve",
    );
    // (b) the emitted event names winA.
    assert_eq!(
        res["event"]["window_label"],
        json!("winA"),
        "event must target the window whose layer the snapshot named",
    );
    assert_eq!(res["event"]["next_fq"], json!(card));

    // (c) focus committed only in winA; winB is untouched.
    assert_eq!(
        focused_in(&server, "winA").await,
        Some(FullyQualifiedMoniker::from_string(card)),
    );
    assert_eq!(
        focused_in(&server, "winB").await,
        None,
        "winB must not receive winA's focus",
    );
}

/// Symmetric: focusing in winB's layer lands in winB, not winA — the
/// second-pushed window is not privileged by the registry.
#[tokio::test]
async fn unique_window_roots_isolate_focus_for_second_window() {
    let server = FocusServer::new();
    push_root_layer(&server, "/winA/window", "winA").await;
    push_root_layer(&server, "/winB/window", "winB").await;

    let card = "/winB/window/board:b/task:t";
    let res = call_tool(
        &server,
        "set focus",
        json!({
            "op": "set focus",
            "fq": card,
            "snapshot": snapshot_one("/winB/window", card),
        }),
    )
    .await
    .expect("set focus should succeed");

    assert_eq!(res["event"]["window_label"], json!("winB"));
    assert_eq!(
        focused_in(&server, "winB").await,
        Some(FullyQualifiedMoniker::from_string(card)),
    );
    assert_eq!(focused_in(&server, "winA").await, None);
}

/// `set focus` derives the owning window from the ROOT SEGMENT of the
/// window-rooted fq — NOT from the explicit `window` arg, which in production
/// is sourced from a broken `require()` lookup and returns the "main"
/// fallback. A card at `/winA/window/...` must emit `window_label == "winA"`
/// even when the wire carries a wrong explicit `window: "main"`.
///
/// This is the regression guard for "jump doesn't clear prior focus":
/// `emit_to("main")` missed the real `board-…` window. The path is
/// authoritative — "when we nav we know where they are".
#[tokio::test]
async fn set_focus_derives_window_from_fq_root_over_wrong_explicit_arg() {
    let server = FocusServer::new();
    push_root_layer(&server, "/winA/window", "winA").await;
    push_root_layer(&server, "/winB/window", "winB").await;

    let card = "/winA/window/board:b/task:t";
    let res = call_tool(
        &server,
        "set focus",
        json!({
            "op": "set focus",
            "fq": card,
            // Wrong explicit window — exactly what the broken frontend
            // `currentWindowLabel()` sends in production.
            "window": "main",
            "snapshot": snapshot_one("/winA/window", card),
        }),
    )
    .await
    .expect("set focus should succeed");

    assert_eq!(
        res["event"]["window_label"],
        json!("winA"),
        "the fq root segment must win over a wrong explicit window arg",
    );
    assert_eq!(
        focused_in(&server, "winA").await,
        Some(FullyQualifiedMoniker::from_string(card)),
    );
    assert_eq!(
        focused_in(&server, "main").await,
        None,
        "the wrong explicit window must not receive the focus",
    );
}

/// A `set focus` that follows a prior focus in the SAME window emits
/// `prev_fq = old` so the prior focus marker clears. The window for both
/// commits is derived from the fq root, so the second commit reconciles
/// against the first in `winA` (not a stale `main` slot).
///
/// Regression guard for "jump doesn't clear prior focus": the misrouted
/// `emit_to("main")` never reached the real window, so the old marker stayed
/// lit alongside the new one (double markers).
#[tokio::test]
async fn set_focus_following_prior_focus_clears_prior_marker() {
    let server = FocusServer::new();
    push_root_layer(&server, "/winA/window", "winA").await;

    let first = "/winA/window/board:b/task:a";
    let second = "/winA/window/board:b/task:b";

    let res1 = call_tool(
        &server,
        "set focus",
        json!({
            "op": "set focus",
            "fq": first,
            "window": "main",
            "snapshot": snapshot_one("/winA/window", first),
        }),
    )
    .await
    .expect("first set focus should succeed");
    assert_eq!(res1["event"]["window_label"], json!("winA"));
    assert_eq!(res1["event"]["prev_fq"], Value::Null);
    assert_eq!(res1["event"]["next_fq"], json!(first));

    let res2 = call_tool(
        &server,
        "set focus",
        json!({
            "op": "set focus",
            "fq": second,
            "window": "main",
            "snapshot": snapshot_one("/winA/window", second),
        }),
    )
    .await
    .expect("second set focus should succeed");
    assert_eq!(res2["event"]["window_label"], json!("winA"));
    assert_eq!(
        res2["event"]["prev_fq"],
        json!(first),
        "the second focus must clear the prior marker via prev_fq",
    );
    assert_eq!(res2["event"]["next_fq"], json!(second));
    assert_eq!(
        focused_in(&server, "winA").await,
        Some(FullyQualifiedMoniker::from_string(second)),
    );
}

/// `drill_in layer` commits focus into the zone's first child and emits a
/// `focus-changed` (via the sink) whose `window_label` is derived from the fq
/// ROOT — `winA`, not the wrong explicit `window: "main"`. Regression guard
/// for "drill-in broke": the misrouted `emit_to("main")` never reached the
/// real window. The drill JSON response carries only `next_fq`; the event is
/// observed through a [`RecordingSink`], exactly as the production wiring
/// forwards it to `emit_to(event.window_label, ...)`.
#[tokio::test]
async fn drill_in_derives_window_from_fq_root_over_wrong_explicit_arg() {
    let sink = Arc::new(RecordingSink::new());
    let server = FocusServer::new().with_sink(sink.clone());
    push_root_layer(&server, "/winA/window", "winA").await;

    let zone = "/winA/window/board:b/zone:z";
    let leaf = "/winA/window/board:b/zone:z/task:t";
    let res = call_tool(
        &server,
        "drill_in layer",
        json!({
            "op": "drill_in layer",
            "fq": zone,
            "focused_fq": zone,
            // Wrong explicit window — what the broken frontend sends.
            "window": "main",
            "snapshot": snapshot_zone_leaf("/winA/window", zone, leaf),
        }),
    )
    .await
    .expect("drill_in layer should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["next_fq"], json!(leaf));

    let events = sink.drain();
    assert_eq!(
        events.len(),
        1,
        "drill-in must emit exactly one focus event"
    );
    assert_eq!(
        events[0].window_label,
        WindowLabel::from_string("winA"),
        "drill-in must emit for the window in the fq root, not the explicit arg",
    );
    assert_eq!(
        events[0].next_fq,
        Some(FullyQualifiedMoniker::from_string(leaf)),
    );
    assert_eq!(
        focused_in(&server, "winA").await,
        Some(FullyQualifiedMoniker::from_string(leaf)),
    );
    assert_eq!(focused_in(&server, "main").await, None);
}

/// `drill_out layer` commits focus to the leaf's `parent_zone` and emits a
/// `focus-changed` (via the sink) whose `window_label` is derived from the fq
/// ROOT — `winA`, not the wrong explicit `window: "main"`. Regression guard
/// for "drill-out broke" under the same misroute.
#[tokio::test]
async fn drill_out_derives_window_from_fq_root_over_wrong_explicit_arg() {
    let sink = Arc::new(RecordingSink::new());
    let server = FocusServer::new().with_sink(sink.clone());
    push_root_layer(&server, "/winA/window", "winA").await;

    let zone = "/winA/window/board:b/zone:z";
    let leaf = "/winA/window/board:b/zone:z/task:t";
    let res = call_tool(
        &server,
        "drill_out layer",
        json!({
            "op": "drill_out layer",
            "fq": leaf,
            "focused_fq": leaf,
            "window": "main",
            "snapshot": snapshot_zone_leaf("/winA/window", zone, leaf),
        }),
    )
    .await
    .expect("drill_out layer should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["next_fq"], json!(zone));

    let events = sink.drain();
    assert_eq!(
        events.len(),
        1,
        "drill-out must emit exactly one focus event"
    );
    assert_eq!(
        events[0].window_label,
        WindowLabel::from_string("winA"),
        "drill-out must emit for the window in the fq root, not the explicit arg",
    );
    assert_eq!(
        events[0].next_fq,
        Some(FullyQualifiedMoniker::from_string(zone)),
    );
    assert_eq!(
        focused_in(&server, "winA").await,
        Some(FullyQualifiedMoniker::from_string(zone)),
    );
    assert_eq!(focused_in(&server, "main").await, None);
}
