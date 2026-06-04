//! In-process `rmcp::ServerHandler` for the `command` operation tool.
//!
//! [`CommandService`] is the platform-facing surface of the command service.
//! It owns the [`CommandRegistry`] (the override stack) and a debounced
//! [`ChangeNotifier`], advertises a single `command` operation tool whose
//! `inputSchema` and `_meta` are derived from the operation structs in
//! [`crate::operations`], and routes incoming `tools/call`s onto per-verb
//! handler stubs.
//!
//! The verb handlers are intentionally stubs in this layer. Subsequent
//! tasks fill in:
//!
//! - the validation + push-to-registry path for `register` and
//!   `unregister`,
//! - the callback round-trip for `execute` and `available`,
//! - the public projection for `list` and `schema`.
//!
//! The dispatch shape — read `arguments["op"]`, match the verb string,
//! deserialize the rest into the matching operation struct, call the
//! handler — is wired here so subsequent tasks only need to fill in the
//! body of each handler.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Mutex;
use swissarmyhammer_operations_macros::operation_tool;
use swissarmyhammer_plugin::{CallerId, Provenance};

use crate::callbacks::{is_callback_present, CallbackHandle};
use crate::invoke::{NoopCallbackDispatcher, SharedCallbackDispatcher};
use crate::latency::{
    run_with_available_budget, AvailableLatencyOutcome, AVAILABLE_TIMEOUT_REASON,
    AVAILABLE_WARN_THRESHOLD,
};
use crate::lifecycle::{NoopCallerLifecycle, SharedCallerLifecycle};
use crate::notifications::ChangeNotifier;
use crate::operations::{
    command_notifications, operations, AvailableCommand, ExecuteCommand, ListCommand,
    RegisterCommand, SchemaCommand, UnregisterCommand,
};
use crate::registry::{CommandRegistry, StackEntry};
use crate::txn::{
    build_commands_executed, NoopActionSink, NoopTransactionSeam, SharedActionSink,
    SharedTransactionSeam,
};
use crate::types::{CallbackMarker, CommandContext, CommandError, CommandMetadata, CommandSchema};

/// Default debounce window for `notifications/commands/changed`.
///
/// Matches the value documented in `ideas/plugins/command-service.md` —
/// 100ms collapses interactive bursts (a plugin registering ten commands in
/// a row) into one tick while staying well under the human perception
/// threshold for "the palette is stale".
pub const DEFAULT_CHANGE_NOTIFICATION_DEBOUNCE: Duration = Duration::from_millis(100);

/// In-process `rmcp::ServerHandler` for the `command` operation tool.
///
/// Owns the override-stack registry plus the debounced change notifier.
/// Verb handlers live on the impl block; the [`ServerHandler`] impl is the
/// dispatch glue that routes `tools/call("command", { op, … })` onto them.
pub struct CommandService {
    /// The override-stack registry. Held behind an `Arc<Mutex<_>>` so the
    /// register handler can hand a clone to a per-caller unload hook —
    /// the hook outlives the borrow on the service and runs when the
    /// platform's unload path drains the caller's ledger entry.
    registry: Arc<Mutex<CommandRegistry>>,
    /// Debounced emitter for `notifications/commands/changed`. The wired
    /// closure is intentionally a no-op at this layer — the platform's
    /// integration layer (subsequent task) replaces it with one that emits
    /// an rmcp notification to the connected peer.
    notifier: Arc<ChangeNotifier>,
    /// Callback dispatcher used by `execute` / `available` to route a
    /// callback invocation back to the registering caller's isolate.
    /// Defaults to a no-op sink that refuses every dispatch; production
    /// wiring replaces it with the platform's real dispatcher.
    dispatcher: SharedCallbackDispatcher,
    /// Seam used to install per-caller unload hooks on every successful
    /// register. Defaults to a no-op so service-level verb tests run
    /// without a platform; production wiring substitutes a host-aware
    /// implementation that targets the calling plugin's ledger entry.
    lifecycle: SharedCallerLifecycle,
    /// Seam used by `execute` to open/close the ambient transaction that
    /// brackets the callback. Defaults to a no-op that grants no transaction
    /// (a `txn: null` action event, no undo group); production wiring
    /// replaces it with one backed by the `store` server's
    /// `begin_transaction` / `end_transaction`.
    transaction: SharedTransactionSeam,
    /// Seam used by `execute` to deliver the `commands/executed` action event
    /// on success. Defaults to a no-op that drops the event; production
    /// wiring replaces it with one that publishes onto the platform's
    /// notification bridge.
    action_sink: SharedActionSink,
    /// Set of callers we have already installed an unload hook for.
    ///
    /// `handle_register` consults and updates this set so we install at
    /// most one ledger hook per caller, no matter how many commands that
    /// caller registers. Without this set a plugin that registers N
    /// commands would append N opaque entries to the host ledger — the
    /// first hook would purge, the remaining N-1 would be redundant
    /// no-ops that still acquire the registry mutex on unload.
    installed_hooks: Mutex<HashSet<CallerId>>,
}

impl std::fmt::Debug for CommandService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandService")
            .field("registry", &self.registry)
            .field("notifier", &self.notifier)
            .field("dispatcher", &self.dispatcher)
            .field("lifecycle", &self.lifecycle)
            .field("transaction", &self.transaction)
            .field("action_sink", &self.action_sink)
            .field("installed_hooks", &self.installed_hooks)
            .finish()
    }
}

