//! Undo coverage for writes made through the `entity` MCP server.
//!
//! A write routed through the `EntityServer` goes through `EntityContext`,
//! which pushes onto the shared `StoreContext`. Driving `StoreContext::undo`
//! directly must therefore reverse an `update field` made through the server
//! — proving the server's writes participate in the one undo stack rather
//! than a fork of it.

use serde_json::json;

use super::common::{call_tool, Harness};

/// `update field` via the `entity` server, then `StoreContext::undo`, reverts
/// the field to its prior value.
#[tokio::test]
async fn update_field_then_undo_reverts() {
    let h = Harness::new().await;
    let server = h.server();

    // Create a tag with an initial color.
    call_tool(
        &server,
        "add entity",
        json!({
            "op": "add entity",
            "type": "tag",
            "id": "c1",
            "fields": { "tag_name": "Color", "color": "#111111" },
        }),
    )
    .await
    .unwrap();

    // Mutate the color through the server.
    call_tool(
        &server,
        "update field",
        json!({
            "op": "update field",
            "type": "tag",
            "id": "c1",
            "field": "color",
            "value": "#222222",
        }),
    )
    .await
    .unwrap();

    let mid = call_tool(
        &server,
        "get entity",
        json!({ "op": "get entity", "type": "tag", "id": "c1" }),
    )
    .await
    .unwrap();
    assert_eq!(mid["entity"]["color"], json!("#222222"));

    // Undo the most recent write directly on the shared StoreContext.
    let outcome = h.store_ctx.undo().await.expect("undo should succeed");
    assert_eq!(outcome.store_name, "tag");

    // The kernel rewrote the data file under the cache; reconcile the cache
    // entry (production does this via the UndoCmd layer) before reading back.
    h.entity_ctx.sync_entity_cache_from_disk("tag", "c1").await;

    let after = call_tool(
        &server,
        "get entity",
        json!({ "op": "get entity", "type": "tag", "id": "c1" }),
    )
    .await
    .unwrap();
    assert_eq!(
        after["entity"]["color"],
        json!("#111111"),
        "undo should revert the field to its pre-update value"
    );
}
