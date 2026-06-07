//! End-to-end tests for the on-demand UI-geometry query path (Card F2).
//!
//! These exercise the [`UiGeometryProvider`] injection seam — the kernel's
//! way to PULL the live geometry / scope-chain / focus from the webview on
//! demand, instead of receiving them inline on the wire. A fake provider
//! stands in for the real `request_from_ui`-backed app implementation so the
//! kernel logic is covered without a Tauri webview.
//!
//! The card's load-bearing claim: a host-side `navigate` over a PULLED
//! snapshot (no caller-supplied `snapshot`, focus resolved from the kernel's
//! `focus_by_window`) lands on the same target the inline-snapshot path would.

use async_trait::async_trait;
use serde_json::{json, Value};
use swissarmyhammer_focus::{
    FocusServer, FullyQualifiedMoniker, NavSnapshot, UiGeometryProvider, WindowLabel,
};

use super::common::call_tool;

/// A two-leaf snapshot under `/L`: `k1` on top, `k2` directly below it. The
/// provider hands this back verbatim when the kernel pulls geometry for the
/// `main` window, so a downward navigate from `k1` must land on `k2`.
fn two_leaf_snapshot() -> NavSnapshot {
    serde_json::from_value(json!({
        "layer_fq": "/L",
        "scopes": [
            { "fq": "/L/k1", "rect": { "x": 0.0, "y": 0.0, "width": 10.0, "height": 10.0 },
              "parent_zone": null, "nav_override": {} },
            { "fq": "/L/k2", "rect": { "x": 0.0, "y": 20.0, "width": 10.0, "height": 10.0 },
              "parent_zone": null, "nav_override": {} }
        ]
    }))
    .expect("snapshot literal should deserialize")
}

/// A fake provider that answers every pull with fixed, known values — the
/// test double standing in for the real `request_from_ui`-backed app
/// implementation.
struct FakeProvider {
    snapshot: NavSnapshot,
    scope_chain: Vec<FullyQualifiedMoniker>,
    focus: Option<FullyQualifiedMoniker>,
}

#[async_trait]
impl UiGeometryProvider for FakeProvider {
    async fn snapshot(&self, _window: &WindowLabel) -> Option<NavSnapshot> {
        Some(self.snapshot.clone())
    }

    async fn scope_chain(&self, _window: &WindowLabel) -> Vec<FullyQualifiedMoniker> {
        self.scope_chain.clone()
    }

    async fn focus(&self, _window: &WindowLabel) -> Option<FullyQualifiedMoniker> {
        self.focus.clone()
    }
}

/// Build a server wired to a fake provider returning the two-leaf snapshot,
/// a known scope chain, and `k1` as the current focus, with `/L` pushed as
/// the `main` window's root layer.
async fn server_with_fake() -> FocusServer {
    let provider = FakeProvider {
        snapshot: two_leaf_snapshot(),
        scope_chain: vec![
            FullyQualifiedMoniker::from_string("/L"),
            FullyQualifiedMoniker::from_string("/L/k1"),
        ],
        focus: Some(FullyQualifiedMoniker::from_string("/L/k1")),
    };
    let server = FocusServer::new().with_provider(std::sync::Arc::new(provider));
    let res = call_tool(
        &server,
        "push layer",
        json!({
            "op": "push layer",
            "fq": "/L",
            "segment": "window",
            "name": "window",
            "parent": null,
            "window": "main",
        }),
    )
    .await
    .expect("push layer should succeed");
    assert_eq!(res["ok"], json!(true));
    server
}

/// The `query geometry` op pulls the live snapshot from the provider and
/// returns it verbatim.
#[tokio::test]
async fn query_geometry_returns_provider_snapshot() {
    let server = server_with_fake().await;

    let res = call_tool(
        &server,
        "query geometry",
        json!({ "op": "query geometry", "window": "main" }),
    )
    .await
    .expect("query geometry should succeed");

    assert_eq!(res["ok"], json!(true));
    let snapshot: NavSnapshot =
        serde_json::from_value(res["snapshot"].clone()).expect("snapshot in response");
    assert_eq!(snapshot, two_leaf_snapshot());
}

