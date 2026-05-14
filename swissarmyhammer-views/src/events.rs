//! Events emitted by `ViewsContext` when state changes.
//!
//! These events are broadcast via a `tokio::sync::broadcast` channel so that
//! consumers (e.g. a Tauri frontend bridge) can subscribe without coupling
//! the views crate to any specific UI framework.
//!
//! Follows the same pattern as `swissarmyhammer_perspectives::events::PerspectiveEvent`.

use serde::{Deserialize, Serialize};

/// Events emitted by the views context when state changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ViewEvent {
    /// A view was created or modified.
    ///
    /// `changed_fields` lists the field names that changed (e.g. `["name"]`,
    /// `["kind", "card_fields"]`). For brand-new views, all fields are listed
    /// and `is_create` is true.
    ViewChanged {
        /// The view ULID.
        id: String,
        /// Names of the fields that changed relative to the prior state.
        changed_fields: Vec<String>,
        /// True when this is a brand-new view (first write), false for
        /// updates. Consumers use this to emit the correct event type
        /// (e.g. `entity-created` vs `entity-field-changed`).
        is_create: bool,
    },
    /// A view was deleted.
    ViewDeleted {
        /// The view ULID.
        id: String,
    },
}