impl Default for CommandService {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandService {
    /// Construct a fresh service with an empty registry, a no-op notifier,
    /// a no-op callback dispatcher, and a no-op caller lifecycle.
    ///
    /// Must be called from inside a tokio runtime (the notifier spawns a
    /// debounce task). Production wiring uses [`Self::with_notifier_sink`],
    /// [`Self::with_dispatcher`], or [`Self::with_lifecycle`] to replace
    /// the no-op seams with the real platform implementations; tests can
    /// use any combination.
    pub fn new() -> Self {
        Self::with_notifier_sink(|| {})
    }

    /// Construct a service whose change notifier invokes `sink` on every
    /// debounced flush.
    ///
    /// The sink is the platform integration seam — subsequent tasks wire
    /// it to an rmcp `notifications/commands/changed` send. Keeping it as
    /// a plain `Fn()` here decouples this crate from the transport layer.
    pub fn with_notifier_sink<F>(sink: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        Self {
            registry: Arc::new(Mutex::new(CommandRegistry::new())),
            notifier: Arc::new(ChangeNotifier::new(
                DEFAULT_CHANGE_NOTIFICATION_DEBOUNCE,
                sink,
            )),
            dispatcher: Arc::new(NoopCallbackDispatcher),
            lifecycle: Arc::new(NoopCallerLifecycle),
            transaction: Arc::new(NoopTransactionSeam),
            action_sink: Arc::new(NoopActionSink),
            installed_hooks: Mutex::new(HashSet::new()),
        }
    }

    /// Replace the callback dispatcher used by `execute` / `available`.
    ///
    /// Returns `self` so the construction can be chained:
    ///
    /// ```ignore
    /// let service = CommandService::new().with_dispatcher(Arc::new(my_dispatcher));
    /// ```
    ///
    /// The dispatcher is the seam between this crate and the plugin
    /// platform: every `execute` / `available` verb call routes the
    /// callback invocation through it back to the registering isolate.
    pub fn with_dispatcher(mut self, dispatcher: SharedCallbackDispatcher) -> Self {
        self.dispatcher = dispatcher;
        self
    }

    /// Replace the caller-lifecycle seam used to install per-register
    /// unload hooks.
    ///
    /// Returns `self` so the construction can be chained. The supplied
    /// lifecycle is invoked once per successful `register command`; the
    /// production wiring uses it to attach a purge hook to the calling
    /// plugin's per-plugin ledger entry, so plugin unload reclaims that
    /// plugin's registrations without the plugin's cooperation.
    pub fn with_lifecycle(mut self, lifecycle: SharedCallerLifecycle) -> Self {
        self.lifecycle = lifecycle;
        self
    }

    /// Replace the transaction seam `execute` uses to bracket the callback.
    ///
    /// Returns `self` so construction can be chained. The supplied seam's
    /// [`begin`](crate::TransactionSeam::begin) is called on the same tokio
    /// task that runs the callback (the handler never spawns between open and
    /// close), so the production seam — backed by the `store` server's
    /// per-task ambient transaction — groups every write the callback makes.
    pub fn with_transaction(mut self, transaction: SharedTransactionSeam) -> Self {
        self.transaction = transaction;
        self
    }

    /// Replace the action sink `execute` publishes `commands/executed` into.
    ///
    /// Returns `self` so construction can be chained. The production sink
    /// publishes onto the platform's notification bridge; tests record the
    /// delivered events.
    pub fn with_action_sink(mut self, action_sink: SharedActionSink) -> Self {
        self.action_sink = action_sink;
        self
    }

    /// Shared handle to the change notifier. Exposed so the platform layer
    /// can call `flush()` on plugin load / unload boundaries without
    /// reaching through the service.
    pub fn notifier(&self) -> Arc<ChangeNotifier> {
        self.notifier.clone()
    }

    /// Purge every registration `caller` ever made from the override stack.
    ///
    /// The bootstrap installs an unload hook on the calling plugin's ledger
    /// the first time it calls `register command`; that hook calls this
    /// method on plugin unload. Direct exposure also lets a host caller (an
    /// external client without a ledger entry) explicitly clean up its own
    /// state.
    ///
    /// Purges are idempotent: a no-op when `caller` has no entries, and
    /// safe to call any number of times. When (and only when) an entry
    /// actually changes, schedules a debounced `commands/changed`
    /// notification and immediately flushes it so subscribers see the
    /// post-purge state without waiting on the debounce window. A duplicate
    /// or empty purge skips both the notify and the flush, so it does not
    /// emit a spurious notification.
    pub fn purge_caller(&self, caller: &CallerId) {
        let changed = {
            let mut registry = self.registry.lock().expect("registry mutex poisoned");
            // Counting total stack entries before/after — rather than the
            // map's `len` — catches the case where purging this caller's
            // entries leaves another caller's entry still active for the
            // same id (the stack shrinks but the id remains).
            let before = registry.total_entries();
            registry.purge_caller(caller);
            registry.total_entries() != before
        };

        if changed {
            self.notifier.notify();
            // Purge is a flush boundary: the active stack may have just
            // changed and subscribers (palette pickers, in particular)
            // must see the post-purge state immediately rather than
            // riding out the debounce window. Both the unload-hook path
            // and direct external callers benefit from the same
            // immediate visibility.
            self.notifier.flush();
        }

        // Clear the install-once bookkeeping so a future caller that
        // happens to recycle this `CallerId` (a plugin reload reuses the
        // same `PluginId`) gets a fresh unload hook installed on its
        // next register. Without this a reloaded plugin's first register
        // would skip the hook install and its eventual unload would not
        // purge.
        self.installed_hooks
            .lock()
            .expect("installed_hooks mutex poisoned")
            .remove(caller);
    }

