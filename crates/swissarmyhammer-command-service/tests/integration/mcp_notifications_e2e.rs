//! End-to-end tests for the MCP notification bridge's four planes.
//!
//! These tests pin the *correlation model* that the whole command-events
//! project rests on: every notification carries `txn` (the transaction the
//! change belongs to) and `origin` (who caused it), one multi-write command's
//! data changes share that command's `txn`, and an undo emits the inverse
//! batch under a new `txn` with `origin:"undo"`.
//!
//! ## What is real here vs. what is a cross-task seam
//!
//! The bridge itself ([`NotificationBridge`]) is the unit under test: its
//! ingress ([`publish`](swissarmyhammer_plugin::NotificationBridge::publish)),
//! its fan-out to in-process subscribers
//! ([`subscribe`](swissarmyhammer_plugin::NotificationBridge::subscribe)), and
//! its fan-out to external clients
//! ([`forward`](swissarmyhammer_plugin::NotificationBridge::forward)).
//!
//! The *upstream* of the bridge — the per-bus fan-in adapters that subscribe
//! to `EntityEvent` / `ViewEvent` / `PerspectiveEvent` / UI-state changes and
//! normalize them — lives in a higher crate that depends on those domain
//! crates (the platform crate must not depend on them: `swissarmyhammer-views`
//! already depends on it, so the edge would cycle). The
//! `swissarmyhammer-command-service` crate, where this integration suite
//! lives, depends only on `swissarmyhammer-plugin`. So these tests stand in
//! for that fan-in by **publishing the already-normalized notifications a
//! command's writes would produce** — exactly the [`McpNotification`] values
//! the adapters emit — and assert the bridge delivers and correlates them.
//!
//! Likewise the `commands/executed` *emission* is owned by the command
//! engine's txn task (`01KS613VPH2G4ZWKZPGW9ZCJAA`); here we publish a test
//! `commands/executed` into the same delivery seam to prove the bridge fans
//! it out alongside the data changes under the shared `txn`. And the
//! `store/undo_changed` *emission* is owned by the change-propagation task
//! (`01KS5F8THM`); the bridge delivers it through the same seam.

use std::sync::Arc;

use serde_json::json;
use swissarmyhammer_plugin::{
    CallerId, ChangeOp, FieldChange, McpNotification, NotificationBridge, Provenance,
};
use tokio::sync::Mutex as AsyncMutex;

/// Drain every notification currently buffered on `sub` without awaiting new
/// ones, returning them in arrival order.
///
/// The bridge's broadcast channel buffers published notifications until a
/// subscriber reads them, so a test can publish a burst and then collect it
/// synchronously. A `Lagged` would indicate the test overran the channel
/// capacity (it never does at these volumes) and is surfaced as a panic so a
/// regression that floods the channel is loud rather than silently dropping
/// notifications.
fn drain(
    sub: &mut swissarmyhammer_plugin::NotificationSubscription,
) -> Vec<McpNotification> {
    let mut out = Vec::new();
    loop {
        match sub.try_recv() {
            Ok(note) => out.push(note),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
            Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
                panic!("subscriber lagged by {n}; the test overran the bridge channel")
            }
        }
    }
    out
}

