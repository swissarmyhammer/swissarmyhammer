//! Generic file-backed store with changelog, undo/redo, and change detection.
//!
//! # Architecture
//!
//! The crate is layered as follows:
//!
//! 1. **[`TrackedStore`]** -- a trait you implement to describe one directory of
//!    files: how to serialize/deserialize items, extract IDs, and what file
//!    extension to use.
//!
//! 2. **[`StoreHandle`]** -- wraps any `TrackedStore` and adds write, delete,
//!    undo, redo, and change-detection. It maintains an in-memory cache, an
//!    append-only JSONL changelog, and a `.trash/` soft-delete directory.
//!
//! 3. **[`StoreContext`]** -- coordinates multiple `StoreHandle`s behind the
//!    type-erased [`ErasedStore`](erased::ErasedStore) trait, sharing a single
//!    [`UndoStack`] so that undo/redo works across heterogeneous stores.
//!
//! # Substrate invariant
//!
//! One `Arc<StoreContext>` per app; share it via `Arc::clone`; never construct
//! a second one for the same board. A second context would fork the undo
//! stack and produce a second `undo_stack.yaml`, so `undo` would only reverse
//! the half of the writes that happened to land in the context the caller
//! reached. See [`StoreContext`] for the full rationale and the location of
//! the regression guard.
//!
//! This crate provides:
//! - [`TrackedStore`] trait for defining file-backed stores
//! - [`StoreHandle`] for write, delete, undo, redo, and change detection
//! - [`StoreContext`] for coordinating multiple stores with shared undo/redo
//! - [`UndoStack`] for persistent undo/redo pointer management
//! - [`Changelog`](changelog::Changelog) for append-only JSONL change logging

pub mod changelog;
pub mod context;
pub mod diff;
pub mod erased;
pub mod error;
pub mod event;
pub mod handle;
pub mod id;
pub mod operations;
pub mod server;
pub mod stack;
pub mod store;
pub mod trash;

pub use changelog::{ChangeOp, ChangelogEntry};
pub use context::{StoreContext, UndoOutcome};
pub use error::StoreError;
pub use event::ChangeEvent;
pub use handle::StoreHandle;
pub use id::{StoredItemId, UndoEntryId};
pub use server::StoreServer;
pub use stack::{UndoEntry, UndoStack};
pub use store::TrackedStore;
