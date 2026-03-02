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

pub mod entity;
pub mod error;
pub mod io;

pub use entity::Entity;
pub use error::{EntityError, Result};
pub use io::{
    delete_entity_files, entity_extension, entity_file_path, read_entity, read_entity_dir,
    write_entity,
};