/// A multi-write command's data changes all share the command's `txn`, carry
/// `origin:"user"`, and are delivered alongside the command's
/// `commands/executed` action event.
///
/// This is the headline correlation test the card calls for. It models a
/// `task.archive`-style command that, inside one transaction, writes two
/// entities (a task field update + a column reorder) and then emits its
/// action event. A subscribed in-process client must receive:
///
/// - a `commands/executed` for the command (delivery — the engine owns the
///   emission gate; here the test publishes it into the same seam),
/// - two `store/changed` events that share the command's `txn`,
/// - all carrying `origin:"user"`.
#[tokio::test]
async fn multi_write_command_shares_one_txn_and_delivers_commands_executed() {
    let bridge = NotificationBridge::new();
    let mut client = bridge.subscribe();

    // The command service generates one `txn` per execute and threads it via
    // `RequestContext::extensions`; the store server's `current_transaction()`
    // reads it back. Here we stand in for that ambient id with a fixed value.
    let txn = "txn-archive-01";
    let caller = CallerId::HostInternal; // a host/user-initiated command
    let prov = || Provenance::for_caller(&caller, Some(txn));

    // Two entity writes inside the command's transaction — each normalized to
    // the generic `store/changed` schema with field-level `changes`.
    bridge.publish(McpNotification::store_changed(
        "task",
        "01TASK",
        ChangeOp::Updated,
        Some(vec![FieldChange {
            field: "column".to_string(),
            value: json!("done"),
        }]),
        prov(),
    ));
    bridge.publish(McpNotification::store_changed(
        "column",
        "01COL",
        ChangeOp::Updated,
        Some(vec![FieldChange {
            field: "task_ids".to_string(),
            value: json!(["01TASK"]),
        }]),
        prov(),
    ));

    // The command engine's txn task emits the action event; the bridge
    // delivers it. The test publishes it into that delivery seam.
    bridge.publish(McpNotification::commands_executed(
        "task.archive",
        json!({ "scope": ["board"], "target": "task:01TASK" }),
        json!({ "ok": true }),
        prov(),
    ));

    let notes = drain(&mut client);
    assert_eq!(notes.len(), 3, "client should receive all three notifications");

    // (a) The client RECEIVES a `commands/executed`.
    let executed: Vec<_> = notes
        .iter()
        .filter(|n| n.method == "notifications/commands/executed")
        .collect();
    assert_eq!(executed.len(), 1, "exactly one commands/executed delivered");
    assert_eq!(executed[0].params["id"], "task.archive");

    // (b) All `store/changed` events share the command's `txn`.
    let changes: Vec<_> = notes
        .iter()
        .filter(|n| n.method == "notifications/store/changed")
        .collect();
    assert_eq!(changes.len(), 2, "both data changes delivered");
    for change in &changes {
        assert_eq!(
            change.txn(),
            Some(txn),
            "every data change shares the command's txn so the UI coalesces them"
        );
        // (c) origin:"user".
        assert_eq!(change.origin(), Some("user"));
    }

    // The action event shares the same txn, correlating action → data changes.
    assert_eq!(executed[0].txn(), Some(txn));
}

/// Editing a perspective produces a `store/changed{store:"perspective"}` with
/// no `changes` field (reload-item semantics), proving the one generic schema
/// covers non-entity stored things.
#[tokio::test]
async fn perspective_edit_emits_store_changed_without_field_changes() {
    let bridge = NotificationBridge::new();
    let mut client = bridge.subscribe();

    // A perspective write — views/perspectives have no field-level diff today,
    // so the adapter omits `changes` and the client reloads the item.
    bridge.publish(McpNotification::store_changed(
        "perspective",
        "01PERSP",
        ChangeOp::Updated,
        None,
        Provenance::for_caller(&CallerId::HostInternal, Some("txn-persp-01")),
    ));

    let notes = drain(&mut client);
    assert_eq!(notes.len(), 1);
    let note = &notes[0];
    assert_eq!(note.method, "notifications/store/changed");
    assert_eq!(note.params["store"], "perspective");
    assert_eq!(note.params["item"], "01PERSP");
    assert!(
        note.params.get("changes").is_none(),
        "perspectives omit `changes` so the client reload-fetches the item"
    );
    assert_eq!(note.origin(), Some("user"));
}

/// Toggling the command palette produces a `ui_state/changed` — an ephemeral
/// plane distinct from the stored-thing planes, carrying no `txn` (UI state is
/// not undoable).
#[tokio::test]
async fn palette_toggle_emits_ui_state_changed() {
    let bridge = NotificationBridge::new();
    let mut client = bridge.subscribe();

    bridge.publish(McpNotification::ui_state_changed(
        Some("main".to_string()),
        "palette_open",
        json!(true),
    ));

    let notes = drain(&mut client);
    assert_eq!(notes.len(), 1);
    let note = &notes[0];
    assert_eq!(note.method, "notifications/ui_state/changed");
    assert_eq!(note.params["window"], "main");
    assert_eq!(note.params["key"], "palette_open");
    assert_eq!(note.params["value"], true);
    assert!(
        note.txn().is_none(),
        "ephemeral UI state is not a transaction and carries no txn"
    );
}

