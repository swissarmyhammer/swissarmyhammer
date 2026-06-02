//! Plugin-host bootstrap for the command service.
//!
//! Wires a [`CommandService`] into a live [`PluginHost`]:
//!
//! - Builds the service with a real [`CallbackDispatcher`] backed by the
//!   host's `invoke_plugin_callback`, so the `execute` and `available`
//!   verbs reach the registering plugin's isolate.
//! - Wires a real [`CallerLifecycle`] backed by the host's
//!   `record_unload_hook`, so every successful `register command` installs
//!   a purge hook on the calling plugin's per-plugin ledger entry. Plugin
//!   unload then reclaims that plugin's command-service entries without
//!   the plugin's cooperation — the headline override-stack re-emergence
//!   scenario from `ideas/plugins/command-service.md`.
//! - Wraps the service in an in-process MCP server and exposes it on the
//!   host under the module id `"commands"`. A plugin (or host caller) then
//!   activates it through the platform's usual `register` envelope —
//!   `{ rust: "commands" }` — and routes its `tools/call("command", ...)`
//!   payloads through the platform.
//!
//! Bootstrap is the production seam between the Tier-0 service core and
//! the plugin platform. The service core stays platform-agnostic; this
//! module is the only place that names both [`PluginHost`] and
//! [`CommandService`].
//!
//! See the bootstrap integration tests under
//! `tests/integration/host_bootstrap_e2e.rs` and friends for end-to-end
//! coverage of the wiring this module provides.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_plugin::{
    CallerId, InProcessServer, McpNotification, McpServer, NotificationBridge, PluginHost,
    Result as PluginResult,
};

use crate::callbacks::CallbackHandle;
use crate::invoke::{CallbackDispatcher, CallbackInvokeError};
use crate::lifecycle::{CallerLifecycle, UnloadHook};
use crate::service::CommandService;
use crate::txn::{ActionSink, SharedTransactionSeam};

/// The module id the command service is exposed under.
///
/// A plugin (or host caller) activates the service with
/// `register("<some-name>", { rust: COMMANDS_MODULE_ID })`; once activated
/// the service answers `tools/call("command", { op: "...", ... })`
/// envelopes routed through the platform.
pub const COMMANDS_MODULE_ID: &str = "commands";

/// Build a [`CommandService`] wired to `host`, wrap it in an in-process
/// MCP server, and expose it on the host under [`COMMANDS_MODULE_ID`].
///
/// Returns the shared service handle so callers can drive it directly —
/// purge a caller from the host code path, snapshot the registry for a
/// test assertion, or `flush()` the notifier at a load/unload boundary —
/// alongside the platform-exposed in-process surface.
///
/// # Wiring
///
/// - **Callback dispatcher** — [`HostCallbackDispatcher`] routes
///   `(caller, callback_id, args)` triples back into the registering
///   plugin's isolate through [`PluginHost::invoke_plugin_callback`]. A
///   non-plugin caller (host or external) has no isolate, so the
///   dispatcher fails such invocations with a descriptive error rather
///   than silently dropping them.
/// - **Caller lifecycle** — [`HostCallerLifecycle`] forwards
///   `install_unload_hook` for a [`CallerId::Plugin`] onto
///   [`PluginHost::record_unload_hook`], so the calling plugin's per-plugin
///   ledger drains the hook on unload. Non-plugin callers have no unload
///   boundary; their hooks are dropped on the floor, mirroring the noop
///   lifecycle.
///
/// # Parameters
///
/// - `host` — the live plugin host to wire the service into and expose
///   the module on.
///
/// # Errors
///
/// Returns the platform error when [`PluginHost::expose_rust_module`]
/// rejects the [`COMMANDS_MODULE_ID`] — in practice, an id already
/// exposed by a previous bootstrap call against the same host.
pub async fn install_commands_module(host: &PluginHost) -> PluginResult<Arc<CommandService>> {
    install_commands_module_with(host, None).await
}

/// Like [`install_commands_module`], but also wires a store-backed
/// transaction seam.
///
/// The Tier-0 service core cannot name `swissarmyhammer-store`, so the
/// store-backed [`TransactionSeam`](crate::TransactionSeam) is supplied by
/// the embedder that owns the board's one `StoreContext` (the kanban app).
/// Pass `Some(seam)` to bracket every `execute` in an ambient store
/// transaction (one undo group per command, `txn`-tagged data events); pass
/// `None` to fall back to the no-op seam (a `txn: null` action event, no
/// undo group).
///
/// The action sink is always wired here to the host's
/// [`NotificationBridge`](swissarmyhammer_plugin::NotificationBridge), so a
/// successful `execute` publishes `notifications/commands/executed` to every
/// subscriber regardless of whether a transaction seam was supplied.
pub async fn install_commands_module_with(
    host: &PluginHost,
    transaction: Option<SharedTransactionSeam>,
) -> PluginResult<Arc<CommandService>> {
    let dispatcher: Arc<dyn CallbackDispatcher> =
        Arc::new(HostCallbackDispatcher::new(host.clone()));
    let lifecycle: Arc<dyn CallerLifecycle> = Arc::new(HostCallerLifecycle::new(host.clone()));
    let action_sink: Arc<dyn ActionSink> =
        Arc::new(BridgeActionSink::new(host.notification_bridge()));

    let mut service = CommandService::new()
        .with_dispatcher(dispatcher)
        .with_lifecycle(lifecycle)
        .with_action_sink(action_sink);
    if let Some(transaction) = transaction {
        service = service.with_transaction(transaction);
    }
    let service = Arc::new(service);

    let server: Arc<dyn McpServer> =
        Arc::new(InProcessServer::from_arc(Arc::clone(&service)).await?);
    host.expose_rust_module(COMMANDS_MODULE_ID, server).await?;

    Ok(service)
}

