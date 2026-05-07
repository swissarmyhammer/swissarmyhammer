//! Spatial focus and keyboard navigation engine.
//!
//! This crate provides the headless spatial-navigation kernel used by GUI
//! consumers (Tauri adapters, CLI front-ends, tests) to track keyboard
//! focus across a 2-D layout and move it in cardinal directions.
//!
//! The crate is **generic and domain-free**: nothing in here knows about
//! kanban tasks, columns, projects, or any other application concept.
//! Identities are [`FullyQualifiedMoniker`] paths produced by the
//! consumer; the kernel only sees rectangles, layers, and per-decision
//! scope snapshots.
//!
//! # Stateless with respect to scope geometry
//!
//! Scope geometry rides on every focus-mutating IPC as a
//! [`NavSnapshot`]; the kernel reads scope state out of the snapshot at
//! the moment of a decision and does not maintain a between-decision
//! replica. The registry holds layers and the cross-snapshot
//! `last_focused_by_fq` memory only.
//!
//! # Navigation rules
//!
//! Cardinal navigation obeys one load-bearing contract: **within a
//! parent scope, child scopes — leaves and containers alike — are
//! siblings.** The Android beam score picks the geometrically best
//! candidate.
//!
//! [`Direction::First`] / [`Direction::Last`] focus the focused
//! scope's children — first by topmost-then-leftmost, last by
//! bottommost-then-rightmost. On a focused leaf they return the
//! focused FQM (no children → no-op). The deprecated
//! `Direction::RowStart` / `Direction::RowEnd` aliases route through
//! the same path during the one-release deprecation window.
//!
//! See the [`navigate`] module docs for the full algorithm.
//!
//! # No-silent-dropout contract
//!
//! Nav APIs always return a [`FullyQualifiedMoniker`]. "No motion
//! possible" is communicated by returning the focused entry's own FQM.
//! Torn state (unknown FQM) emits `tracing::error!` and echoes the
//! input FQM. There is no [`Option`] or [`Result`] on these APIs.
//!
//! # Modules
//!
//! - [`types`] — newtype wrappers, the [`Rect`] value type, and the
//!   [`Direction`] enum used by the spatial-nav surface.
//! - [`layer`] — [`FocusLayer`], the modal-boundary primitive.
//! - [`snapshot`] — per-decision [`NavSnapshot`] / [`SnapshotScope`]
//!   wire types and the [`IndexedSnapshot`] read helper.
//! - [`registry`] — [`SpatialRegistry`] (layer store + cross-snapshot
//!   focus memory).
//! - [`state`] — [`SpatialState`] per-window focus tracker plus the
//!   [`FocusChangedEvent`] adapters emit on every focus mutation.
//! - [`navigate`] — [`pick_target`], the snapshot-driven
//!   Android-beam-search pathfinder.
//! - [`observer`] — the [`FocusEventSink`] trait plus [`NoopSink`] and
//!   [`RecordingSink`] for adapters that prefer push-based event
//!   delivery.
//!
//! # Wire format
//!
//! All public types use `serde` with stable JSON shapes — string newtypes
//! serialize transparently as bare strings, [`Pixels`] serializes as a
//! bare number. The frontend mirrors these as branded TypeScript types.

pub mod layer;
pub mod navigate;
pub mod observer;
pub mod registry;
pub mod snapshot;
pub mod state;
pub mod types;

pub use layer::FocusLayer;
pub use navigate::{drill_in, drill_out, pick_target};
pub use observer::{FocusEventSink, NoopSink, RecordingSink};
pub use registry::SpatialRegistry;
pub use snapshot::{FocusOverrides, IndexedSnapshot, NavSnapshot, SnapshotScope};
pub use state::{FallbackResolution, FocusChangedEvent, LostFocusContext, SpatialState};
pub use types::{
    Direction, FullyQualifiedMoniker, LayerName, Pixels, Rect, SegmentMoniker, WindowLabel,
};
