//! Perspective registry and changelog system
//!
//! `swissarmyhammer-perspectives` is a standalone crate that manages perspective definitions.
//! Perspectives describe how to render and filter kanban board views. The actual rendering
//! logic lives in the frontend.
//!
//! # Architecture
//!
//! - **Metadata-only**: Owns perspective definitions, not rendering logic
//! - **YAML on disk**: One `.yaml` file per perspective definition
//! - **Consumer-agnostic**: Takes a `Path`, consumers decide where it lives
//! - **Changelog**: Whole-perspective snapshot JSONL log with undo/redo support

pub mod changelog;
pub mod context;
pub mod error;
pub mod types;

pub use changelog::{PerspectiveChangelog, PerspectiveChangeEntry, PerspectiveChangeOp};
pub use context::PerspectiveContext;
pub use error::{PerspectiveError, Result};
pub use types::{Perspective, PerspectiveFieldEntry, SortDirection, SortEntry};
