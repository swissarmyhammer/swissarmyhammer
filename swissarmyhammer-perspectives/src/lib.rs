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
//! - **TrackedStore**: Implements `TrackedStore` for undo/redo via `swissarmyhammer-store`

pub mod context;
pub mod error;
pub mod store;
pub mod types;

use serde::{Deserialize, Serialize};
use std::fmt;
use swissarmyhammer_common::define_id;

define_id!(PerspectiveId, "ULID-based identifier for perspectives");

pub use context::PerspectiveContext;
pub use error::{PerspectiveError, Result};
pub use store::PerspectiveStore;
pub use types::{Perspective, PerspectiveFieldEntry, SortDirection, SortEntry};
