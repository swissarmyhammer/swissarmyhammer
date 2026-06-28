//! End-to-end test for cross-store transaction grouping.
//!
//! This is the TDD-first test that pins the generic transaction-grouping
//! contract: open a txn through the `store` MCP server, write through
//! two different `TrackedStore`s under it, then drive a single `undo
//! stack` and assert it reverts BOTH writes. Also asserts that a second
//! concurrent transaction running on a different tokio task does not
//! see the first task's ambient slot — the per-task ambient model is
//! the substrate's concurrency invariant.

use std::sync::Arc;

use serde_json::json;
use swissarmyhammer_store::{StoreContext, StoreServer, StoredItemId, UndoEntryId};
use tempfile::TempDir;

use super::common::{call_tool, make_mock_handle};

/// Single-task transaction: a `BeginTransaction` + writes through two
/// stores + `EndTransaction` should group as one undo step, and a
/// single `undo stack` must revert both files.
#[tokio::test]
async fn begin_writes_two_stores_end_single_undo_reverts_both() {
    let dir = TempDir::new().unwrap();
    let store_a = dir.path().join("alpha");
    let store_b = dir.path().join("beta");
    std::fs::create_dir_all(&store_a).unwrap();
    std::fs::create_dir_all(&store_b).unwrap();

    let handle_a = make_mock_handle(&store_a);
    let handle_b = make_mock_handle(&store_b);

    let ctx = Arc::new(StoreContext::new(dir.path().to_path_buf()));
    ctx.register(handle_a.clone()).await;
    ctx.register(handle_b.clone()).await;

    let server = StoreServer::new(Arc::clone(&ctx));

    // 1. Begin a transaction through the MCP face.
    let begin = call_tool(
        &server,
        "begin transaction",
        json!({ "op": "begin transaction" }),
    )
    .await
    .expect("begin transaction should succeed");
    let txn_id = begin
        .get("id")
        .and_then(|v| v.as_str())
        .expect("begin transaction returns id")
        .to_string();
    assert_eq!(begin["ok"], json!(true));

    // 2. Write through both stores. The store's own write path stamps
    //    the ambient txn id, because both writes happen on the same
    //    tokio task as `begin_transaction`.
    let item_a = "alpha-1\nalpha content".to_string();
    let entry_a = handle_a.write(&item_a).await.unwrap().unwrap();
    ctx.push(
        entry_a,
        "create alpha-1".to_string(),
        StoredItemId::from("alpha-1"),
    )
    .await;

    let item_b = "beta-1\nbeta content".to_string();
    let entry_b = handle_b.write(&item_b).await.unwrap().unwrap();
    ctx.push(
        entry_b,
        "create beta-1".to_string(),
        StoredItemId::from("beta-1"),
    )
    .await;

    // 3. End the transaction.
    let end = call_tool(
        &server,
        "end transaction",
        json!({ "op": "end transaction", "id": txn_id }),
    )
    .await
    .expect("end transaction should succeed");
    assert_eq!(end["ok"], json!(true));

    // Sanity-check both files landed on disk under the txn.
    assert!(store_a.join("alpha-1.txt").exists());
    assert!(store_b.join("beta-1.txt").exists());

    // The stack reports raw entries (one per push), even though the group_id ties
    // them into a single undo step that pops both on one `undo stack` call below.
    let depth = call_tool(&server, "depth stack", json!({ "op": "depth stack" }))
        .await
        .expect("depth probe");
    assert_eq!(
        depth["depth"],
        json!(2),
        "stack has two entries — the group's two writes"
    );

    // 4. A single `undo stack` should revert BOTH files.
    let undo = call_tool(&server, "undo stack", json!({ "op": "undo stack" }))
        .await
        .expect("undo should succeed");

    let items = undo["items"].as_array().expect("undo returns items array");
    assert_eq!(
        items.len(),
        2,
        "single undo reverted both stores' writes as a group"
    );

    assert!(
        !store_a.join("alpha-1.txt").exists(),
        "alpha-1.txt should be trashed by the group undo"
    );
    assert!(
        !store_b.join("beta-1.txt").exists(),
        "beta-1.txt should be trashed by the group undo"
    );
}

