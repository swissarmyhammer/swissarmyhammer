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

use serde_json::{json, Value};
use swissarmyhammer_focus::{FocusServer, FullyQualifiedMoniker, WindowLabel};

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

/// `set focus` with an explicit `window` derives the target window from the
/// arg, never from the layer side field. Even if the layer's `window_label`
/// were clobbered (shared root), the explicit `window` must win.
///
/// This pins the "derive window from the explicit arg, stop peeking" contract:
/// the `Focus` op must carry a `window` and thread it to `SpatialState::focus`.
#[tokio::test]
async fn set_focus_honors_explicit_window_over_layer_side_field() {
    let server = FocusServer::new();
    // Deliberately reproduce the OLD shared-root collision: both windows push
    // the literal `/window`, so the registry holds only winB's layer (last
    // push wins) and its `window_label` is "winB".
    push_root_layer(&server, "/window", "winA").await;
    push_root_layer(&server, "/window", "winB").await;

    let card = "/window/board:b/task:t";
    let res = call_tool(
        &server,
        "set focus",
        json!({
            "op": "set focus",
            "fq": card,
            "window": "winA",
            "snapshot": snapshot_one("/window", card),
        }),
    )
    .await
    .expect("set focus should succeed");

    assert_eq!(
        res["event"]["window_label"],
        json!("winA"),
        "explicit window arg must win over the clobbered layer side field",
    );
    assert_eq!(
        focused_in(&server, "winA").await,
        Some(FullyQualifiedMoniker::from_string(card)),
    );
    assert_eq!(
        focused_in(&server, "winB").await,
        None,
        "the clobbered-label window must not receive the focus",
    );
}
