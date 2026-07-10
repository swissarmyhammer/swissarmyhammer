//! End-to-end tests for the `store` MCP server's stack-wide and
//! store-scoped verbs.
//!
//! Builds a `StoreContext` with a couple of mock stores, wraps it in a
//! `StoreServer`, and exercises every verb the `_meta` tree advertises.

use std::sync::Arc;

use serde_json::json;
use swissarmyhammer_store::{StoreContext, StoreServer, StoredItemId};
use tempfile::TempDir;

use super::common::{call_tool, make_mock_handle};

/// A bare round-trip: write, push, then `undo stack` + `redo stack`.
/// Asserts the file disappears on undo and reappears on redo.
#[tokio::test]
async fn undo_redo_round_trip_over_shared_context() {
    let dir = TempDir::new().unwrap();
    let store_dir = dir.path().join("task");
    std::fs::create_dir_all(&store_dir).unwrap();
    let handle = make_mock_handle(&store_dir);

    let ctx = Arc::new(StoreContext::new(dir.path().to_path_buf()));
    ctx.register(handle.clone()).await;
    let server = StoreServer::new(Arc::clone(&ctx));

    let item = "t1\nthe content".to_string();
    let entry = handle.write(&item).await.unwrap().unwrap();
    ctx.push(entry, "create t1".to_string(), StoredItemId::from("t1"))
        .await;

    // can_undo / can_redo before the round-trip.
    let probe = call_tool(&server, "can_undo stack", json!({ "op": "can_undo stack" }))
        .await
        .unwrap();
    assert_eq!(probe["can_undo"], json!(true));
    let probe = call_tool(&server, "can_redo stack", json!({ "op": "can_redo stack" }))
        .await
        .unwrap();
    assert_eq!(probe["can_redo"], json!(false));

    // undo
    let undo = call_tool(&server, "undo stack", json!({ "op": "undo stack" }))
        .await
        .unwrap();
    assert_eq!(undo["ok"], json!(true));
    assert_eq!(undo["store_name"], json!("task"));
    assert_eq!(undo["item_id"], json!("t1"));
    assert!(!store_dir.join("t1.txt").exists(), "undo trashes the file");

    // redo
    let redo = call_tool(&server, "redo stack", json!({ "op": "redo stack" }))
        .await
        .unwrap();
    assert_eq!(redo["ok"], json!(true));
    assert!(store_dir.join("t1.txt").exists(), "redo restores the file");
}

