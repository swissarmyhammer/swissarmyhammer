//! Field registry and entity template system
//!
//! `swissarmyhammer-fields` is a standalone, schema-only crate that manages field
//! definitions and entity templates. It knows nothing about kanban, tasks, or tags --
//! consumers provide their own built-in definitions via YAML files.
//!
//! # Architecture
//!
//! - **Schema-only**: Owns field definitions and entity templates, not field values
//! - **YAML on disk**: One `.yaml` file per field definition, one per entity template
//! - **Consumer-agnostic**: Takes a `Path`, consumers decide where it lives
//! - **VFS loading**: `from_yaml_sources()` loads from pre-resolved YAML entries (builtin + local)

pub mod compute;
pub mod context;
pub mod error;
pub mod id_types;
pub mod types;
pub mod validation;

pub use compute::{ComputeEngine, DeriveFn};
pub use context::{load_yaml_dir, FieldsContext, FieldsContextBuilder};
pub use error::{FieldsError, Result};
pub use id_types::{EntityTypeName, FieldDefId, FieldName};
pub use types::{Display, Editor, EntityDef, FieldDef, FieldType, SelectOption, SortKind};
pub use validation::{EntityLookup, ValidationEngine};
