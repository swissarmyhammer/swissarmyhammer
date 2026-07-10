//! Callback handles stored alongside registrations.
//!
//! When a plugin registers a command, the plugin SDK strips out the
//! `available` and `execute` function values before sending the
//! registration payload across the host/plugin boundary, replacing each
//! function with an opaque [`CallbackMarker`] of shape
//! `{ "$callback": "cb_..." }`. The service stores those markers in the
//! registry so that, later, the `execute` / `available` verb handlers can
//! pair a marker with the registering caller and send a
//! `notifications/callbacks/invoke` back to the originating isolate.
//!
//! A [`CallbackHandle`] is exactly that pairing — a `(caller, callback_id)`
//! tuple — and is the dispatch-time form of a callback. It is constructed
//! when the platform-integration layer (in a subsequent task) wires the
//! callback dispatcher; this layer just defines the type and the helper
//! used by [`crate::service::CommandService`] to recover callback markers
//! from incoming registration payloads.
//!
//! See `ideas/plugins/command-service.md` §"Callback markers" for the
//! end-to-end design.

use swissarmyhammer_plugin::CallerId;

use crate::types::CallbackMarker;

/// A callback marker paired with the caller that registered it.
///
/// The registry stores each registration's full payload (including its
/// [`CallbackMarker`] fields) and the registering [`CallerId`]. At dispatch
/// time the service zips these together into a [`CallbackHandle`], which
/// the platform-integration layer's callback dispatcher uses to route the
/// invocation back to the originating isolate.
///
/// This task only defines the shape; the dispose-fn ledger entry and the
/// `notifications/callbacks/invoke` wire path land in a follow-up task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackHandle {
    /// The caller that registered the command this callback belongs to.
    pub caller: CallerId,
    /// The SDK-assigned callback id (e.g. `"cb_42"`). Opaque to the host —
    /// the platform's callback dispatcher resolves it back into the
    /// originating isolate's function table.
    pub callback_id: String,
}

impl CallbackHandle {
    /// Construct a [`CallbackHandle`] from a caller and a borrowed marker.
    ///
    /// Convenience for the dispatch path, where the service already holds
    /// the caller (from `RequestContext::extensions`) and a reference to
    /// the marker pulled out of the active [`crate::registry::StackEntry`].
    pub fn from_marker(caller: CallerId, marker: &CallbackMarker) -> Self {
        Self {
            caller,
            callback_id: marker.callback_id.clone(),
        }
    }
}

/// Predicate: is this marker present (non-empty `callback_id`)?
///
/// Returns `true` when the marker carries a non-empty id. The only way
/// to receive an empty id is an SDK serializer bug — the SDK always
/// mints a fresh `cb_<n>` id before stripping the function — so callers
/// use this predicate as the structured signal to emit a `MissingCallback`
/// error rather than silently storing an unroutable marker.
pub fn is_callback_present(marker: &CallbackMarker) -> bool {
    !marker.callback_id.is_empty()
}
