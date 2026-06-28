//! Callback dispatcher abstraction for the `execute` and `available` verbs.
//!
//! The `execute` and `available` verb handlers in
//! [`crate::service::CommandService`] need to call back into the registering
//! caller's isolate — the plugin SDK stored the original function under a
//! `cb_<n>` id, and the host has only the [`crate::CallbackHandle`] paired
//! with that id and the registering [`swissarmyhammer_plugin::CallerId`].
//!
//! [`CallbackDispatcher`] is the seam between this crate and the platform's
//! routing layer. The service receives an `Arc<dyn CallbackDispatcher>` and
//! calls [`CallbackDispatcher::invoke`] to deliver the callback request back
//! to the originating isolate; tests substitute an in-memory fake.
//!
//! This keeps the command-service crate Tier-0 — it does not depend on the
//! plugin runtime, transport, or any concrete dispatcher implementation. The
//! platform-integration layer wires a real implementation (built on top of
//! `PluginRuntime::invoke_callback`) in a higher-tier crate.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::callbacks::CallbackHandle;

/// Error returned by [`CallbackDispatcher::invoke`].
///
/// The dispatcher abstraction is intentionally narrow: every concrete
/// failure mode (transport error, runtime panic, missing callback id) is
/// flattened into a single `message` string. The service layer wraps the
/// failure in [`crate::CommandError::CallbackFailed`] for the wire response;
/// callers that care about the underlying cause read the message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackInvokeError {
    /// Human-readable description of why the callback could not be invoked
    /// or did not return cleanly.
    pub message: String,
}

impl CallbackInvokeError {
    /// Construct a new error from any displayable cause.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CallbackInvokeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CallbackInvokeError {}

/// Routes a callback invocation back to the registering caller's isolate.
///
/// The dispatcher receives a [`CallbackHandle`] (caller + callback id) and a
/// positional arguments value, delivers the invocation to the originating
/// isolate's callback table, and returns the function's settled result.
///
/// The trait is intentionally narrow — one method, no lifecycle — because
/// the service only uses it on the `execute` and `available` verb paths.
/// Plugin-load / unload / purge concerns belong to the layer that owns the
/// dispatcher implementation, not to this seam.
#[async_trait]
pub trait CallbackDispatcher: Send + Sync + std::fmt::Debug {
    /// Invoke `handle.callback_id` in `handle.caller`'s isolate with `args`.
    ///
    /// # Parameters
    ///
    /// - `handle` — the `(caller, callback_id)` pair stored on the active
    ///   registry entry.
    /// - `args` — the positional arguments payload as a single JSON value.
    ///   Conventionally a single-element array carrying the context, but
    ///   the dispatcher does not interpret it.
    ///
    /// # Returns
    ///
    /// The callback's settled return value as a JSON value.
    ///
    /// # Errors
    ///
    /// Returns [`CallbackInvokeError`] when the callback id does not
    /// resolve, the registering isolate is gone, the function throws, or
    /// the transport otherwise fails to deliver the invocation.
    async fn invoke(
        &self,
        handle: &CallbackHandle,
        args: Value,
    ) -> Result<Value, CallbackInvokeError>;
}

/// A dispatcher that refuses every invocation.
///
/// Used as the default sink so [`crate::CommandService::new`] keeps working
/// for callers that never exercise `execute` / `available`. Any attempt to
/// dispatch through this sink returns [`CallbackInvokeError`] with a
/// pointer at the wiring gap.
#[derive(Debug, Default)]
pub struct NoopCallbackDispatcher;

#[async_trait]
impl CallbackDispatcher for NoopCallbackDispatcher {
    async fn invoke(
        &self,
        handle: &CallbackHandle,
        _args: Value,
    ) -> Result<Value, CallbackInvokeError> {
        Err(CallbackInvokeError::new(format!(
            "no callback dispatcher wired; cannot invoke {:?}",
            handle.callback_id
        )))
    }
}

/// Type alias for the shared dispatcher handle stored on the service.
pub type SharedCallbackDispatcher = Arc<dyn CallbackDispatcher>;
