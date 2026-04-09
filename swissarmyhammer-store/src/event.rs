//! Change events produced by store flush operations.
//!
//! These events are consumed by a dispatch layer (e.g., Tauri event emitter)
//! to notify the frontend of changes detected on disk.

/// A change event produced by a store's `flush_changes()`.
///
/// The caller (dispatch layer) emits these to the frontend.
/// Fields are private to preserve semver freedom to add new fields.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangeEvent {
    /// Event name for the frontend (e.g. "entity-field-changed", "perspective-changed").
    event_name: String,
    /// JSON payload with change details.
    payload: serde_json::Value,
}

impl ChangeEvent {
    /// Creates a new change event.
    ///
    /// # Parameters
    ///
    /// - `event_name` -- the event name for the frontend (e.g. "item-created").
    /// - `payload` -- JSON payload with change details.
    pub fn new(event_name: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            event_name: event_name.into(),
            payload,
        }
    }

    /// Returns the event name (e.g. "item-created", "item-changed", "item-removed").
    pub fn event_name(&self) -> &str {
        &self.event_name
    }

    /// Returns the JSON payload with change details.
    pub fn payload(&self) -> &serde_json::Value {
        &self.payload
    }
}
