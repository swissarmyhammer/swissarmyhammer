//! Transaction-bracketing and action-event seams for the `execute` verb.
//!
//! The `execute` handler wraps the `execute` callback in two seams so the
//! command-as-unit contract holds for **any** command, with zero plugin
//! effort:
//!
//! 1. [`TransactionSeam`] — opens an ambient transaction *before* the
//!    callback and closes it *after* (on success AND error). Every downstream
//!    store write the callback makes inherits the ambient `txn`, so the
//!    command's N writes land in one undo group and tag their emitted
//!    `store/changed` events with the same `txn`.
//! 2. [`ActionSink`] — receives the `notifications/commands/executed` action
//!    event on success, carrying the same `txn` the data changes carried plus
//!    the caller-derived `origin`.
//!
//! Both seams are abstracted so the service core stays Tier-0 (no dependency
//! on `swissarmyhammer-store` and no transport): the platform-integration
//! layer wires real implementations (the store server's
//! `begin_transaction`/`end_transaction` and the
//! [`NotificationBridge`](swissarmyhammer_plugin::NotificationBridge)); tests
//! fake both.
//!
//! # The same-task constraint
//!
//! The store's ambient transaction slot is keyed by tokio task id
//! (`tokio::task::try_id()`): `begin_transaction()` sets the slot for the
//! current task, every `push` on that task reads it back, and
//! `end_transaction()` clears it. So the callback's store writes inherit the
//! ambient `txn` **only when the callback runs on the same tokio task** that
//! opened it. The `execute` handler holds this invariant by `.await`-ing the
//! dispatcher inline — it never `tokio::spawn`s between opening and closing —
//! so the open → callback → close sequence stays pinned to one task.

use std::sync::Arc;

use serde_json::Value;
use swissarmyhammer_operations::Notification;
use swissarmyhammer_plugin::{CallerId, McpNotification, Provenance};

use crate::operations::CommandsExecuted;

/// Opens and closes the ambient transaction that brackets an `execute`.
///
/// The production implementation wraps the `store` server's
/// `StoreContext::begin_transaction()` / `end_transaction()`. Because that
/// ambient slot is per-tokio-task, the seam must be invoked from the same
/// task that runs the callback — the `execute` handler guarantees this by
/// never spawning between [`begin`](Self::begin) and [`end`](Self::end).
pub trait TransactionSeam: Send + Sync + std::fmt::Debug {
    /// Open a transaction on the current task and return its id.
    ///
    /// Returns the freshly allocated transaction id as a string; every store
    /// write the callback makes (on this task) until [`end`](Self::end)
    /// inherits it. The no-op seam returns `None` — there is no store to
    /// group against, and a `None` `txn` flows through `Provenance` to mean
    /// "made outside any transaction".
    ///
    /// `origin` is the caller-derived actor classification
    /// (`"user"` / `"agent:<id>"`, from
    /// [`Provenance::origin_for_caller`](swissarmyhammer_plugin::Provenance::origin_for_caller)).
    /// The store-backed impl stamps it into the ambient slot alongside the
    /// `txn` so a forward entity write the callback makes — reading the slot
    /// back at emit time — reports the same `origin` the command's
    /// `commands/executed` event carries, not a hardcoded `"user"`. The
    /// no-op seam ignores it. Keeping `origin` a `&str` (not a store type) is
    /// what lets the Tier-0 core pass it through without naming
    /// `swissarmyhammer-store`.
    fn begin(&self, origin: &str) -> Option<String>;

    /// Close the transaction `txn` on the current task.
    ///
    /// Idempotent and safe to call even when `begin` returned `None` (then
    /// `txn` is empty and this is a no-op). The `execute` handler calls this
    /// on BOTH the success and error paths so a failed callback never leaks
    /// an open transaction.
    fn end(&self, txn: &str);
}

/// Shared handle to the transaction seam stored on the service.
pub type SharedTransactionSeam = Arc<dyn TransactionSeam>;

