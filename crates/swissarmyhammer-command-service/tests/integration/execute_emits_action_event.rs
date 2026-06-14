//! On success, `execute` emits `notifications/commands/executed` whose `txn`
//! matches the `store/changed` events the command produced, and whose `origin`
//! reflects the caller.
//!
//! This pins the action-event plane: a subscribed client receives the command's
//! action event correlated (by shared `txn`) with the data changes the command
//! made, so reactive plugins can join action → data.
//!
//! ## What stands in for what
//!
//! The engine OWNS the `commands/executed` emission (the unit under test).
//! The callback's data writes — which a production fan-in would normalize into
//! `store/changed` carrying the ambient `txn` — are modeled here by having the
//! `execute` callback read the open ambient transaction off the shared
//! `StoreContext` and publish the two `store/changed` notifications it would
//! produce onto the same bridge. (This mirrors `mcp_notifications_e2e`, which
//! publishes the already-normalized notifications a command's writes produce.)
//! The point proven: the engine stamps the action event with the SAME `txn` the
//! callback's writes observed, under one shared bridge, with the caller-derived
//! `origin`.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use swissarmyhammer_command_service::{
    ActionSink, CallbackDispatcher, CallbackHandle, CallbackInvokeError, CommandService,
    TransactionSeam,
};
use swissarmyhammer_plugin::{
    CallerId, ChangeOp, FieldChange, McpNotification, NotificationBridge, NotificationSubscription,
    Provenance,
};
use swissarmyhammer_store::{StoreContext, UndoEntryId};
use tempfile::TempDir;

use super::support::{call_command, register_args};

/// Store-backed transaction seam over a shared `StoreContext`.
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

/// Action sink that publishes `commands/executed` onto the bridge — the same
/// `BridgeActionSink` shape the production bootstrap wires.
#[derive(Debug)]
struct BridgeSink {
    bridge: NotificationBridge,
}

impl ActionSink for BridgeSink {
    fn commands_executed(&self, notification: McpNotification) {
        self.bridge.publish(notification);
    }
}

/// A callback that, inside the bracketed transaction, publishes the two
/// `store/changed` notifications its writes would produce — each stamped with
/// the ambient `txn` it reads off the shared `StoreContext`, exactly as a fan-in
/// would after the entity layer stamped the event.
struct EchoWithDataChanges {
    store: Arc<StoreContext>,
    bridge: NotificationBridge,
    caller: CallerId,
}

impl std::fmt::Debug for EchoWithDataChanges {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EchoWithDataChanges").finish()
    }
}

#[async_trait]
impl CallbackDispatcher for EchoWithDataChanges {
    async fn invoke(
        &self,
        _handle: &CallbackHandle,
        _args: Value,
    ) -> Result<Value, CallbackInvokeError> {
        // The command runs inside the engine's bracketed transaction: the
        // ambient txn is set on this task right now.
        let txn = self.store.current_transaction().map(|t| t.to_string());
        let prov = || Provenance::for_caller(&self.caller, txn.clone());

        self.bridge.publish(McpNotification::store_changed(
            "tag",
            "alpha",
            ChangeOp::Created,
            Some(vec![FieldChange {
                field: "tag_name".to_string(),
                value: json!("Alpha"),
            }]),
            prov(),
        ));
        self.bridge.publish(McpNotification::store_changed(
            "tag",
            "bravo",
            ChangeOp::Created,
            Some(vec![FieldChange {
                field: "tag_name".to_string(),
                value: json!("Bravo"),
            }]),
            prov(),
        ));

        Ok(json!({ "echo": "ok" }))
    }
}

/// Drain every notification currently buffered on `sub`.
fn drain(sub: &mut NotificationSubscription) -> Vec<McpNotification> {
    let mut out = Vec::new();
    while let Ok(note) = sub.try_recv() {
        out.push(note);
    }
    out
}

#[tokio::test]
async fn commands_executed_shares_the_txn_of_the_store_changes() {
    let dir = TempDir::new().unwrap();
    let store = Arc::new(StoreContext::new(dir.path().to_path_buf()));
    let bridge = NotificationBridge::new();
    let mut client = bridge.subscribe();

    // An external agent caller so we can assert origin = "agent:<id>".
    let caller = CallerId::External("agent-007".to_string());

    let dispatcher = Arc::new(EchoWithDataChanges {
        store: Arc::clone(&store),
        bridge: bridge.clone(),
        caller: caller.clone(),
    });
    let transaction = Arc::new(StoreTransactionSeam {
        store: Arc::clone(&store),
    });
    let action_sink = Arc::new(BridgeSink {
        bridge: bridge.clone(),
    });

    let service = CommandService::new()
        .with_dispatcher(dispatcher)
        .with_transaction(transaction)
        .with_action_sink(action_sink);

    call_command(
        &service,
        caller.clone(),
        register_args("tag.echo", "Echo", "cb_echo"),
    )
    .await;

    // Execute with a non-trivial ctx so we can check it round-trips into the
    // action event.
    call_command(
        &service,
        caller.clone(),
        json!({
            "op": "execute command",
            "id": "tag.echo",
            "ctx": { "scope_chain": ["board:01B"], "target": "tag:alpha" },
        }),
    )
    .await;

    let notes = drain(&mut client);

    let executed: Vec<_> = notes
        .iter()
        .filter(|n| n.method == "notifications/commands/executed")
        .collect();
    assert_eq!(executed.len(), 1, "exactly one commands/executed delivered");
    let ev = executed[0];
    assert_eq!(ev.params["id"], "tag.echo");
    assert_eq!(ev.params["result"], json!({ "echo": "ok" }));
    // The ctx round-trips into the action event.
    assert_eq!(ev.params["ctx"]["target"], "tag:alpha");
    // origin reflects the (external agent) caller.
    assert_eq!(ev.origin(), Some("agent:agent-007"));

    let changes: Vec<_> = notes
        .iter()
        .filter(|n| n.method == "notifications/store/changed")
        .collect();
    assert_eq!(changes.len(), 2, "both data changes delivered");

    // The action event carries a real txn (the command opened a transaction).
    let action_txn = ev.txn().expect("commands/executed carries the command txn");

    // Every data change shares the command's txn — action ↔ data correlation.
    for change in &changes {
        assert_eq!(
            change.txn(),
            Some(action_txn),
            "every store/changed shares the command's txn so consumers correlate action → data"
        );
        assert_eq!(change.origin(), Some("agent:agent-007"));
    }

    // The transaction was closed.
    assert!(
        store.current_transaction().is_none(),
        "execute closed the transaction after the callback resolved"
    );
}
