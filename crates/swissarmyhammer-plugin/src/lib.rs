//! The SwissArmyHammer plugin platform.
//!
//! This crate hosts plugins as MCP (Model Context Protocol) servers and
//! routes work to them. Its two responsibilities are:
//!
//! - **Registration** — plugins register and unregister MCP servers with the
//!   platform under unique names, making their tools and operations available.
//! - **Dispatch** — callers issue generic operation requests against a named
//!   server/tool/operation triple, and the platform dispatches them to the
//!   appropriate registered server.
//!
//! The modules below carry the pieces of that platform:
//!
//! - [`registry`] — tracks the set of registered MCP servers by name.
//! - [`dispatcher`] — routes generic operation requests to a registered server.
//! - [`server`] — the MCP server abstraction and its transports.
//! - [`notify`] — the MCP notification surface: the bridge, the per-client
//!   subscription registry, and the normalized four-plane notification model
//!   (`store/changed`, `commands/executed`, `commands/changed` /
//!   `tools/list_changed`, `ui_state/changed` / `store/undo_changed`), each
//!   carrying `txn` (correlation) and `origin` (provenance).
//! - [`runtime`] — the JavaScript runtime that hosts plugin code.
//! - [`sdk`] — the `@swissarmyhammer/plugin` TypeScript SDK, embedded.
//! - [`host`] — host-side bindings exposed to plugins.
//! - [`ledger`] — records of registration and dispatch activity.
//! - [`discovery`] — stacked, point-in-time discovery of plugins on disk.
//! - [`reload`] — hot reload seam: the per-plugin reload status the host
//!   surfaces.
//! - [`codegen`] — code generation for plugin scaffolding and bindings.
//! - [`error`] — the platform [`Error`] type and [`Result`] alias.
//!
//! This is the scaffold crate; the module bodies are filled in by later work.

pub mod codegen;
pub mod discovery;
pub mod dispatcher;
pub mod error;
/// Per-host registry of plugin event subscriptions (notification method →
/// interested plugin callbacks). Internal to the host's event-delivery path.
mod events;
pub mod host;
pub mod ledger;
pub mod notify;
pub mod registry;
pub mod reload;
pub mod runtime;
pub mod sdk;
pub mod server;

pub use discovery::{discover_plugins, DiscoveredPlugin, LayerRoot, PLUGINS_SUBDIR};
pub use dispatcher::Dispatcher;
pub use error::{Error, Result};
pub use host::{BridgeCallFuture, BridgeCallScope, BridgeCallScopeGuard, PluginHost};
pub use ledger::{CallbackId, PluginLedger, RegistrationHandle};
pub use notify::{
    ChangeOp, FieldChange, McpNotification, NotificationBridge, NotificationSubscription,
    Provenance, SubscriberId, SubscriberKind,
};
pub use registry::{
    RegisterOutcome, ServerName, ServerRegistry, ServerSource, ServerStatus, UnregisterOutcome,
};
pub use reload::ReloadStatus;
pub use runtime::{
    transpile_typescript, CallbackInvoker, HostDispatcher, PluginLifecycle, PluginModuleLoader,
    PluginRuntime, RuntimeConfig, TranspiledModule, UnboundHostDispatcher,
};
pub use server::{
    CallerId, CliServer, InProcessServer, McpServer, PluginId, ToolMetadata, UrlServer,
};
