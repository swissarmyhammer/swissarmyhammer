//! The SDK-to-host bridge seam exposed inside a plugin isolate.
//!
//! Plugin code calls back into the host platform — to register MCP servers, to
//! dispatch operations, and so on. Those calls cross the JavaScript/Rust
//! boundary through a single `deno_core` op, [`op_host_dispatch`], which this
//! module installs into every plugin isolate via the [`host_bridge`] extension.
//!
//! This module deliberately provides only the **seam**, not the dispatch
//! logic. The op forwards each call to a [`HostDispatcher`] looked up from the
//! runtime's `OpState`. Until a real dispatcher is installed — by the SDK and
//! `PluginHost` tasks — the slot holds [`UnboundHostDispatcher`], which rejects
//! every call. Wiring a working dispatcher is a later task; the contract here
//! is just "there is one clean extension point, and it is an op".

use std::sync::Arc;

use deno_core::op2;
use deno_core::OpState;
use deno_error::JsErrorBox;

/// A host-side handler for calls made by plugin code through the bridge op.
///
/// Implementors receive the raw JSON payload a plugin passed to the bridge and
/// return a JSON response (or an error message). The platform's real
/// dispatcher — registration and operation routing — is supplied by a later
/// task; this trait is the seam it plugs into.
pub trait HostDispatcher: Send + Sync {
    /// Handle one call from plugin code.
    ///
    /// # Arguments
    ///
    /// * `payload` - The JSON value the plugin passed to the bridge op.
    ///
    /// # Errors
    ///
    /// Returns an error message string when the call cannot be served. The op
    /// surfaces it to the calling plugin as a thrown JavaScript exception.
    fn dispatch(&self, payload: serde_json::Value) -> Result<serde_json::Value, String>;
}

/// The default [`HostDispatcher`] installed before a real one is wired in.
///
/// Every call is rejected. This keeps the bridge op total — a plugin that
/// calls the host before the platform has bound a dispatcher gets a clear
/// error instead of a panic or a silent no-op.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnboundHostDispatcher;

impl HostDispatcher for UnboundHostDispatcher {
    /// Reject the call: no host dispatcher has been bound to this runtime yet.
    fn dispatch(&self, _payload: serde_json::Value) -> Result<serde_json::Value, String> {
        Err("no host dispatcher is bound to this plugin runtime".to_string())
    }
}

/// The `OpState` slot holding the runtime's current [`HostDispatcher`].
///
/// It is an `Arc<dyn HostDispatcher>` so the host can hand the same dispatcher
/// to many isolates, and a newtype so it has a distinct `OpState` key that
/// will not collide with slots inserted by other extensions.
#[derive(Clone)]
pub struct HostDispatcherSlot(pub Arc<dyn HostDispatcher>);

/// The bridge op: the single seam plugin code uses to call into the host.
///
/// Plugin SDK code invokes this op with a JSON payload; the op looks up the
/// [`HostDispatcher`] currently bound in `OpState` and forwards the call. A
/// dispatcher error becomes a thrown JavaScript exception on the plugin side.
///
/// The op is intentionally generic over the payload shape: the SDK and
/// `PluginHost` tasks define the concrete request/response protocol on top of
/// this raw JSON seam.
#[op2]
#[serde]
pub fn op_host_dispatch(
    state: &mut OpState,
    #[serde] payload: serde_json::Value,
) -> Result<serde_json::Value, JsErrorBox> {
    let dispatcher = state.borrow::<HostDispatcherSlot>().0.clone();
    dispatcher.dispatch(payload).map_err(JsErrorBox::generic)
}

deno_core::extension!(
    host_bridge,
    ops = [op_host_dispatch],
    options = { dispatcher: Arc<dyn HostDispatcher> },
    state = |state, options| {
        state.put(HostDispatcherSlot(options.dispatcher));
    },
    docs = "Installs the SDK-to-host bridge op into a plugin isolate.",
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unbound_dispatcher_rejects_every_call() {
        let dispatcher = UnboundHostDispatcher;
        let result = dispatcher.dispatch(serde_json::json!({ "anything": true }));
        assert!(
            result.is_err(),
            "the unbound dispatcher must reject calls until a real one is wired in"
        );
    }
}
