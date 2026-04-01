//! Change events produced by store flush operations.
//!
//! These events are consumed by a dispatch layer (e.g., Tauri event emitter)
//! to notify the frontend of changes detected on disk.

/// A change event produced by a store's `flush_changes()`.
///
/// The caller (dispatch layer) emits these to the frontend.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangeEvent {
    /// Event name for the frontend (e.g. "entity-field-changed", "perspective-changed").
    pub event_name: String,
    /// JSON payload with change details.
    pub payload: serde_json::Value,
}