/// A transaction seam that grants no transaction.
///
/// The default for [`CommandService::new`](crate::CommandService::new):
/// `begin` returns `None` and `end` is a no-op, so a service with no `store`
/// wiring still runs `execute` end-to-end — the command simply produces no
/// undo group and its action event carries `txn: null`.
#[derive(Debug, Default)]
pub struct NoopTransactionSeam;

impl TransactionSeam for NoopTransactionSeam {
    fn begin(&self, _origin: &str) -> Option<String> {
        None
    }

    fn end(&self, _txn: &str) {}
}

/// Delivers the `notifications/commands/executed` action event on success.
///
/// The production implementation publishes onto the platform's
/// [`NotificationBridge`](swissarmyhammer_plugin::NotificationBridge); tests
/// record the events. Emission is gated by the `execute` handler — the sink
/// only delivers what it is handed.
pub trait ActionSink: Send + Sync + std::fmt::Debug {
    /// Deliver a fully-formed `commands/executed` notification.
    fn commands_executed(&self, notification: McpNotification);
}

/// Shared handle to the action sink stored on the service.
pub type SharedActionSink = Arc<dyn ActionSink>;

/// An action sink that drops every event on the floor.
///
/// The default for [`CommandService::new`](crate::CommandService::new): a
/// service with no notification wiring still runs `execute` end-to-end; the
/// action event simply reaches no subscriber.
#[derive(Debug, Default)]
pub struct NoopActionSink;

impl ActionSink for NoopActionSink {
    fn commands_executed(&self, _notification: McpNotification) {}
}

/// Delivers the debounced `notifications/commands/changed` event when the
/// registry changes.
///
/// The command service's [`ChangeNotifier`](crate::notifications::ChangeNotifier)
/// coalesces a burst of register / unregister / purge mutations into a single
/// emission; that emission is handed here. The production implementation
/// publishes onto the platform's
/// [`NotificationBridge`](swissarmyhammer_plugin::NotificationBridge) so the
/// palette / availability cache refreshes and plugins can react; tests record
/// the events. The struct == payload publish path is the same as
/// [`ActionSink`]: the engine hands a fully-formed [`McpNotification`] built by
/// the declared [`commands_changed_notification`](crate::operations::commands_changed_notification)
/// helper, so the declared schema and the published payload cannot drift.
pub trait NotifierSink: Send + Sync + std::fmt::Debug {
    /// Deliver the debounced `commands/changed` notification.
    fn commands_changed(&self, notification: McpNotification);
}

/// Shared handle to the notifier sink stored on the service.
pub type SharedNotifierSink = Arc<dyn NotifierSink>;

/// A notifier sink that drops every event on the floor.
///
/// The default for [`CommandService::new`](crate::CommandService::new): a
/// service with no notification wiring still runs register / unregister
/// end-to-end; the `commands/changed` event simply reaches no subscriber. The
/// production bootstrap replaces it with one that publishes onto the host's
/// notification bridge.
#[derive(Debug, Default)]
pub struct NoopNotifierSink;

impl NotifierSink for NoopNotifierSink {
    fn commands_changed(&self, _notification: McpNotification) {}
}

