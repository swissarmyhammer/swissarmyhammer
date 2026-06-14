//! Undo coverage for mutations made through the `views` MCP server.
//!
//! A perspective mutation routed through `ViewsServer` goes through
//! `PerspectiveContext`, which pushes onto the shared `StoreContext`. Driving
//! `StoreContext::undo` directly must therefore reverse a `set filter` made
//! through the server — proving the server's writes participate in the one
//! unified changelog rather than a fork of it. This is the load-bearing claim
//! of the card: the `views` server implements no undo of its own.

use serde_json::json;

use super::common::{call_tool, Harness};

/// `set filter` via the `views` server, then `StoreContext::undo`, reverts the
/// filter to its prior (absent) value.
#[tokio::test]
async fn set_filter_then_undo_reverts() {
    let h = Harness::new().await;
    let server = h.server();

    // Create a perspective (undo entry #1).
    let saved = call_tool(
        &server,
        "save perspective",
        json!({ "op": "save perspective", "name": "Undo Me", "view": "board" }),
    )
    .await
    .unwrap();
    let id = saved["perspective"]["id"].as_str().unwrap().to_string();

    // Set a filter (undo entry #2).
    call_tool(
        &server,
        "set filter",
        json!({ "op": "set filter", "perspective_id": id, "filter": "#bug" }),
    )
    .await
    .unwrap();

    let mid = call_tool(
        &server,
        "load perspective",
        json!({ "op": "load perspective", "name": id }),
    )
    .await
    .unwrap();
    assert_eq!(mid["perspective"]["filter"], json!("#bug"));

    // Undo the most recent write directly on the shared StoreContext.
    let outcome = h.store_ctx.undo().await.expect("undo should succeed");
    assert_eq!(outcome.store_name, "perspective");

    // The store layer rewrote the on-disk YAML under the cache; reconcile the
    // cache entry from disk (production does this via the post-undo
    // reconciliation path) before reading back through the server.
    h.perspectives
        .write()
        .await
        .reload_from_disk(&id)
        .await
        .unwrap();

    let after = call_tool(
        &server,
        "load perspective",
        json!({ "op": "load perspective", "name": id }),
    )
    .await
    .unwrap();
    assert!(
        after["perspective"]["filter"].is_null(),
        "undo should revert the filter to its pre-set (absent) value, got {:?}",
        after["perspective"]["filter"]
    );
}