/// `History { store: "task", item_id }` returns the per-item changelog.
/// Empty when the item has never been written; non-empty (oldest first)
/// after writes land.
#[tokio::test]
async fn history_returns_per_item_changelog() {
    let dir = TempDir::new().unwrap();
    let store_dir = dir.path().join("task");
    std::fs::create_dir_all(&store_dir).unwrap();
    let handle = make_mock_handle(&store_dir);

    let ctx = Arc::new(StoreContext::new(dir.path().to_path_buf()));
    ctx.register(handle.clone()).await;
    let server = StoreServer::new(Arc::clone(&ctx));

    // Empty changelog for an item that doesn't exist.
    let history = call_tool(
        &server,
        "history item",
        json!({ "op": "history item", "store": "task", "item_id": "ghost" }),
    )
    .await
    .unwrap();
    assert_eq!(history["ok"], json!(true));
    assert_eq!(history["entries"].as_array().unwrap().len(), 0);

    // Two writes produce two entries.
    handle.write(&"t1\nv1".to_string()).await.unwrap();
    handle.write(&"t1\nv2".to_string()).await.unwrap();

    let history = call_tool(
        &server,
        "history item",
        json!({ "op": "history item", "store": "task", "item_id": "t1" }),
    )
    .await
    .unwrap();
    let entries = history["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["op"], json!("create"));
    assert_eq!(entries[1]["op"], json!("update"));
}

/// `GetItem { store, item_id }` returns the current bytes; `null` when
/// the item is missing.
#[tokio::test]
async fn get_item_returns_current_bytes_or_null() {
    let dir = TempDir::new().unwrap();
    let store_dir = dir.path().join("task");
    std::fs::create_dir_all(&store_dir).unwrap();
    let handle = make_mock_handle(&store_dir);

    let ctx = Arc::new(StoreContext::new(dir.path().to_path_buf()));
    ctx.register(handle.clone()).await;
    let server = StoreServer::new(Arc::clone(&ctx));

    // Missing item.
    let resp = call_tool(
        &server,
        "get item",
        json!({ "op": "get item", "store": "task", "item_id": "ghost" }),
    )
    .await
    .unwrap();
    assert_eq!(resp["ok"], json!(true));
    assert_eq!(resp["bytes"], json!(null));

    // Existing item.
    handle.write(&"t1\nthe bytes".to_string()).await.unwrap();
    let resp = call_tool(
        &server,
        "get item",
        json!({ "op": "get item", "store": "task", "item_id": "t1" }),
    )
    .await
    .unwrap();
    assert_eq!(resp["bytes"], json!("t1\nthe bytes"));
}

/// `History` against an unknown store name returns a structured error.
#[tokio::test]
async fn unknown_store_name_returns_structured_error() {
    let dir = TempDir::new().unwrap();
    let store_dir = dir.path().join("task");
    std::fs::create_dir_all(&store_dir).unwrap();
    let handle = make_mock_handle(&store_dir);

    let ctx = Arc::new(StoreContext::new(dir.path().to_path_buf()));
    ctx.register(handle).await;
    let server = StoreServer::new(Arc::clone(&ctx));

    let err = call_tool(
        &server,
        "history item",
        json!({ "op": "history item", "store": "does-not-exist", "item_id": "any" }),
    )
    .await
    .expect_err("unknown store should error");

    // The error data should carry the unknown store name so callers
    // can branch on it.
    let data = err.data.as_ref().expect("error carries structured data");
    assert_eq!(data["store"], json!("does-not-exist"));
}

/// `ListStores` returns the names of every registered store, in
/// registration order.
#[tokio::test]
async fn list_stores_returns_registered_names() {
    let dir = TempDir::new().unwrap();
    let store_a = dir.path().join("task");
    let store_b = dir.path().join("column");
    std::fs::create_dir_all(&store_a).unwrap();
    std::fs::create_dir_all(&store_b).unwrap();

    let ctx = Arc::new(StoreContext::new(dir.path().to_path_buf()));
    ctx.register(make_mock_handle(&store_a)).await;
    ctx.register(make_mock_handle(&store_b)).await;
    let server = StoreServer::new(Arc::clone(&ctx));

    let resp = call_tool(&server, "list stores", json!({ "op": "list stores" }))
        .await
        .unwrap();
    let names = resp["stores"].as_array().unwrap();
    assert_eq!(names, &vec![json!("task"), json!("column")]);
}

/// `depth stack` reports the count of entries available to undo.
#[tokio::test]
async fn depth_stack_reflects_pending_entries() {
    let dir = TempDir::new().unwrap();
    let store_dir = dir.path().join("task");
    std::fs::create_dir_all(&store_dir).unwrap();
    let handle = make_mock_handle(&store_dir);

    let ctx = Arc::new(StoreContext::new(dir.path().to_path_buf()));
    ctx.register(handle.clone()).await;
    let server = StoreServer::new(Arc::clone(&ctx));

    let resp = call_tool(&server, "depth stack", json!({ "op": "depth stack" }))
        .await
        .unwrap();
    assert_eq!(resp["depth"], json!(0));

    handle.write(&"t1\nv1".to_string()).await.unwrap();
    ctx.push(
        swissarmyhammer_store::UndoEntryId::new(),
        "noop".to_string(),
        StoredItemId::from("t1"),
    )
    .await;
    let resp = call_tool(&server, "depth stack", json!({ "op": "depth stack" }))
        .await
        .unwrap();
    assert_eq!(resp["depth"], json!(1));
}