/// The `query scope_chain` op returns the provider's current scope chain.
#[tokio::test]
async fn query_scope_chain_returns_provider_chain() {
    let server = server_with_fake().await;

    let res = call_tool(
        &server,
        "query scope_chain",
        json!({ "op": "query scope_chain", "window": "main" }),
    )
    .await
    .expect("query scope_chain should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["scope_chain"], json!(["/L", "/L/k1"]));
}

/// The `query focus` op returns the provider's current focused FQM.
#[tokio::test]
async fn query_focus_returns_provider_focus() {
    let server = server_with_fake().await;

    let res = call_tool(
        &server,
        "query focus",
        json!({ "op": "query focus", "window": "main" }),
    )
    .await
    .expect("query focus should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["focus"], json!("/L/k1"));
}

/// The make-or-break claim: a host-driven `navigate focus` that supplies NO
/// caller snapshot resolves the current focus from the kernel's
/// `focus_by_window` and PULLS the geometry from the provider — landing on
/// the same target (`k2`) the inline-snapshot path would.
#[tokio::test]
async fn navigate_over_pulled_snapshot_yields_expected_target() {
    let server = server_with_fake().await;

    // Seed the kernel's per-window focus so the host-driven nav resolves
    // `focused_fq` from `focus_by_window["main"]` rather than the wire.
    call_tool(
        &server,
        "set focus",
        json!({ "op": "set focus", "fq": "/L/k1", "snapshot": two_leaf_snapshot() }),
    )
    .await
    .expect("seed focus");

    // Navigate down with NO snapshot and NO focused_fq on the wire: the
    // kernel pulls geometry from the provider and reads focus from its own
    // per-window slot.
    let res = call_tool(
        &server,
        "navigate focus",
        json!({ "op": "navigate focus", "window": "main", "direction": "down" }),
    )
    .await
    .expect("navigate focus should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["event"]["prev_fq"], json!("/L/k1"));
    assert_eq!(res["event"]["next_fq"], json!("/L/k2"));
}

/// Regression for the running-app break (directional nav did nothing): the
/// kernel's per-window focus slot is EMPTY — in the real app React owns focus
/// and the kernel's `focus_by_window` is never populated (set-focus commits
/// drop without a snapshot). A host-driven `navigate focus` with no wire focus
/// must PULL the authoritative current focus from the provider (the UI), NOT
/// rely on the empty kernel slot.
///
/// This deliberately does NOT seed focus via `set focus` (that masked the bug
/// in `navigate_over_pulled_snapshot_yields_expected_target`). Before the
/// provider-focus fallback this dropped with "window has no focused slot" and
/// returned a null event; now it pulls `/L/k1` from the provider and lands on
/// `/L/k2`.
#[tokio::test]
async fn navigate_pulls_focus_from_provider_when_kernel_slot_empty() {
    let server = server_with_fake().await; // provider.focus == /L/k1; kernel slot NOT seeded

    let res = call_tool(
        &server,
        "navigate focus",
        json!({ "op": "navigate focus", "window": "main", "direction": "down" }),
    )
    .await
    .expect("navigate focus should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["event"]["prev_fq"], json!("/L/k1"));
    assert_eq!(res["event"]["next_fq"], json!("/L/k2"));
}

/// When the provider yields no snapshot (window closed / responder absent),
/// a host-driven navigate drops silently with a null event — never panics,
/// never holds a lock across the pull.
#[tokio::test]
async fn navigate_drops_when_provider_has_no_geometry() {
    struct EmptyProvider;
    #[async_trait]
    impl UiGeometryProvider for EmptyProvider {
        async fn snapshot(&self, _window: &WindowLabel) -> Option<NavSnapshot> {
            None
        }
        async fn scope_chain(&self, _window: &WindowLabel) -> Vec<FullyQualifiedMoniker> {
            Vec::new()
        }
        async fn focus(&self, _window: &WindowLabel) -> Option<FullyQualifiedMoniker> {
            None
        }
    }

    let server = FocusServer::new().with_provider(std::sync::Arc::new(EmptyProvider));
    call_tool(
        &server,
        "push layer",
        json!({ "op": "push layer", "fq": "/L", "segment": "window",
                "name": "window", "parent": null, "window": "main" }),
    )
    .await
    .unwrap();
    call_tool(
        &server,
        "set focus",
        json!({ "op": "set focus", "fq": "/L/k1", "snapshot": two_leaf_snapshot() }),
    )
    .await
    .unwrap();

    let res = call_tool(
        &server,
        "navigate focus",
        json!({ "op": "navigate focus", "window": "main", "direction": "down" }),
    )
    .await
    .expect("navigate focus should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["event"], Value::Null);
}
