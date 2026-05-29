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
//! The headline correlation test
//! ([`multi_write_command_shares_one_txn_and_delivers_commands_executed`])
//! drives the **real** forward-edit path end to end: a real `CommandService`
//! brackets a command's `execute` in a store-backed transaction seam; the
//! command's callback makes REAL forward entity writes through an
//! `EntityCache` wired to one shared `StoreContext`; the real
//! `swissarmyhammer-kanban` fan-in adapter normalizes the resulting
//! `EntityEvent`s into `store/changed` notifications on the bridge; and the
//! real `BridgeActionSink` publishes the `commands/executed` action event.
//! Nothing in the correlation assertion is hand-built — if the ambient txn
//! were not threaded into the forward `store/changed` emission, the two data
//! events would carry `txn: null` and the test would FAIL.
//!
//! The remaining tests still exercise the bridge's *delivery* contract
//! (fan-out to in-process + external subscribers, the
//! `perspective`/`ui_state`/`undo` planes) by publishing the already-
//! normalized notifications the relevant upstream would produce — those are
//! genuinely delivery-only assertions, so a canned publish is faithful.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use swissarmyhammer_command_service::bootstrap::BridgeActionSink;
use swissarmyhammer_command_service::{
    ActionSink, CallbackDispatcher, CallbackHandle, CallbackInvokeError, CommandService,
    TransactionSeam,
};
use swissarmyhammer_entity::test_utils::test_fields_context;
use swissarmyhammer_entity::{Entity, EntityCache, EntityContext, EntityTypeStore};
use swissarmyhammer_kanban::notify_fanin::spawn_notification_fanin;
use swissarmyhammer_plugin::{
    CallerId, ChangeOp, FieldChange, McpNotification, NotificationBridge, Provenance,
};
use swissarmyhammer_store::{StoreContext, StoreHandle, UndoEntryId};
use tempfile::TempDir;
use tokio::sync::Mutex as AsyncMutex;

use super::support::{call_command, execute_args, register_args};

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

/// Store-backed transaction seam over a shared `StoreContext`, mirroring the
/// production seam the kanban app wires: `begin` opens the per-task ambient
/// transaction stamped with the caller-derived `origin`, `end` closes it.
struct StoreTransactionSeam {
    store: Arc<StoreContext>,
}

impl std::fmt::Debug for StoreTransactionSeam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreTransactionSeam").finish()
    }
}

impl TransactionSeam for StoreTransactionSeam {
    fn begin(&self, origin: &str) -> Option<String> {
        Some(
            self.store
                .begin_transaction_with_origin(Some(origin.to_string()))
                .to_string(),
        )
    }

    fn end(&self, txn: &str) {
        if let Ok(id) = txn.parse::<UndoEntryId>() {
            self.store.end_transaction(id);
        }
    }
}

/// A dispatcher whose single `execute` callback makes TWO REAL forward entity
/// writes through the cache-aware `EntityContext`. These run on the same task
/// the engine opened the transaction on (the handler `.await`s inline), so the
/// writes inherit the ambient `txn`+`origin` and the cache emits `EntityEvent`s
/// the kanban fan-in turns into `store/changed`.
struct TwoForwardWriteDispatcher {
    entity: Arc<EntityContext>,
}

impl std::fmt::Debug for TwoForwardWriteDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TwoForwardWriteDispatcher").finish()
    }
}

#[async_trait]
impl CallbackDispatcher for TwoForwardWriteDispatcher {
    async fn invoke(
        &self,
        _handle: &CallbackHandle,
        _args: Value,
    ) -> Result<Value, CallbackInvokeError> {
        let mut a = Entity::new("tag", "alpha");
        a.set("tag_name", json!("Alpha"));
        self.entity
            .write(&a)
            .await
            .map_err(|e| CallbackInvokeError::new(e.to_string()))?;

        let mut b = Entity::new("tag", "bravo");
        b.set("tag_name", json!("Bravo"));
        self.entity
            .write(&b)
            .await
            .map_err(|e| CallbackInvokeError::new(e.to_string()))?;

        Ok(json!({ "wrote": 2 }))
    }
}

/// The booted real substrate the headline test drives.
struct Substrate {
    _dir: TempDir,
    entity: Arc<EntityContext>,
    // Held alive: the cache is the source of the EntityEvents the fan-in
    // forwards; dropping the only strong Arc would sever the path.
    _cache: Arc<EntityCache>,
    store: Arc<StoreContext>,
    bridge: NotificationBridge,
    _fanin: swissarmyhammer_kanban::notify_fanin::NotificationFanin,
}