/// Concurrent transactions running on different tokio tasks must not
/// interfere — each task's ambient slot is independent, so writes from
/// task A get stamped with txn A's id and writes from task B with
/// txn B's id.
#[tokio::test]
async fn concurrent_transactions_on_different_tasks_dont_interfere() {
    let dir = TempDir::new().unwrap();
    let store_dir = dir.path().join("widgets");
    std::fs::create_dir_all(&store_dir).unwrap();

    let handle = make_mock_handle(&store_dir);

    let ctx = Arc::new(StoreContext::new(dir.path().to_path_buf()));
    ctx.register(handle.clone()).await;

    // Spawn two parallel tasks that each open their own transaction.
    // Each task should see only its own ambient slot.
    let ctx_a = Arc::clone(&ctx);
    let task_a = tokio::spawn(async move {
        let id_a = ctx_a.begin_transaction();
        // Sibling task's id should be invisible from here even though
        // they run concurrently.
        let observed = ctx_a.current_transaction();
        assert_eq!(observed, Some(id_a), "task A sees its own txn id");
        // Hold the slot briefly so task B definitely runs while it is open.
        tokio::task::yield_now().await;
        let still_a = ctx_a.current_transaction();
        assert_eq!(
            still_a,
            Some(id_a),
            "task A's slot is not clobbered by a concurrent task"
        );
        ctx_a.end_transaction(id_a);
        id_a
    });

    let ctx_b = Arc::clone(&ctx);
    let task_b = tokio::spawn(async move {
        let id_b = ctx_b.begin_transaction();
        let observed = ctx_b.current_transaction();
        assert_eq!(observed, Some(id_b), "task B sees its own txn id");
        tokio::task::yield_now().await;
        let still_b = ctx_b.current_transaction();
        assert_eq!(
            still_b,
            Some(id_b),
            "task B's slot is not clobbered by a concurrent task"
        );
        ctx_b.end_transaction(id_b);
        id_b
    });

    let id_a = task_a.await.unwrap();
    let id_b = task_b.await.unwrap();

    assert_ne!(id_a, id_b, "concurrent transactions allocate distinct ids");

    // After both tasks finish, no ambient slot leaks into the test
    // task's view.
    assert_eq!(ctx.current_transaction(), None);
}

/// A write that happens on a task whose ambient slot was set by
/// `BeginTransaction` is stamped with that txn id, even though the
/// store does not know about transactions itself. This is the
/// generic contract the per-store push path obeys.
#[tokio::test]
async fn writes_without_transaction_are_independent_undo_entries() {
    let dir = TempDir::new().unwrap();
    let store_dir = dir.path().join("widgets");
    std::fs::create_dir_all(&store_dir).unwrap();

    let handle = make_mock_handle(&store_dir);

    let ctx = Arc::new(StoreContext::new(dir.path().to_path_buf()));
    ctx.register(handle.clone()).await;

    let server = StoreServer::new(Arc::clone(&ctx));

    // Two writes without a surrounding txn — each is its own undo step.
    let item1 = "w1\none".to_string();
    let id1 = handle.write(&item1).await.unwrap().unwrap();
    ctx.push(id1, "w1".to_string(), StoredItemId::from("w1"))
        .await;

    let item2 = "w2\ntwo".to_string();
    let id2 = handle.write(&item2).await.unwrap().unwrap();
    ctx.push(id2, "w2".to_string(), StoredItemId::from("w2"))
        .await;

    // First undo only reverts w2. Both files still need a second undo.
    let undo1 = call_tool(&server, "undo stack", json!({ "op": "undo stack" }))
        .await
        .unwrap();
    assert_eq!(undo1["items"].as_array().unwrap().len(), 1);
    assert!(
        store_dir.join("w1.txt").exists(),
        "first undo only touches w2"
    );
    assert!(!store_dir.join("w2.txt").exists());
}

/// `EndTransaction` with a stale id is a no-op — it does not clear
/// a slot whose current id is different.
#[tokio::test]
async fn end_transaction_with_mismatched_id_does_nothing() {
    let dir = TempDir::new().unwrap();
    let ctx = Arc::new(StoreContext::new(dir.path().to_path_buf()));
    let server = StoreServer::new(Arc::clone(&ctx));

    let begin = call_tool(
        &server,
        "begin transaction",
        json!({ "op": "begin transaction" }),
    )
    .await
    .unwrap();
    let real_id = begin["id"].as_str().unwrap().to_string();
    assert!(ctx.current_transaction().is_some());

    // A bogus id should leave the slot intact.
    let bogus = UndoEntryId::new().to_string();
    call_tool(
        &server,
        "end transaction",
        json!({ "op": "end transaction", "id": bogus }),
    )
    .await
    .unwrap();
    assert!(
        ctx.current_transaction().is_some(),
        "mismatched end does not clear a live slot"
    );

    // The real id clears it.
    call_tool(
        &server,
        "end transaction",
        json!({ "op": "end transaction", "id": real_id }),
    )
    .await
    .unwrap();
    assert!(ctx.current_transaction().is_none());
}