    /// Build the platform-facing `command` tool definition.
    ///
    /// The `inputSchema` is the flat `op` enum derived from the operation
    /// structs in [`crate::operations`]; the `_meta` tree under
    /// `io.swissarmyhammer/operations` is the discovery surface for the SDK
    /// path sugar. Both come from the same operation slice via the
    /// `operation_tool!` macro, so they cannot drift.
    fn build_tool_definition() -> Tool {
        operation_tool! {
            name: "command",
            description: "Register, execute, and discover user-invocable commands across host and plugin isolates.",
            operations: operations(),
            notifications: command_notifications(),
        }
    }

    /// Register a command.
    ///
    /// Validates the payload (id non-empty, name non-empty, required
    /// `execute` callback marker present), pushes the registration onto
    /// the registry under `caller` (per-caller dedupe replaces this
    /// caller's prior entry for the same id in place), schedules a
    /// debounced `commands/changed` notification, and returns a JSON
    /// envelope describing the active entry.
    ///
    /// # Returns
    ///
    /// `{ "ok": true, "active": <CommandMetadata>, "stack_depth": <usize> }`
    /// on success. The metadata projection omits the [`CallbackMarker`]
    /// fields — those are dispatch-time concerns, not registration
    /// confirmation data.
    ///
    /// # Errors
    ///
    /// - [`CommandError::EmptyId`] when `req.id.is_empty()`.
    /// - [`CommandError::EmptyName`] when `req.name.is_empty()`.
    /// - [`CommandError::MissingExecuteCallback`] when the required
    ///   `execute` marker has an empty `callback_id` (an SDK serializer
    ///   bug — the marker itself is required by the payload type).
    /// - [`CommandError::MissingAvailableCallback`] when an optional
    ///   `available` marker is supplied but its `callback_id` is empty
    ///   (same SDK serializer bug class — an unroutable marker would
    ///   surface later as an opaque dispatch failure, so it is rejected
    ///   at registration time).
    fn handle_register(&self, caller: CallerId, req: RegisterCommand) -> Result<Value, McpError> {
        Self::validate_registration(&req)?;

        let (active_metadata, stack_depth) = {
            let mut registry = self.registry.lock().expect("registry mutex poisoned");
            registry.push(caller.clone(), req.clone());
            let active = registry
                .active(&req.id)
                .expect("registry must hold the entry we just pushed");
            (
                CommandMetadata::from_registration(&active.registration),
                registry.stack_for(&req.id).len(),
            )
        };

        // Install an unload hook on first register from this caller. The
        // platform-integration layer wires this onto the calling plugin's
        // per-plugin ledger so unload purges that plugin's entries without
        // the plugin's cooperation. We dedupe through `installed_hooks` so
        // a plugin that registers N commands appends one opaque entry to
        // the host ledger instead of N — the first hook does the purge,
        // and we avoid N-1 wasted mutex acquisitions on unload.
        self.install_unload_hook_for(&caller);

        self.notifier.notify();

        Ok(serde_json::json!({
            "ok": true,
            "active": active_metadata,
            "stack_depth": stack_depth,
        }))
    }

    /// Hand the lifecycle seam a one-shot dispose hook scoped to `caller`,
    /// at most once per caller.
    ///
    /// The first call for a given `caller` records the caller in
    /// `installed_hooks` and installs a hook that purges the caller on
    /// unload; subsequent calls (additional registers from the same
    /// caller) return immediately without installing a duplicate hook.
    /// The hook owns `Arc` clones of the registry and notifier so it
    /// survives the borrow on the service — the platform's unload path
    /// runs the hook from inside its ledger drain, after the host has
    /// taken the loaded plugin out of its own state map. Purges schedule a
    /// debounced `commands/changed` notification when (and only when) an
    /// entry actually changes, then immediately flush so subscribers see
    /// the post-unload state without waiting on the debounce window.
    ///
    /// The dedupe also bounds per-plugin ledger growth: a plugin that
    /// registers N commands appends exactly one opaque entry to the host
    /// ledger, not N. Purge on unload still runs once and reclaims every
    /// entry the caller ever made.
    fn install_unload_hook_for(&self, caller: &CallerId) {
        let newly_inserted = self
            .installed_hooks
            .lock()
            .expect("installed_hooks mutex poisoned")
            .insert(caller.clone());
        if !newly_inserted {
            return;
        }

        let registry = Arc::clone(&self.registry);
        let notifier = Arc::clone(&self.notifier);
        let hook_caller = caller.clone();
        let hook: crate::lifecycle::UnloadHook = Box::new(move || {
            let changed = {
                let mut guard = registry.lock().expect("registry mutex poisoned");
                let before = guard.total_entries();
                guard.purge_caller(&hook_caller);
                guard.total_entries() != before
            };
            if changed {
                notifier.notify();
                // Unload is a flush boundary: the plugin is gone and the
                // active stack may have changed, so subscribers must see
                // the post-purge state immediately instead of riding out
                // the debounce window.
                notifier.flush();
            }
        });
        self.lifecycle.install_unload_hook(caller, hook);
    }

