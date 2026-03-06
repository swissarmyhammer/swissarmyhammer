//! Events emitted by the entity cache when state changes.
//!
//! These events are broadcast via a `tokio::sync::broadcast` channel so that
//! consumers (e.g. a Tauri frontend bridge) can subscribe without coupling
//! the entity crate to any specific UI framework.

use serde::{Deserialize, Serialize};

/// Events emitted by the entity cache when state changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum EntityEvent {
    /// An entity was created or modified.
    EntityChanged {
        /// The entity type name (e.g. "tag", "task").
        entity_type: String,
        /// The entity id.
        id: String,
        /// Monotonically increasing version stamp.
        version: u64,
    },
    /// An entity was deleted.
    EntityDeleted {
        /// The entity type name (e.g. "tag", "task").
        entity_type: String,
        /// The entity id.
        id: String,
    },
}
