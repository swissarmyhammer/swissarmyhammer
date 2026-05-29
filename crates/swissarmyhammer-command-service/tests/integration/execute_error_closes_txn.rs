//! `execute` closes its transaction on BOTH paths, and a write-nothing command
//! still emits `commands/executed`.
//!
//! Two guarantees the card calls for:
//! - A callback that ERRORS must still close the transaction the engine opened
//!   — no leaked open `txn` on the task afterward.
//! - A command that writes NOTHING (a pure UI command, say) still emits its
//!   `commands/executed` action event, with an empty undo group and no leaked
//!   transaction.

use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::{json, Value};

use swissarmyhammer_command_service::{
    ActionSink, CallbackDispatcher, CallbackHandle, CallbackInvokeError, CommandService,
    TransactionSeam,
};
use swissarmyhammer_plugin::{CallerId, McpNotification};
use swissarmyhammer_store::{StoreContext, UndoEntryId};
use tempfile::TempDir;

use super::support::{call_command, execute_args, register_args, try_call_command};

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

/// Records every `commands/executed` it is handed.
#[derive(Debug, Default)]
struct RecordingSink {
    seen: Mutex<Vec<McpNotification>>,
}

impl ActionSink for RecordingSink {
    fn commands_executed(&self, notification: McpNotification) {
        self.seen.lock().unwrap().push(notification);
    }
}

/// A callback that always fails (and writes nothing).
#[derive(Debug)]
struct FailingDispatcher;

#[async_trait]
impl CallbackDispatcher for FailingDispatcher {
    async fn invoke(
        &self,
        _handle: &CallbackHandle,
        _args: Value,
    ) -> Result<Value, CallbackInvokeError> {
        Err(CallbackInvokeError::new("callback blew up"))
    }
}

/// A callback that succeeds but writes nothing (a pure command).
#[derive(Debug)]
struct WriteNothingDispatcher;

#[async_trait]
impl CallbackDispatcher for WriteNothingDispatcher {
    async fn invoke(
        &self,
        _handle: &CallbackHandle,
        _args: Value,
    ) -> Result<Value, CallbackInvokeError> {
        Ok(json!({ "ok": true }))
    }
}

#[tokio::test]
async fn callback_error_still_closes_the_transaction() {
    let dir = TempDir::new().unwrap();
    let store = Arc::new(StoreContext::new(dir.path().to_path_buf()));
    let sink = Arc::new(RecordingSink::default());

    let service = CommandService::new()
        .with_dispatcher(Arc::new(FailingDispatcher))
        .with_transaction(Arc::new(StoreTransactionSeam {
            store: Arc::clone(&store),
        }))
        .with_action_sink(Arc::clone(&sink) as Arc<dyn ActionSink>);

    call_command(
        &service,
        CallerId::HostInternal,
        register_args("cmd.boom", "Boom", "cb_boom"),
    )
    .await;

    let result =
        try_call_command(&service, CallerId::HostInternal, execute_args("cmd.boom")).await;
    assert!(result.is_err(), "a failing callback surfaces as an error");

    // The transaction the engine opened was closed despite the error: no
    // ambient txn leaks onto this task.
    assert!(
        store.current_transaction().is_none(),
        "a callback error must still close the transaction — no leaked open txn"
    );

    // No action event on the error path.
    assert!(
        sink.seen.lock().unwrap().is_empty(),
        "a failed execute emits no commands/executed"
    );
}

#[tokio::test]
async fn write_nothing_command_still_emits_commands_executed_with_no_leaked_txn() {
    let dir = TempDir::new().unwrap();
    let store = Arc::new(StoreContext::new(dir.path().to_path_buf()));
    let sink = Arc::new(RecordingSink::default());

    let service = CommandService::new()
        .with_dispatcher(Arc::new(WriteNothingDispatcher))
        .with_transaction(Arc::new(StoreTransactionSeam {
            store: Arc::clone(&store),
        }))
        .with_action_sink(Arc::clone(&sink) as Arc<dyn ActionSink>);

    call_command(
        &service,
        CallerId::HostInternal,
        register_args("ui.palette.open", "Open Palette", "cb_palette"),
    )
    .await;

    call_command(
        &service,
        CallerId::HostInternal,
        execute_args("ui.palette.open"),
    )
    .await;

    // The command wrote nothing — the undo stack stays empty (an empty group
    // is free).
    assert!(
        !store.can_undo().await,
        "a write-nothing command produces an empty undo group"
    );

    // It still emits its action event.
    let seen = sink.seen.lock().unwrap();
    assert_eq!(
        seen.len(),
        1,
        "a write-nothing command still emits commands/executed"
    );
    assert_eq!(seen[0].params["id"], "ui.palette.open");
    // origin reflects the host/user caller.
    assert_eq!(seen[0].origin(), Some("user"));

    // No leaked open transaction.
    assert!(
        store.current_transaction().is_none(),
        "the transaction was closed even though the command wrote nothing"
    );
}