/// Action sink that publishes `commands/executed` onto a [`NotificationBridge`].
///
/// The production delivery seam for the `execute` action event: the engine
/// hands a fully-formed [`McpNotification`] here and this sink fans it out to
/// every subscriber (the in-process webview and any external agent) via
/// [`NotificationBridge::publish`].
pub struct BridgeActionSink {
    /// Cheap clone of the host's one bridge; every clone shares the channel.
    bridge: NotificationBridge,
}

impl BridgeActionSink {
    /// Construct a sink that publishes onto `bridge`.
    pub fn new(bridge: NotificationBridge) -> Self {
        Self { bridge }
    }
}

impl std::fmt::Debug for BridgeActionSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeActionSink").finish()
    }
}

impl ActionSink for BridgeActionSink {
    fn commands_executed(&self, notification: McpNotification) {
        self.bridge.publish(notification);
    }
}

/// Callback dispatcher that routes invocations through a [`PluginHost`].
///
/// The dispatcher reads the [`CallerId`] off the [`CallbackHandle`] and,
/// for a [`CallerId::Plugin`], delivers the callback to that plugin's
/// isolate via [`PluginHost::invoke_plugin_callback`]. Non-plugin callers
/// have no isolate to reach back into, so the dispatcher rejects the
/// invocation with a structured error pointing at the wiring gap.
pub struct HostCallbackDispatcher {
    /// Cheap clone of the host this dispatcher routes through.
    host: PluginHost,
}

impl HostCallbackDispatcher {
    /// Construct a dispatcher rooted at `host`.
    ///
    /// Holding a [`PluginHost`] clone is cheap — the host's state lives
    /// behind an `Arc` — so a dispatcher per service is the natural seam.
    pub fn new(host: PluginHost) -> Self {
        Self { host }
    }
}

impl std::fmt::Debug for HostCallbackDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostCallbackDispatcher").finish()
    }
}

#[async_trait]
impl CallbackDispatcher for HostCallbackDispatcher {
    async fn invoke(
        &self,
        handle: &CallbackHandle,
        args: Value,
    ) -> Result<Value, CallbackInvokeError> {
        let plugin_id = match &handle.caller {
            CallerId::Plugin(plugin_id) => plugin_id.clone(),
            other => {
                return Err(CallbackInvokeError::new(format!(
                    "callback dispatcher cannot route to caller {other:?}; only \
                     plugin-scoped callbacks have an isolate to invoke",
                )));
            }
        };

        self.host
            .invoke_plugin_callback(&plugin_id, handle.callback_id.clone(), args)
            .await
            .map_err(|error| CallbackInvokeError::new(error.to_string()))
    }
}

/// Caller-lifecycle implementation that installs unload hooks on a
/// [`PluginHost`]'s per-plugin ledger.
///
/// For a [`CallerId::Plugin`] the hook is appended to the plugin's
/// ledger; the host's unload drain runs it as part of normal cleanup. A
/// non-plugin caller has no per-host unload boundary, so its hook is
/// dropped on the floor, exactly as [`crate::NoopCallerLifecycle`] would.
pub struct HostCallerLifecycle {
    /// Cheap clone of the host this lifecycle targets.
    host: PluginHost,
}

impl HostCallerLifecycle {
    /// Construct a lifecycle rooted at `host`.
    pub fn new(host: PluginHost) -> Self {
        Self { host }
    }
}

impl std::fmt::Debug for HostCallerLifecycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostCallerLifecycle").finish()
    }
}

impl CallerLifecycle for HostCallerLifecycle {
    fn install_unload_hook(&self, caller: &CallerId, hook: UnloadHook) {
        // Only plugin callers have a per-host unload boundary. A host or
        // external caller's hook is dropped here, mirroring the noop
        // lifecycle — the only behavioral difference is that production
        // wiring still installs the host's plugin-scoped hooks.
        let CallerId::Plugin(plugin_id) = caller else {
            return;
        };
        // `record_unload_hook` is synchronous: it only takes the host's
        // state mutex briefly to append a ledger entry, so the install is
        // immediate. There is still a narrow race window where a register
        // can complete and mutate the service's registry, then the host
        // unloads the plugin (draining its ledger) before this install
        // runs. In that case the host returns `false` and the hook never
        // attaches — the entries the register just pushed would leak
        // until another caller's purge or the next bootstrap. We log
        // `warn!` (not `debug!`) so the leak is observable in production
        // even though the race itself is atypical (register-during-unload
        // of the calling plugin should not happen in normal use).
        if !self.host.record_unload_hook(plugin_id, hook) {
            tracing::warn!(
                plugin = %plugin_id.as_str(),
                "command-service unload hook dropped: plugin not tracked; \
                 any registrations made between the failed install and \
                 the unload may leak until another caller purges"
            );
        }
    }
}