    /// Unregister this caller's entry for `req.id`.
    ///
    /// Removes the calling caller's entry from the registry stack for
    /// `req.id`, if any. Scheduling a notification only when an entry
    /// actually changes prevents the no-op race case (plugin unload
    /// purges that beat the explicit unregister) from emitting a
    /// spurious `commands/changed`.
    ///
    /// # Returns
    ///
    /// `{ "ok": true, "removed": <bool> }`. `removed` is `false` when the
    /// caller had no entry for the id — by design this is still success,
    /// because explicit unregister can legitimately race with a plugin
    /// unload purge.
    fn handle_unregister(
        &self,
        caller: CallerId,
        req: UnregisterCommand,
    ) -> Result<Value, McpError> {
        let removed = {
            let mut registry = self.registry.lock().expect("registry mutex poisoned");
            registry.pop_caller(&caller, &req.id)
        };

        if removed {
            self.notifier.notify();
        }

        Ok(serde_json::json!({
            "ok": true,
            "removed": removed,
        }))
    }

    /// Validate a registration payload before it touches the registry.
    ///
    /// Pulled out so the handler stays small and the validation rules
    /// have one obvious home. Each rejected payload maps onto a structured
    /// [`CommandError`] variant so downstream callers can branch on the
    /// failure rather than parsing a message.
    fn validate_registration(req: &RegisterCommand) -> Result<(), McpError> {
        if req.id.is_empty() {
            return Err(command_error_to_mcp(CommandError::EmptyId));
        }
        if req.name.is_empty() {
            return Err(command_error_to_mcp(CommandError::EmptyName {
                id: req.id.clone(),
            }));
        }
        if !is_callback_present(&req.execute) {
            return Err(command_error_to_mcp(CommandError::MissingExecuteCallback {
                id: req.id.clone(),
            }));
        }
        // The `available` marker is optional, but when supplied it must
        // carry a routable id for the same reason `execute` does.
        if let Some(available) = req.available.as_ref() {
            if !is_callback_present(available) {
                return Err(command_error_to_mcp(
                    CommandError::MissingAvailableCallback { id: req.id.clone() },
                ));
            }
        }
        Ok(())
    }

    /// Execute a registered command via its `execute` callback.
    ///
    /// Resolves the active stack entry for `req.id` (rejecting
    /// [`CommandError::UnknownCommand`] when missing), optionally rechecks
    /// `available` (skipped only when `req.force` is `Some(true)`), then
    /// invokes the `execute` callback in the registering caller's isolate
    /// via the wired [`CallbackDispatcher`]. The callback's settled return
    /// value is returned to the caller verbatim, wrapped in the standard
    /// `{ ok: true, result: <value> }` envelope.
    ///
    /// # Returns
    ///
    /// `{ "ok": true, "result": <callback return value> }` on success.
    ///
    /// # Errors
    ///
    /// - [`CommandError::UnknownCommand`] when no caller has an active
    ///   registration for `req.id`.
    /// - [`CommandError::CommandUnavailable`] when the recheck of
    ///   `available` returns `false`. `force: true` skips this recheck.
    /// - [`CommandError::CallbackFailed`] when the callback dispatcher
    ///   returns an error (transport failure, callback id not resolvable,
    ///   the function itself threw).
    ///
    /// # Transaction bracketing
    ///
    /// The callback is wrapped in an ambient transaction: a `txn` is opened
    /// via the [`TransactionSeam`](crate::TransactionSeam) **before** the
    /// callback and closed **after** it resolves — on BOTH the success and
    /// error paths, so a failing callback never leaks an open transaction.
    /// Because the store's ambient slot is per-tokio-task and this handler
    /// `.await`s the dispatcher inline (never spawning between open and
    /// close), every store write the callback makes inherits the `txn`: the
    /// command's writes land in one undo group and tag their emitted
    /// `store/changed` events with the same `txn`.
    ///
    /// # Action event
    ///
    /// On success, a `notifications/commands/executed { id, ctx, result, txn,
    /// origin }` is delivered through the
    /// [`ActionSink`](crate::ActionSink). It shares the command's `txn` with
    /// the data changes the command produced (so consumers correlate
    /// action → data) and carries the caller-derived `origin`
    /// (user / agent:id). A command that writes nothing still emits the
    /// action event — its `txn` simply groups an empty undo set.
    async fn handle_execute(
        &self,
        caller: CallerId,
        req: ExecuteCommand,
    ) -> Result<Value, McpError> {
        let active = self.active_entry_snapshot(&req.id)?;

        if !req.force.unwrap_or(false) {
            self.recheck_available_for_execute(&active, &req.ctx, &req.id)
                .await?;
        }

        let execute_handle = CallbackHandle::from_marker(active.caller, &active.execute);
        let args = callback_args_with_ctx(&req.ctx);

        // Open the transaction immediately before the callback so every
        // downstream store write inherits the ambient `txn`. `begin` runs on
        // this task; the inline `.await` below keeps the callback on the same
        // task, so the per-task ambient slot reaches the callback's writes.
        //
        // Pass the caller-derived `origin` through the seam so the store-backed
        // impl stamps it into the ambient slot alongside the `txn`. A forward
        // entity write the callback makes then reads BOTH back at emit time
        // (`StoreContext::current_provenance`), so its `store/changed` carries
        // the same `txn`+`origin` as the `commands/executed` event built below
        // — closing the forward-edit correlation gap.
        let origin = Provenance::origin_for_caller(&caller);
        let txn = self.transaction.begin(&origin);

        let outcome = self.dispatcher.invoke(&execute_handle, args).await;

        // Close the transaction on BOTH paths before returning — a failing
        // callback must not leak an open transaction onto this task.
        if let Some(txn) = txn.as_deref() {
            self.transaction.end(txn);
        }

        let result = outcome.map_err(|err| {
            command_error_to_mcp(CommandError::CallbackFailed {
                message: err.message,
            })
        })?;

        // Success: emit the action event, sharing the command's `txn` with
        // the data changes it produced and stamping the caller-derived origin.
        let ctx_value = serde_json::to_value(&req.ctx).unwrap_or(Value::Null);
        self.action_sink.commands_executed(build_commands_executed(
            &req.id,
            ctx_value,
            result.clone(),
            &caller,
            txn,
        ));

        Ok(serde_json::json!({
            "ok": true,
            "result": result,
        }))
    }

