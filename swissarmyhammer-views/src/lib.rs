//! View registry and changelog system
//!
//! `swissarmyhammer-views` is a standalone crate that manages view definitions.
//! Views are simple metadata records describing how to render entities. The `kind`
//! field is a renderer hint -- the actual rendering logic lives in the frontend.
//!
//! # Architecture
//!
//! - **Metadata-only**: Owns view definitions, not rendering logic
//! - **YAML on disk**: One `.yaml` file per view definition
//! - **Consumer-agnostic**: Takes a `Path`, consumers decide where it lives
//! - **VFS loading**: `from_yaml_sources()` loads from pre-resolved YAML entries (builtin + local)
//! - **Changelog**: Whole-view snapshot JSONL log with undo/redo support

pub mod changelog;
pub mod context;
pub mod error;
pub mod types;

pub use changelog::{ViewChangeEntry, ViewChangeOp, ViewsChangelog};
pub use context::{load_yaml_dir, ViewsContext};
pub use error::{Result, ViewsError};
pub use types::{ViewCommand, ViewCommandKeys, ViewDef, ViewId, ViewKind};
