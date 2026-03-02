//! Field registry and entity template system
//!
//! `swissarmyhammer-fields` is a standalone, schema-only crate that manages field
//! definitions and entity templates. It knows nothing about kanban, tasks, or tags —
//! consumers provide their own built-in definitions via `with_defaults()`.
//!
//! # Architecture
//!
//! - **Schema-only**: Owns field definitions and entity templates, not field values
//! - **YAML on disk**: One `.yaml` file per field definition, one per entity template
//! - **Consumer-agnostic**: Takes a `Path`, consumers decide where it lives
//! - **Default seeding**: `with_defaults()` writes defaults that don't exist, preserves customizations

pub mod context;
pub mod error;
pub mod types;
pub mod validation;

pub use context::{FieldDefaults, FieldsContext, FieldsContextBuilder};
pub use error::{FieldsError, Result};
pub use types::{Display, Editor, EntityDef, FieldDef, FieldType, SelectOption, SortKind};
pub use validation::{EntityLookup, ValidationEngine};