    /// Ask whether a registered command can currently run.
    ///
    /// Resolves the active stack entry for `req.id` (rejecting
    /// [`CommandError::UnknownCommand`] when missing). When the entry has
    /// no `available` callback, returns `{ ok: true }` — the command is
    /// always available. Otherwise invokes the `available` callback under
    /// the soft latency budget defined in [`crate::latency`]:
    ///
    /// - Past [`crate::AVAILABLE_WARN_THRESHOLD`] logs WARN but returns the
    ///   real result.
    /// - Past [`crate::AVAILABLE_HARD_DEADLINE`] is force-cancelled and
    ///   returns `{ ok: false, reason: "available timeout" }`.
    ///
    /// # Returns
    ///
    /// One of:
    /// - `{ "ok": true }` — no `available` callback, or the callback
    ///   returned `true` / a non-`false` value;
    /// - `{ "ok": false, "reason": <string> }` — the callback returned
    ///   `false`, an `{ ok: false, reason }` object, or was force-cancelled
    ///   by the latency budget.
    ///
    /// # Errors
    ///
    /// - [`CommandError::UnknownCommand`] when no caller has an active
    ///   registration for `req.id`.
    /// - [`CommandError::CallbackFailed`] when the callback dispatcher
    ///   returns an error (transport failure, callback id not resolvable,
    ///   the function itself threw).
    async fn handle_available(
        &self,
        _caller: CallerId,
        req: AvailableCommand,
    ) -> Result<Value, McpError> {
        let active = self.active_entry_snapshot(&req.id)?;
        let raw = self.invoke_available(&active, &req.ctx, &req.id).await?;
        Ok(available_response(raw))
    }

    /// Recheck `available` before executing.
    ///
    /// Reuses the same dispatch path as the `available` verb, then
    /// rejects with [`CommandError::CommandUnavailable`] when the result
    /// is `false` / `{ ok: false, reason }` / a timeout. Returning `Ok(())`
    /// means the command may proceed.
    async fn recheck_available_for_execute(
        &self,
        active: &ActiveEntrySnapshot,
        ctx: &CommandContext,
        id: &str,
    ) -> Result<(), McpError> {
        let raw = self.invoke_available(active, ctx, id).await?;
        match interpret_available(raw) {
            AvailableResult::Available => Ok(()),
            AvailableResult::Unavailable { reason } => {
                Err(command_error_to_mcp(CommandError::CommandUnavailable {
                    reason,
                }))
            }
        }
    }

    /// Invoke the `available` callback under the soft latency budget.
    ///
    /// Returns the raw JSON the callback produced (interpreted by the
    /// caller), or the canned `{ ok: false, reason: "available timeout" }`
    /// object on a hard-deadline cancellation. When the entry has no
    /// `available` callback this returns `Value::Bool(true)` — the command
    /// is always available.
    async fn invoke_available(
        &self,
        active: &ActiveEntrySnapshot,
        ctx: &CommandContext,
        id: &str,
    ) -> Result<Value, McpError> {
        let Some(marker) = active.available.as_ref() else {
            return Ok(Value::Bool(true));
        };

        let handle = CallbackHandle::from_marker(active.caller.clone(), marker);
        let args = callback_args_with_ctx(ctx);
        let dispatcher = Arc::clone(&self.dispatcher);
        let invocation = async move { dispatcher.invoke(&handle, args).await };

        match run_with_available_budget(invocation).await {
            AvailableLatencyOutcome::Completed { result, elapsed } => {
                if elapsed > AVAILABLE_WARN_THRESHOLD {
                    tracing::warn!(
                        command_id = %id,
                        elapsed_ms = elapsed.as_millis() as u64,
                        "available callback exceeded warn threshold",
                    );
                }
                result.map_err(|err| {
                    command_error_to_mcp(CommandError::CallbackFailed {
                        message: err.message,
                    })
                })
            }
            AvailableLatencyOutcome::TimedOut { elapsed } => {
                tracing::warn!(
                    command_id = %id,
                    elapsed_ms = elapsed.as_millis() as u64,
                    "available callback force-cancelled by latency budget",
                );
                Ok(serde_json::json!({
                    "ok": false,
                    "reason": AVAILABLE_TIMEOUT_REASON,
                }))
            }
        }
    }

