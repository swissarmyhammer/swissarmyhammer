//! # SwissArmyHammer Command Service
//!
//! Operation structs, registration payload types, and the in-process
//! `rmcp::ServerHandler` for the `command` operation tool. This crate is
//! the source of truth for the tool's verb / noun / parameter surface —
//! the wire-level `inputSchema` and the discovery `_meta` tree are both
//! derived from the operation structs defined here.
//!
//! The service's override-stack registry and debounced change notifier
//! also live here; verb handler bodies (callback dispatch, transaction
//! bracketing) are stubs in this layer and get filled in by follow-up
//! tasks.

pub mod bootstrap;
mod callbacks;
mod invoke;
mod latency;
mod lifecycle;
mod notifications;
mod operations;
mod registry;
mod service;
mod txn;
mod types;

pub use callbacks::CallbackHandle;
pub use invoke::{
    CallbackDispatcher, CallbackInvokeError, NoopCallbackDispatcher, SharedCallbackDispatcher,
};
pub use latency::{
    AvailableLatencyOutcome, AVAILABLE_HARD_DEADLINE, AVAILABLE_TIMEOUT_REASON,
    AVAILABLE_WARN_THRESHOLD,
};
pub use lifecycle::{CallerLifecycle, NoopCallerLifecycle, SharedCallerLifecycle, UnloadHook};
pub use notifications::ChangeNotifier;
pub use operations::{
    command_notifications, operations, AvailableCommand, ExecuteCommand, ListCommand,
    RegisterCommand, SchemaCommand, UnregisterCommand,
};
pub use registry::{CommandRegistry, StackEntry};
pub use service::{CommandService, DEFAULT_CHANGE_NOTIFICATION_DEBOUNCE};
pub use txn::{
    ActionSink, NoopActionSink, NoopTransactionSeam, SharedActionSink, SharedTransactionSeam,
    TransactionSeam,
};
pub use types::{
    CallbackMarker, CommandContext, CommandError, CommandMetadata, CommandSchema, ParamDef,
    ParamOption, ParamShape, ParamSource,
};

/// Alias for [`RegisterCommand`] — the full registration payload.
///
/// `RegisterCommand` IS the registration data; this alias exists so
/// callers who refer to "the registration payload" outside the operation-
/// tool dispatch path can use a noun-shaped name.
pub type CommandRegistration = RegisterCommand;
