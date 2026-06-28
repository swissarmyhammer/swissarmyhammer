//! `save perspective` with `if_absent` — the idempotent ensure path the
//! frontend's auto-create-default dispatches through the perspective-commands
//! plugin (live regression 01KTY6T1GPY94VYWANE9X41SKJ).
//!
//! The original ensure fix landed on the legacy `swissarmyhammer-kanban`
//! `SavePerspectiveCmd` / `AddPerspective` path — which production no longer
//! dispatches: the kanban app routes `perspective.save` to the
//! `perspective-commands` TS plugin, which calls THIS server's
//! `save perspective` op. Without ensure semantics here, every app start
//! re-minted a duplicate "Default" pinned to the frontend's `"default"`
//! placeholder view id (observed live: ULID-named Defaults with
//! `view_id: default` accumulating per window per boot).
//!
//! These tests drive the production wire path (`call_tool` → op
//! deserialization → `handle_save`) and pin the ensure contract:
//!
//! - a genuine create lands under the deterministic `default-<scope>` id so
//!   concurrent windows / stale caches converge on ONE file;
//! - an existing perspective for the scope is returned WITHOUT a write;
//! - a `view_id` unknown to the views registry (the `"default"` placeholder)
//!   falls back to the view-kind scope instead of minting a dead-pinned file;
//! - with an empty registry, the filename-safety guard is the backstop.

use serde_json::json;

use super::common::{call_tool, Harness};

/// Save with `if_absent` on an empty board creates the perspective under the
/// deterministic `default-<kind>` id; a second identical save converges on
/// the same file instead of duplicating.
#[tokio::test]
async fn if_absent_save_creates_under_the_deterministic_scope_id() {
    let h = Harness::new().await;
    let server = h.server();

    let args = json!({
        "op": "save perspective",
        "name": "Default",
        "view": "board",
        "if_absent": true,
    });

    let first = call_tool(&server, "save perspective", args.clone())
        .await
        .unwrap();
    assert_eq!(first["ok"], json!(true));
    assert_eq!(
        first["perspective"]["id"],
        json!("default-board"),
        "an ensure-created default must land under the deterministic scope id, got {first}"
    );

    let second = call_tool(&server, "save perspective", args).await.unwrap();
    assert_eq!(
        second["perspective"]["id"],
        json!("default-board"),
        "a repeat ensure must converge on the same perspective, got {second}"
    );

    let listed = call_tool(
        &server,
        "list perspective",
        json!({ "op": "list perspective" }),
    )
    .await
    .unwrap();
    assert_eq!(
        listed["count"],
        json!(1),
        "two if_absent saves must yield exactly one perspective, got {listed}"
    );
}

/// An existing perspective for the target scope short-circuits the create:
/// the ensure returns it and writes nothing.
#[tokio::test]
async fn if_absent_save_returns_existing_scope_match_without_creating() {
    let h = Harness::new().await;
    let server = h.server();

    let existing = call_tool(
        &server,
        "save perspective",
        json!({ "op": "save perspective", "name": "Ready", "view": "board" }),
    )
    .await
    .unwrap();
    let existing_id = existing["perspective"]["id"].as_str().unwrap().to_string();

    let ensured = call_tool(
        &server,
        "save perspective",
        json!({
            "op": "save perspective",
            "name": "Default",
            "view": "board",
            "if_absent": true,
        }),
    )
    .await
    .unwrap();
    assert_eq!(
        ensured["perspective"]["id"],
        json!(existing_id),
        "if_absent must return the existing same-scope perspective, got {ensured}"
    );

    let listed = call_tool(
        &server,
        "list perspective",
        json!({ "op": "list perspective" }),
    )
    .await
    .unwrap();
    assert_eq!(
        listed["count"],
        json!(1),
        "the ensure must not have written a second perspective, got {listed}"
    );
}

/// A `view_id` the views registry does not know (the frontend's `"default"`
/// placeholder when views have not loaded yet) falls back to the view-kind
/// scope — never minting a default pinned to a nonexistent view that the
/// next board-open reconciliation would prune (create/prune churn).
#[tokio::test]
async fn if_absent_save_with_unknown_view_id_falls_back_to_kind_scope() {
    let h = Harness::new().await;
    let server = h.server();

    // A real view exists, so the registry is authoritative.
    let view = call_tool(
        &server,
        "set view",
        json!({ "op": "set view", "name": "Board", "kind": "board", "entity_type": "task" }),
    )
    .await
    .unwrap();
    assert_eq!(view["ok"], json!(true));

    let ensured = call_tool(
        &server,
        "save perspective",
        json!({
            "op": "save perspective",
            "name": "Default",
            "view": "board",
            "view_id": "default",
            "if_absent": true,
        }),
    )
    .await
    .unwrap();
    assert!(
        ensured["perspective"]["view_id"].is_null(),
        "an unknown view_id must fall back to the kind scope, got {ensured}"
    );
    assert_eq!(
        ensured["perspective"]["id"],
        json!("default-board"),
        "the fallback must use the kind-scoped deterministic id, got {ensured}"
    );
}

/// A `view_id` the registry knows pins the ensured default to that view and
/// derives the deterministic id from it.
#[tokio::test]
async fn if_absent_save_with_known_view_id_pins_to_the_view() {
    let h = Harness::new().await;
    let server = h.server();

    let view = call_tool(
        &server,
        "set view",
        json!({ "op": "set view", "name": "Board", "kind": "board", "entity_type": "task" }),
    )
    .await
    .unwrap();
    let view_id = view["view"]["id"].as_str().unwrap().to_string();

    let ensured = call_tool(
        &server,
        "save perspective",
        json!({
            "op": "save perspective",
            "name": "Default",
            "view": "board",
            "view_id": view_id,
            "if_absent": true,
        }),
    )
    .await
    .unwrap();
    assert_eq!(
        ensured["perspective"]["view_id"],
        json!(view_id),
        "a known view_id must stay pinned, got {ensured}"
    );
    assert_eq!(
        ensured["perspective"]["id"],
        json!(format!("default-{view_id}")),
        "the deterministic id must embed the pinned view id, got {ensured}"
    );
}

/// With an EMPTY views registry (bare context) the filename-safety guard is
/// the backstop: a `view_id` carrying path separators must never reach the
/// deterministic `default-<scope>.yaml` filename.
#[tokio::test]
async fn if_absent_save_with_unsafe_view_id_falls_back_when_registry_is_empty() {
    let h = Harness::new().await;
    let server = h.server();

    let ensured = call_tool(
        &server,
        "save perspective",
        json!({
            "op": "save perspective",
            "name": "Default",
            "view": "board",
            "view_id": "../escape",
            "if_absent": true,
        }),
    )
    .await
    .unwrap();
    assert!(
        ensured["perspective"]["view_id"].is_null(),
        "an unsafe view_id must fall back to the kind scope, got {ensured}"
    );
    assert_eq!(
        ensured["perspective"]["id"],
        json!("default-board"),
        "the fallback must use the kind-scoped deterministic id, got {ensured}"
    );
    assert!(
        h.dir
            .path()
            .join("perspectives/default-board.yaml")
            .exists(),
        "the ensured default must persist inside the perspectives dir"
    );
}