    /// Capture an [`ActiveEntrySnapshot`] for `id`, or fail with
    /// [`CommandError::UnknownCommand`].
    ///
    /// Pulled out so the verb handlers do not hold the registry lock
    /// across `await` points: the snapshot copies just the fields the
    /// dispatch path needs (caller + callback markers) and releases the
    /// guard immediately.
    fn active_entry_snapshot(&self, id: &str) -> Result<ActiveEntrySnapshot, McpError> {
        self.with_registry(|registry| {
            registry
                .active(id)
                .map(|entry| ActiveEntrySnapshot {
                    caller: entry.caller.clone(),
                    execute: entry.registration.execute.clone(),
                    available: entry.registration.available.clone(),
                })
                .ok_or_else(|| {
                    command_error_to_mcp(CommandError::UnknownCommand { id: id.to_string() })
                })
        })
    }

    /// List active (top-of-stack) commands matching the supplied filters.
    ///
    /// Filters intersect — every filter that is `Some(_)` must match. A
    /// command's `scope` field is treated as a membership test: empty /
    /// absent means "global" (matches every `scope` filter), otherwise the
    /// filter must appear in the registered scope vec. `category` is exact
    /// match; `id_prefix` is `starts_with`.
    ///
    /// # Returns
    ///
    /// `{ "ok": true, "commands": [<CommandMetadata>, …] }`. The metadata
    /// projection omits the [`CallbackMarker`] fields — those are
    /// dispatch-time concerns, not discovery data.
    ///
    /// This verb does not invoke any callbacks; it is a pure registry read.
    fn handle_list(&self, _caller: CallerId, req: ListCommand) -> Result<Value, McpError> {
        let commands: Vec<CommandMetadata> = self.with_registry(|registry| {
            registry
                .list()
                .into_iter()
                .filter(|entry| Self::list_filter_matches(entry, &req))
                .map(|entry| CommandMetadata::from_registration(&entry.registration))
                .collect()
        });

        Ok(serde_json::json!({
            "ok": true,
            "commands": commands,
        }))
    }

    /// Return whether `entry` passes every filter that is `Some(_)` in `req`.
    ///
    /// Pulled out so the handler stays small and the filter rules have one
    /// obvious home. All three filters compose as a logical AND — an entry
    /// is kept only when every supplied filter matches.
    fn list_filter_matches(entry: &StackEntry, req: &ListCommand) -> bool {
        let registration = &entry.registration;

        if let Some(expected_scope) = req.scope.as_deref() {
            // A command's scope chain is "global" when the field is absent
            // or empty — global commands match every scope filter. When the
            // field is populated, the filter must appear in the vec.
            let scope_matches = match registration.scope.as_deref() {
                None | Some([]) => true,
                Some(scopes) => scopes.iter().any(|s| s == expected_scope),
            };
            if !scope_matches {
                return false;
            }
        }

        if let Some(expected_category) = req.category.as_deref() {
            if registration.category.as_deref() != Some(expected_category) {
                return false;
            }
        }

        if let Some(prefix) = req.id_prefix.as_deref() {
            if !registration.id.starts_with(prefix) {
                return false;
            }
        }

        true
    }

    /// Return the param schema for one registered command.
    ///
    /// # Returns
    ///
    /// `{ "ok": true, "schema": <CommandSchema> }` when the id is registered.
    /// The `CommandSchema` wrapper carries the command id alongside the
    /// `params` array so the wire shape can grow new fields without
    /// breaking palette / popover callers.
    ///
    /// # Errors
    ///
    /// [`CommandError::UnknownCommand`] when no caller has an active
    /// registration for `req.id`.
    ///
    /// This verb does not invoke any callbacks; it is a pure registry read.
    fn handle_schema(&self, _caller: CallerId, req: SchemaCommand) -> Result<Value, McpError> {
        let schema = self.with_registry(|registry| {
            registry
                .active(&req.id)
                .map(|entry| CommandSchema::from_registration(&entry.registration))
        });

        let schema = schema.ok_or_else(|| {
            command_error_to_mcp(CommandError::UnknownCommand { id: req.id.clone() })
        })?;

        Ok(serde_json::json!({
            "ok": true,
            "schema": schema,
        }))
    }

    /// Read-only view of the registry. Used by integration callers and
    /// tests that need to assert on the stack from outside.
    ///
    /// Acquires the internal mutex; do not hold across `await` points.
    pub fn with_registry<R>(&self, f: impl FnOnce(&CommandRegistry) -> R) -> R {
        let guard = self.registry.lock().expect("registry mutex poisoned");
        f(&guard)
    }