/// Undoing the command emits the inverse `store/changed` batch sharing one
/// *new* `txn` with `origin:"undo"` — the correlation test for the undo path.
///
/// What this layer proves: given the inverse data changes the undo produces
/// (the change-propagation task stamps them in the reconcile), the bridge
/// delivers them all under one shared new `txn` distinct from the original
/// command's, every one carrying `origin:"undo"`. The `store/undo_changed`
/// stack-state notification is delivered through the same seam.
///
/// Seam: the *generation* of the inverse changes and the `undo`-origin
/// stamping on the upstream structs is owned by `01KS5F8THM`; this test
/// publishes the normalized inverse batch the bridge will receive.
#[tokio::test]
async fn undo_emits_inverse_batch_under_new_txn_with_undo_origin() {
    let bridge = NotificationBridge::new();
    let mut client = bridge.subscribe();

    let original_txn = "txn-archive-01";
    let undo_txn = "txn-undo-07";
    assert_ne!(original_txn, undo_txn);

    // The undo of the two-write command reverses both writes under one new
    // transaction, with origin "undo".
    let undo_prov = || Provenance::new(Some(undo_txn), "undo");
    bridge.publish(McpNotification::store_changed(
        "task",
        "01TASK",
        ChangeOp::Updated,
        Some(vec![FieldChange {
            field: "column".to_string(),
            value: json!("todo"),
        }]),
        undo_prov(),
    ));
    bridge.publish(McpNotification::store_changed(
        "column",
        "01COL",
        ChangeOp::Updated,
        Some(vec![FieldChange {
            field: "task_ids".to_string(),
            value: json!([]),
        }]),
        undo_prov(),
    ));
    // The undo-stack state update is delivered through the same bridge.
    bridge.publish(McpNotification::store_undo_changed(
        false,
        true,
        None,
        Some("Archive".to_string()),
    ));

    let notes = drain(&mut client);

    let changes: Vec<_> = notes
        .iter()
        .filter(|n| n.method == "notifications/store/changed")
        .collect();
    assert_eq!(changes.len(), 2, "both inverse data changes delivered");
    for change in &changes {
        assert_eq!(
            change.txn(),
            Some(undo_txn),
            "the inverse batch shares one new txn distinct from the original command"
        );
        assert_ne!(change.txn(), Some(original_txn));
        assert_eq!(change.origin(), Some("undo"));
    }

    let undo_changed: Vec<_> = notes
        .iter()
        .filter(|n| n.method == "notifications/store/undo_changed")
        .collect();
    assert_eq!(undo_changed.len(), 1, "undo-stack state delivered");
    assert_eq!(undo_changed[0].params["can_undo"], false);
    assert_eq!(undo_changed[0].params["can_redo"], true);
    assert_eq!(undo_changed[0].params["redo_label"], "Archive");
}

/// An external client (a `CliServer`/`UrlServer`-backed agent) receives the
/// same stream as the in-process webview, fanned out through the bridge's
/// `forward` seam.
///
/// The external delivery path is: the transport registers an external
/// subscriber via [`forward`] and supplies a sink that pushes each
/// notification to its connected MCP peer over the wire. Here the sink records
/// what it would push, proving both clients see identical notifications from
/// one `publish`.
///
/// Seam: the concrete sink for a `CliServer` (sending `notification.method` /
/// `notification.params` to the spawned subprocess's peer) is wired where the
/// transport is hosted; this test exercises the bridge's fan-out contract that
/// path depends on.
#[tokio::test]
async fn external_client_receives_the_same_stream_as_in_process() {
    let bridge = NotificationBridge::new();
    let mut in_process = bridge.subscribe();

    // The external transport's sink: record each notification's method, as a
    // real sink would forward it to the agent's MCP peer.
    let pushed = Arc::new(AsyncMutex::new(Vec::<String>::new()));
    let pushed_sink = Arc::clone(&pushed);
    let forwarder = bridge.forward("agent-cli".to_string(), move |note| {
        let pushed = Arc::clone(&pushed_sink);
        async move {
            pushed.lock().await.push(note.method);
        }
    });

    // Wait for the forwarder's subscription to register so the publish reaches
    // it (the in-process subscription is already live).
    for _ in 0..100 {
        if bridge.subscriber_count() == 2 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(
        bridge.subscriber_count(),
        2,
        "both the in-process and external subscribers are registered"
    );

    bridge.publish(McpNotification::store_changed(
        "task",
        "01TASK",
        ChangeOp::Created,
        Some(vec![FieldChange {
            field: "title".to_string(),
            value: json!("New task"),
        }]),
        Provenance::new(Some("txn-x"), "agent:agent-cli"),
    ));

    // The in-process client sees it synchronously.
    let in_proc_notes = drain(&mut in_process);
    assert_eq!(in_proc_notes.len(), 1);
    assert_eq!(in_proc_notes[0].method, "notifications/store/changed");
    assert_eq!(in_proc_notes[0].origin(), Some("agent:agent-cli"));

    // The external sink receives the identical notification.
    let mut external_methods = Vec::new();
    for _ in 0..100 {
        external_methods = pushed.lock().await.clone();
        if !external_methods.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(
        external_methods,
        vec!["notifications/store/changed".to_string()],
        "the external client receives the same stream as the in-process client"
    );

    forwarder.abort();
}
