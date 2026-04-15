//! Events emitted by the entity cache when state changes.
//!
//! These events are broadcast via a `tokio::sync::broadcast` channel so that
//! consumers (e.g. a Tauri frontend bridge) can subscribe without coupling
//! the entity crate to any specific UI framework.

use serde::{Deserialize, Serialize};

/// A single field-level change within an `EntityChanged` event.
///
/// Carries the name of the field that changed and its new value. A removal
/// is encoded as `value: serde_json::Value::Null` — this matches the
/// frontend's existing patch semantics where a `null` value at a field
/// position means the field was deleted.
///
/// This is a distinct shape from `changelog::FieldChange`, which is a rich
/// enum used for undo/redo. The event-layer struct is deliberately simple:
/// the frontend's `entity-field-changed` Tauri event contract only needs
/// `{field, value}` pairs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldChange {
    /// The name of the field that was added, modified, or removed.
    pub field: String,
    /// The new value of the field, or `Value::Null` when the field was removed.
    pub value: serde_json::Value,
}

/// Events emitted by the entity cache when state changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum EntityEvent {
    /// An entity was created or modified.
    ///
    /// `changes` describes which fields were added, modified, or removed as
    /// part of this write. Removals are encoded as `FieldChange { value: Null }`.
    /// Brand-new entities (no prior cache entry) include every field of the
    /// new entity in `changes`.
    EntityChanged {
        /// The entity type name (e.g. "tag", "task").
        entity_type: String,
        /// The entity id.
        id: String,
        /// Monotonically increasing version stamp.
        version: u64,
        /// Field-level diff between the previous cached state and the new
        /// on-disk state. Empty only when `EntityCache` detects a no-op write,
        /// in which case no event is emitted at all.
        changes: Vec<FieldChange>,
    },
    /// An entity was deleted.
    EntityDeleted {
        /// The entity type name (e.g. "tag", "task").
        entity_type: String,
        /// The entity id.
        id: String,
    },
    /// A file under an entity's `.attachments/` directory was created,
    /// modified, or removed.
    ///
    /// Attachments are not entities — they do not populate the `EntityCache`
    /// map. This event is purely a notification so consumers (e.g. a frontend
    /// bridge) can refresh thumbnails or badge counts.
    ///
    /// Field names mirror the kanban-app's historical `attachment-changed`
    /// Tauri payload exactly so a downstream bridge can forward without shape
    /// translation.
    AttachmentChanged {
        /// The entity type that owns the attachment (e.g. `"task"`). Derived
        /// from the parent directory name by stripping the trailing `s`.
        entity_type: String,
        /// The stored filename including extension (e.g. `"01ABC-photo.png"`).
        filename: String,
        /// `true` if the file no longer exists after this event, `false` for
        /// create/modify.
        removed: bool,
    },
}