    /// Dispatch one `execute` request directly from Rust, returning the
    /// callback's result value.
    ///
    /// This is the production entrypoint for the kanban app's Tauri
    /// `dispatch_command` handler: with an `Arc<CommandService>` in hand,
    /// the handler scopes its per-board task-locals (see
    /// `swissarmyhammer-kanban`'s `scope_store_context`) and calls
    /// `service.dispatch(caller, req).await` — bypassing the
    /// `call_tool` → rmcp dispatch hop that external MCP clients take.
    ///
    /// Wraps the internal `handle_execute` verbatim: the same
    /// [`TransactionSeam::begin`] / [`TransactionSeam::end`] bracketing,
    /// the same callback invocation through the registered dispatcher,
    /// the same `commands/executed` action-event emission on success.
    /// No behavior difference — just a public surface so in-process
    /// callers don't have to go through the MCP call-tool plumbing.
    ///
    /// [`TransactionSeam::begin`]: crate::TransactionSeam::begin
    /// [`TransactionSeam::end`]: crate::TransactionSeam::end
    pub async fn dispatch(&self, caller: CallerId, req: ExecuteCommand) -> Result<Value, McpError> {
        self.handle_execute(caller, req).await
    }
}

/// Map a JSON value into one of the six operation structs, returning a
/// readable rmcp error when the shape is wrong.
fn deserialize_op<T: DeserializeOwned>(arguments: Value, op: &str) -> Result<T, McpError> {
    serde_json::from_value(arguments).map_err(|err| {
        McpError::invalid_params(format!("invalid arguments for op {op:?}: {err}"), None)
    })
}

/// Map a [`CommandError`] onto a structured [`McpError`] suitable for the
/// `tools/call` response.
///
/// The error code is `invalid_params` for client-recoverable shape failures
/// (the four registration-validation variants) and `internal_error` for
/// the dispatch-time variants (`UnknownCommand`, `CommandUnavailable`,
/// `CallbackFailed`, `LatencyBudgetExceeded`). The `data` field carries
/// the JSON-shaped variant for callers who want to branch on the
/// discriminant rather than the human-readable `message`.
fn command_error_to_mcp(err: CommandError) -> McpError {
    let message = err.to_string();
    let data = command_error_data(&err);
    match err {
        CommandError::EmptyId
        | CommandError::EmptyName { .. }
        | CommandError::MissingExecuteCallback { .. }
        | CommandError::MissingAvailableCallback { .. } => {
            McpError::invalid_params(message, Some(data))
        }
        CommandError::UnknownCommand { .. }
        | CommandError::CommandUnavailable { .. }
        | CommandError::CallbackFailed { .. }
        | CommandError::LatencyBudgetExceeded { .. } => {
            McpError::internal_error(message, Some(data))
        }
    }
}

/// Project a [`CommandError`] onto a JSON object suitable for the
/// `data` field of an rmcp error response.
///
/// The shape is `{ "kind": "<VariantName>", ...fields }` so callers can
/// branch on `kind` and grab the structured fields without parsing the
/// message string.
fn command_error_data(err: &CommandError) -> Value {
    match err {
        CommandError::UnknownCommand { id } => {
            serde_json::json!({ "kind": "UnknownCommand", "id": id })
        }
        CommandError::CommandUnavailable { reason } => {
            serde_json::json!({ "kind": "CommandUnavailable", "reason": reason })
        }
        CommandError::CallbackFailed { message } => {
            serde_json::json!({ "kind": "CallbackFailed", "message": message })
        }
        CommandError::LatencyBudgetExceeded { id } => {
            serde_json::json!({ "kind": "LatencyBudgetExceeded", "id": id })
        }
        CommandError::EmptyId => serde_json::json!({ "kind": "EmptyId" }),
        CommandError::EmptyName { id } => {
            serde_json::json!({ "kind": "EmptyName", "id": id })
        }
        CommandError::MissingExecuteCallback { id } => {
            serde_json::json!({ "kind": "MissingExecuteCallback", "id": id })
        }
        CommandError::MissingAvailableCallback { id } => {
            serde_json::json!({ "kind": "MissingAvailableCallback", "id": id })
        }
    }
}

/// Minimal copy of the dispatch-time fields the verb handlers need from an
/// active [`StackEntry`].
///
/// The handlers must not hold the registry mutex across `await` points —
/// `Mutex` is `!Send` after lock and the dispatcher invocation is async —
/// so they snapshot the three fields they actually use and release the lock
/// immediately.
struct ActiveEntrySnapshot {
    /// The caller that registered the active entry. Used to route callback
    /// invocations back to the originating isolate.
    caller: CallerId,
    /// The required `execute` callback marker.
    execute: CallbackMarker,
    /// The optional `available` callback marker. `None` means the command
    /// is always available.
    available: Option<CallbackMarker>,
}

/// Interpretation of the `available` callback's raw return value, after
/// timeout-fallback flattening.
///
/// The wire callback may return a bool, an `{ ok, reason }` object, or any
/// other JSON shape. The `available` verb returns the value pretty much
/// verbatim, but the `execute` recheck path needs to branch on it — that
/// branching lives in [`interpret_available`], and this enum is the
/// branch-friendly shape it produces.
enum AvailableResult {
    /// The callback signalled "available" (returned `true`, `null`, or any
    /// shape that does not match the canonical "unavailable" patterns).
    Available,
    /// The callback signalled "unavailable" (returned `false`, an
    /// `{ ok: false, reason }` object, or was force-cancelled by the
    /// latency budget).
    Unavailable {
        /// Reason string surfaced to callers via
        /// [`CommandError::CommandUnavailable`].
        reason: String,
    },
}

/// Wrap a [`CommandContext`] in the positional-args JSON array the
/// dispatcher expects (`[ctx]`).
///
/// The callback dispatcher takes a positional arguments value; both
/// `execute` and `available` callbacks are conventionally invoked with the
/// context as their single argument. Centralizing the wrapping here keeps
/// the calling convention in one place.
fn callback_args_with_ctx(ctx: &CommandContext) -> Value {
    serde_json::json!([ctx])
}

/// Project a raw `available` callback return value onto the
/// `{ ok, reason? }` wire response.
///
/// Mirrors the rules in [`interpret_available`]:
/// - `Value::Bool(false)` and `{ ok: false, reason }` objects flatten to
///   `{ ok: false, reason: <derived> }`;
/// - everything else flattens to `{ ok: true }`.
fn available_response(raw: Value) -> Value {
    match interpret_available(raw) {
        AvailableResult::Available => serde_json::json!({ "ok": true }),
        AvailableResult::Unavailable { reason } => {
            serde_json::json!({ "ok": false, "reason": reason })
        }
    }
}

/// Map a raw `available` callback return value onto the [`AvailableResult`]
/// enum.
///
/// The rules:
/// - `Value::Bool(false)` → `Unavailable { reason: "unavailable" }` (the
///   canonical default reason when the callback gave none).
/// - `Value::Object` where `ok` is `false` → `Unavailable` with the
///   object's `reason` field (if any string), else the canonical default.
/// - Everything else (including `Value::Object` without an `ok` field) →
///   `Available`.
///
/// The `Object` arm also matches the canned timeout payload
/// `{ "ok": false, "reason": "available timeout" }`, so the timeout reason
/// propagates verbatim through both the `available` verb response and
/// `execute`'s `CommandUnavailable` reason.
///
/// # Gotcha for SDK authors
///
/// An object that omits `ok` is treated as Available, and any sibling
/// `reason` field is **silently discarded**. For example,
/// `{ "reason": "no selection" }` returns `Available`, not
/// `Unavailable { reason: "no selection" }`. SDKs that want to signal
/// "unavailable" via an object MUST set `ok: false` explicitly — `reason`
/// alone is not enough. The defensive default (treating an object missing
/// `ok` as Available rather than malformed) matches the boolean default
/// for forward compatibility: a callback that returns a bare object today
/// behaves the same as one that returns `true`, and a future schema
/// extension can add new fields without breaking older services.
fn interpret_available(raw: Value) -> AvailableResult {
    match raw {
        Value::Bool(true) => AvailableResult::Available,
        Value::Bool(false) => AvailableResult::Unavailable {
            reason: DEFAULT_UNAVAILABLE_REASON.to_string(),
        },
        Value::Object(map) => {
            let ok = map.get("ok").and_then(Value::as_bool).unwrap_or(true);
            if ok {
                AvailableResult::Available
            } else {
                let reason = map
                    .get("reason")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| DEFAULT_UNAVAILABLE_REASON.to_string());
                AvailableResult::Unavailable { reason }
            }
        }
        _ => AvailableResult::Available,
    }
}

