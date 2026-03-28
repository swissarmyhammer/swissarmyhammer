//! Dynamic field-driven entity type with generic I/O.
//!
//! This crate provides a generic `Entity` type backed by a `HashMap<String, Value>`
//! and I/O functions that read/write entities as YAML or YAML-frontmatter + markdown
//! body, depending on the `EntityDef` schema.
//!
//! It is consumer-agnostic — it knows nothing about kanban, tasks, or tags.
//! Consumers provide entity type names, directory paths, and `EntityDef` schemas.
//!
//! ## Storage Formats
//!
//! - **With body_field**: `.md` file — YAML frontmatter + markdown body
//! - **Without body_field**: `.yaml` file — plain YAML
//!
//! Entity IDs come from filenames, not file contents.
//! Writes are atomic (temp file + rename).

pub mod cache;
pub mod changelog;
pub mod context;
pub mod entity;
pub mod error;
pub mod events;
pub mod id_types;
pub mod io;
pub mod undo_stack;
pub mod watcher;

pub use cache::{CachedEntity, EntityCache};
pub use context::EntityContext;
pub use entity::Entity;
pub use error::{EntityError, Result};
pub use events::EntityEvent;
pub use id_types::{ChangeEntryId, EntityId, TransactionId};
pub use io::{
    entity_extension, entity_file_path, read_entity, read_entity_dir, restore_entity_files,
    trash_entity_files, write_entity,
};
pub use undo_stack::UndoStack;
pub use watcher::EntityWatcher;

/// Test utilities shared between unit tests and integration tests.
///
/// Available when running tests (`#[cfg(test)]`) or when the `test-support`
/// feature is enabled. Integration tests enable `test-support` via
/// dev-dependency features.
#[cfg(any(test, feature = "test-support"))]
pub mod test_utils;