/// Build the `commands/executed` notification for one finished `execute`.
///
/// Centralizes the shape so the engine's emission point and any test that
/// checks it agree: `id` is the command id, `ctx` the execution context that
/// was passed in, `result` the callback's return value, and provenance is
/// `txn` (the ambient transaction that bracketed the writes) + the
/// caller-derived `origin`.
pub(crate) fn build_commands_executed(
    id: &str,
    ctx: Value,
    result: Value,
    caller: &CallerId,
    txn: Option<String>,
) -> McpNotification {
    let prov = Provenance::for_caller(caller, txn);
    // The declared `CommandsExecuted` struct IS the payload: serializing it
    // produces the params, so the `_meta` schema and the wire payload share one
    // source. Provenance is stamped on top by `from_declared`.
    let payload = CommandsExecuted {
        id: id.to_string(),
        ctx,
        result,
    };
    McpNotification::from_declared(payload.method(), &payload, prov)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::{command_notifications, commands_changed_notification};
    use std::collections::BTreeSet;
    use swissarmyhammer_operations::generate_notifications_meta;

    /// The set of notification methods this service DECLARES (the "declared"
    /// side of the coverage guard), read from the `_meta` generator.
    fn declared_methods() -> BTreeSet<String> {
        generate_notifications_meta(command_notifications())
            .as_object()
            .expect("notifications meta is an object")
            .values()
            .map(|leaf| {
                leaf["method"]
                    .as_str()
                    .expect("each notification leaf carries a method")
                    .to_string()
            })
            .collect()
    }

    /// The complete set of methods the service's production emission paths
    /// actually raise — the "raised" side of the coverage guard. Each entry is
    /// produced by building the notification through its real helper, NOT a
    /// string literal, so a renamed method is caught here too.
    fn raised_methods() -> BTreeSet<String> {
        let executed = build_commands_executed(
            "task.move",
            serde_json::json!({ "scope_chain": ["task:1"] }),
            serde_json::json!({ "ok": true }),
            &CallerId::HostInternal,
            Some("txn-1".to_string()),
        );
        let changed = commands_changed_notification();
        BTreeSet::from([executed.method, changed.method])
    }

    /// Coverage guard (declared ⟺ raised). The methods the production emission
    /// paths actually publish MUST be exactly the methods the service declares
    /// in `_meta` — so neither `commands/executed` nor `commands/changed` can be
    /// raised without appearing in `_meta`, nor declared without a path
    /// producing it. Every notification-migration card reuses this shape for its
    /// own service.
    #[test]
    fn raised_methods_equal_declared_methods() {
        assert_eq!(
            raised_methods(),
            declared_methods(),
            "the methods this service raises must be exactly the methods it declares in _meta",
        );
        // And the set is exactly the two events this service emits.
        assert_eq!(
            declared_methods(),
            BTreeSet::from([
                "notifications/commands/changed".to_string(),
                "notifications/commands/executed".to_string(),
            ]),
        );
    }

    /// The `commands/changed` helper builds the declared, thin epoch-bump
    /// payload: an empty domain payload plus universal provenance, under the
    /// declared method — proving the struct == payload publish path even when
    /// the struct has no fields.
    #[test]
    fn commands_changed_is_a_thin_provenance_only_payload() {
        let note = commands_changed_notification();
        assert_eq!(note.method, "notifications/commands/changed");
        let params = note.params.as_object().expect("params is an object");
        // Provenance is stamped on top of the (empty) declared payload; there is
        // no per-command enrichment — the consumer refetches the registry.
        assert_eq!(params["origin"], "user");
        // No domain fields beyond provenance: the only keys are the universal
        // correlation fields.
        let domain_keys: Vec<&String> = params
            .keys()
            .filter(|k| k.as_str() != "txn" && k.as_str() != "origin")
            .collect();
        assert!(
            domain_keys.is_empty(),
            "commands/changed carries no per-command payload, got extra keys {domain_keys:?}",
        );
    }

    /// The emitted payload carries the declared domain fields (id/ctx/result)
    /// from the `CommandsExecuted` struct, plus universal provenance — proving
    /// the struct=payload serialization path.
    #[test]
    fn emitted_payload_carries_declared_fields_plus_provenance() {
        let note = build_commands_executed(
            "task.move",
            serde_json::json!({ "k": "v" }),
            serde_json::json!(42),
            &CallerId::HostInternal,
            Some("txn-1".to_string()),
        );
        assert_eq!(note.method, "notifications/commands/executed");
        let params = note.params.as_object().expect("params is an object");
        assert_eq!(params["id"], "task.move");
        assert_eq!(params["ctx"], serde_json::json!({ "k": "v" }));
        assert_eq!(params["result"], serde_json::json!(42));
        // Universal provenance stamped on top of the declared payload.
        assert_eq!(params["txn"], "txn-1");
        assert_eq!(params["origin"], "user");
    }
}