/// Canonical reason string used when a callback signalled "unavailable" but
/// did not supply a reason of its own.
const DEFAULT_UNAVAILABLE_REASON: &str = "unavailable";

/// Recover the [`CallerId`] from the rmcp request context's extensions.
///
/// The in-process transport (`InProcessServer`) inserts the caller into
/// `RequestContext::extensions` before dispatching `call_tool`. When the
/// service is reached through an external rmcp transport that does not
/// thread a caller (a plain `stdio` server, say), the caller defaults to
/// [`CallerId::Unknown`].
fn caller_from_context(context: &RequestContext<RoleServer>) -> CallerId {
    context
        .extensions
        .get::<CallerId>()
        .cloned()
        .unwrap_or(CallerId::Unknown)
}

impl ServerHandler for CommandService {
    /// Advertise the single `command` operation tool.
    ///
    /// The tool definition is rebuilt on every call so the service has no
    /// hidden state to keep in sync; the `operation_tool!` macro expansion
    /// is cheap (it walks a fixed-size operation slice).
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: vec![Self::build_tool_definition()],
            next_cursor: None,
            meta: None,
        })
    }

    /// Route a `tools/call` for the `command` tool to the matching verb
    /// handler.
    ///
    /// Reads `arguments["op"]` to pick the verb, deserializes the rest of
    /// the arguments into the matching operation struct, then calls the
    /// stub handler. The set of verbs accepted here is exactly the verbs
    /// the `inputSchema`'s `op` enum publishes.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        if request.name.as_ref() != "command" {
            return Err(McpError::invalid_request(
                format!("unknown tool {:?}; expected \"command\"", request.name),
                None,
            ));
        }

        let arguments = Value::Object(request.arguments.unwrap_or_default());
        let op = arguments
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                McpError::invalid_params(
                    "missing required field `op` for command tool".to_string(),
                    None,
                )
            })?
            .to_string();

        let caller = caller_from_context(&context);

        let response = match op.as_str() {
            "register command" => {
                let req: RegisterCommand = deserialize_op(arguments, &op)?;
                self.handle_register(caller, req)?
            }
            "unregister command" => {
                let req: UnregisterCommand = deserialize_op(arguments, &op)?;
                self.handle_unregister(caller, req)?
            }
            "execute command" => {
                let req: ExecuteCommand = deserialize_op(arguments, &op)?;
                self.handle_execute(caller, req).await?
            }
            "available command" => {
                let req: AvailableCommand = deserialize_op(arguments, &op)?;
                self.handle_available(caller, req).await?
            }
            "list command" => {
                let req: ListCommand = deserialize_op(arguments, &op)?;
                self.handle_list(caller, req)?
            }
            "schema command" => {
                let req: SchemaCommand = deserialize_op(arguments, &op)?;
                self.handle_schema(caller, req)?
            }
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown op {other:?} for command tool"),
                    None,
                ))
            }
        };

        Ok(CallToolResult::structured(response))
    }
}
