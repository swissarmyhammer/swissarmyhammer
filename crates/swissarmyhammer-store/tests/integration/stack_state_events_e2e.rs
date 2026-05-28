//! End-to-end coverage for the undo-stack-state broadcast.
//!
//! `StoreContext` owns the [`UndoStack`] and is the one layer that may emit
//! the stack-state event directly (it carries no foreign types). These tests
//! subscribe to `subscribe_stack_state()` and assert the snapshot that fires
//! on each stack mutation:
//!
//! - `push` fires with `can_undo: true`,
//! - `undo` fires with `can_redo: true`,
//! - `redo` fires with `can_redo: false`,
//! - a **fresh edit after an undo** fires with `can_redo: false` — the
//!   non-symmetric case: `UndoStack::push` truncates the redo tail, so the
//!   Redo control must be turned off by a plain `push`, not just by `redo`.

use swissarmyhammer_store::{StackState, StoreContext, StoredItemId};
use tempfile::TempDir;

use super::common::make_mock_handle;

/// Drain the most recent `StackState` from a receiver, returning the last one
/// available right now (the channel may hold several from a burst).
fn drain_latest(rx: &mut tokio::sync::broadcast::Receiver<StackState>) -> Option<StackState> {
    let mut latest = None;
    while let Ok(state) = rx.try_recv() {
        latest = Some(state);
    }
    latest
}

#[tokio::test]
async fn push_fires_stack_state_with_can_undo_true() {
    let dir = TempDir::new().unwrap();
    let store_dir = dir.path().join("store1");
    std::fs::create_dir_all(&store_dir).unwrap();

    let handle = make_mock_handle(&store_dir);
    let ctx = StoreContext::new(dir.path().to_path_buf());
    ctx.register(handle.clone()).await;

    let mut rx = ctx.subscribe_stack_state();

    let item = "item1\ndata".to_string();
    let entry_id = handle.write(&item).await.unwrap().unwrap();
    ctx.push(entry_id, "create item1".to_string(), StoredItemId::from("item1"))
        .await;

    let state = drain_latest(&mut rx).expect("push must fire a stack-state event");
    assert!(state.can_undo, "after a push, undo must be available");
    assert!(!state.can_redo, "after a push, there is nothing to redo");
    assert_eq!(state.undo_label.as_deref(), Some("create item1"));
}

#[tokio::test]
async fn undo_fires_stack_state_with_can_redo_true() {
    let dir = TempDir::new().unwrap();
    let store_dir = dir.path().join("store1");
    std::fs::create_dir_all(&store_dir).unwrap();

    let handle = make_mock_handle(&store_dir);
    let ctx = StoreContext::new(dir.path().to_path_buf());
    ctx.register(handle.clone()).await;

    let item = "item1\ndata".to_string();
    let entry_id = handle.write(&item).await.unwrap().unwrap();
    ctx.push(entry_id, "create item1".to_string(), StoredItemId::from("item1"))
        .await;

    let mut rx = ctx.subscribe_stack_state();
    ctx.undo().await.unwrap();

    let state = drain_latest(&mut rx).expect("undo must fire a stack-state event");
    assert!(!state.can_undo, "after undoing the only entry, nothing is left to undo");
    assert!(state.can_redo, "after an undo, redo must be available");
    assert_eq!(state.redo_label.as_deref(), Some("create item1"));
}

#[tokio::test]
async fn redo_fires_stack_state_with_can_redo_false() {
    let dir = TempDir::new().unwrap();
    let store_dir = dir.path().join("store1");
    std::fs::create_dir_all(&store_dir).unwrap();

    let handle = make_mock_handle(&store_dir);
    let ctx = StoreContext::new(dir.path().to_path_buf());
    ctx.register(handle.clone()).await;

    let item = "item1\ndata".to_string();
    let entry_id = handle.write(&item).await.unwrap().unwrap();
    ctx.push(entry_id, "create item1".to_string(), StoredItemId::from("item1"))
        .await;
    ctx.undo().await.unwrap();

    let mut rx = ctx.subscribe_stack_state();
    ctx.redo().await.unwrap();

    let state = drain_latest(&mut rx).expect("redo must fire a stack-state event");
    assert!(state.can_undo, "after a redo, the redone entry can be undone again");
    assert!(!state.can_redo, "after redoing the only undone entry, nothing is left to redo");
}

/// The non-symmetric case the card calls out: a plain edit after an undo
/// discards the redo tail, so `can_redo` must flip to false on `push` alone.
#[tokio::test]
async fn fresh_edit_after_undo_fires_stack_state_with_can_redo_false() {
    let dir = TempDir::new().unwrap();
    let store_dir = dir.path().join("store1");
    std::fs::create_dir_all(&store_dir).unwrap();

    let handle = make_mock_handle(&store_dir);
    let ctx = StoreContext::new(dir.path().to_path_buf());
    ctx.register(handle.clone()).await;

    // First edit, then undo it — now redo is available.
    let item1 = "item1\ndata".to_string();
    let id1 = handle.write(&item1).await.unwrap().unwrap();
    ctx.push(id1, "create item1".to_string(), StoredItemId::from("item1"))
        .await;
    ctx.undo().await.unwrap();
    assert!(ctx.can_redo().await, "precondition: redo available after undo");

    // A brand-new edit (a plain push) discards the redo tail.
    let mut rx = ctx.subscribe_stack_state();
    let item2 = "item2\ndata".to_string();
    let id2 = handle.write(&item2).await.unwrap().unwrap();
    ctx.push(id2, "create item2".to_string(), StoredItemId::from("item2"))
        .await;

    let state = drain_latest(&mut rx).expect("a fresh edit after undo must fire a stack-state event");
    assert!(state.can_undo, "the new edit can be undone");
    assert!(
        !state.can_redo,
        "a plain edit after an undo discards the redo tail; can_redo must be false"
    );
    assert_eq!(state.undo_label.as_deref(), Some("create item2"));
}