/// Boot a real entity substrate over one shared `StoreContext`, wire its cache
/// events through the real kanban fan-in into a `NotificationBridge`.
async fn boot_real_substrate() -> Substrate {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    let store = Arc::new(StoreContext::new(root.clone()));

    let fields = test_fields_context();
    let entity = Arc::new(EntityContext::new(&root, fields.clone()));
    let tag_dir = root.join("tags");
    std::fs::create_dir_all(&tag_dir).unwrap();
    let tag_def = fields.get_entity("tag").unwrap();
    let tag_fields: Vec<_> = fields.fields_for_entity("tag").into_iter().cloned().collect();
    let tag_store = EntityTypeStore::new(
        &tag_dir,
        "tag",
        Arc::new(tag_def.clone()),
        Arc::new(tag_fields),
    );
    let tag_handle = Arc::new(StoreHandle::new(Arc::new(tag_store)));
    entity.register_store("tag", Arc::clone(&tag_handle)).await;
    store.register(tag_handle).await;
    entity.set_store_context(Arc::clone(&store));

    // The cache the EntityContext dispatches forward writes through; attach it
    // so `EntityContext::write` routes through the cache's emit path.
    let cache = Arc::new(EntityCache::new(Arc::clone(&entity)));
    entity.attach_cache(&cache);

    let bridge = NotificationBridge::new();
    let fanin = spawn_notification_fanin(
        bridge.clone(),
        Some(cache.subscribe()),
        None,
        None,
        None,
    );
    // Let the forwarder register its subscription before the first write.
    tokio::task::yield_now().await;

    Substrate {
        _dir: dir,
        entity,
        _cache: cache,
        store,
        bridge,
        _fanin: fanin,
    }
}

/// Drain notifications buffered on `client`, giving the async fan-in
/// forwarder a few scheduler turns to deliver (it runs on its own task).
async fn drain_async(
    client: &mut swissarmyhammer_plugin::NotificationSubscription,
) -> Vec<McpNotification> {
    let mut out = Vec::new();
    for _ in 0..200 {
        let mut drained_any = false;
        while let Ok(note) = client.try_recv() {
            out.push(note);
            drained_any = true;
        }
        if !out.is_empty() && !drained_any {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }
    out
}

/// A multi-write command's REAL forward data changes all share the command's
/// `txn` (matching its `commands/executed` event) and carry the caller-derived
/// `origin`.
///
/// This is the headline correlation test the card calls for, driven end to end
/// through the real path — NOT canned publishes:
///
/// 1. A real `CommandService` brackets `execute` in the store-backed
///    [`StoreTransactionSeam`], which opens the ambient txn stamped with the
///    caller-derived origin.
/// 2. The command's callback makes two REAL forward writes through the
///    `EntityContext`/`EntityCache`, on the same task, so they inherit the
///    ambient `txn`+`origin` (the fix under test).
/// 3. The real kanban fan-in normalizes the cache's `EntityEvent`s into
///    `store/changed` notifications on the bridge.
/// 4. The real `BridgeActionSink` publishes the `commands/executed` event.
///
/// The subscribed client must receive two `store/changed` and one
/// `commands/executed`, all sharing one `txn` and `origin:"user"`. If the
/// ambient txn were not threaded into the forward emission, the two data
/// events would carry `txn: null` and this test FAILS.
#[tokio::test]
async fn multi_write_command_shares_one_txn_and_delivers_commands_executed() {
    let substrate = boot_real_substrate().await;
    let mut client = substrate.bridge.subscribe();
    tokio::task::yield_now().await;

    let dispatcher = Arc::new(TwoForwardWriteDispatcher {
        entity: Arc::clone(&substrate.entity),
    });
    let transaction = Arc::new(StoreTransactionSeam {
        store: Arc::clone(&substrate.store),
    });
    // The production action-event delivery seam: publishes commands/executed
    // onto the same bridge the data changes flow through.
    let action_sink: Arc<dyn ActionSink> =
        Arc::new(BridgeActionSink::new(substrate.bridge.clone()));

    let service = CommandService::new()
        .with_dispatcher(dispatcher)
        .with_transaction(transaction)
        .with_action_sink(action_sink);

    // Register a command whose execute callback makes the two forward writes.
    // HostInternal → origin "user".
    call_command(
        &service,
        CallerId::HostInternal,
        register_args("tag.makeTwo", "Make Two Tags", "cb_two"),
    )
    .await;

    // Execute it. The two callback writes happen inside the bracketed txn.
    call_command(&service, CallerId::HostInternal, execute_args("tag.makeTwo")).await;

    let notes = drain_async(&mut client).await;

    // (a) The client RECEIVES a `commands/executed`.
    let executed: Vec<_> = notes
        .iter()
        .filter(|n| n.method == "notifications/commands/executed")
        .collect();
    assert_eq!(
        executed.len(),
        1,
        "exactly one commands/executed delivered; got {notes:?}"
    );
    assert_eq!(executed[0].params["id"], "tag.makeTwo");
    let command_txn = executed[0]
        .txn()
        .expect("the command's action event carries a txn")
        .to_string();

    // (b) Both REAL forward `store/changed` events share the command's `txn`.
    let changes: Vec<_> = notes
        .iter()
        .filter(|n| n.method == "notifications/store/changed")
        .collect();
    assert_eq!(
        changes.len(),
        2,
        "both real forward data changes delivered; got {notes:?}"
    );
    for change in &changes {
        assert_eq!(
            change.txn(),
            Some(command_txn.as_str()),
            "every forward data change must share the command's txn so the UI \
             coalesces them — this is the threaded-ambient-txn contract under test"
        );
        // (c) origin:"user" — the HostInternal caller.
        assert_eq!(change.origin(), Some("user"));
    }
    // Both entities were actually written (real path, not canned).
    let stores: std::collections::HashSet<_> =
        changes.iter().map(|c| c.params["store"].as_str()).collect();
    assert_eq!(stores, std::collections::HashSet::from([Some("tag")]));

    // The transaction was closed: no ambient txn leaks onto the task.
    assert!(
        substrate.store.current_transaction().is_none(),
        "execute must close the transaction it opened"
    );
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
