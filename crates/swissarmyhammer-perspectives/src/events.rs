//! Events emitted by `PerspectiveContext` when state changes.
//!
//! These events are broadcast via a `tokio::sync::broadcast` channel so that
//! consumers (e.g. a Tauri frontend bridge) can subscribe without coupling
//! the perspectives crate to any specific UI framework.
//!
//! Follows the same pattern as `swissarmyhammer_entity::events::EntityEvent`.

use serde::{Deserialize, Serialize};

/// Events emitted by the perspective context when state changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum PerspectiveEvent {
    /// A perspective was created or modified.
    ///
    /// `changed_fields` lists the field names that changed (e.g. `["filter"]`,
    /// `["name", "sort"]`). For brand-new perspectives, all fields are listed
    /// and `is_create` is true.
    PerspectiveChanged {
        /// The perspective ULID.
        id: String,
        /// Names of the fields that changed relative to the prior state.
        changed_fields: Vec<String>,
        /// True when this is a brand-new perspective (first write), false for
        /// updates. Consumers use this to emit the correct event type
        /// (e.g. `entity-created` vs `entity-field-changed`).
        is_create: bool,
    },
    /// A perspective was deleted.
    PerspectiveDeleted {
        /// The perspective ULID.
        id: String,
    },
}
