//! Spatial focus and keyboard navigation engine.
//!
//! This crate provides the headless spatial-navigation kernel used by GUI
//! consumers (Tauri adapters, CLI front-ends, tests) to track keyboard
//! focus across a 2-D layout and move it in cardinal directions.
//!
//! The crate is **generic and domain-free**: nothing in here knows about
//! kanban tasks, columns, projects, or any other application concept.
//! Identities are [`FullyQualifiedMoniker`] paths produced by the
//! consumer (the path through the focus hierarchy); the kernel only
//! sees rectangles, layers, and scopes.
//!
//! # Two peers, not three
//!
//! The kernel exposes two peer types: [`FocusLayer`] (modal boundary)
//! and [`FocusScope`]. There is no separate "zone" type — whether a
//! scope is a leaf or a navigable container is determined at runtime
//! by whether anything else is registered under it
//! ([`SpatialRegistry::children_of`] / [`SpatialRegistry::has_children`]).
//! UI authoring stays simple: the consumer mounts a `<FocusScope>`
//! and the kernel decides what role it plays.
//!
//! # Navigation rules
//!
//! Cardinal navigation obeys one load-bearing contract: **within a
//! parent [`FocusScope`], child scopes — leaves and containers alike —
//! are siblings.** The Android beam score picks the geometrically best
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
//! Nav and drill APIs always return a [`FullyQualifiedMoniker`]. "No
//! motion possible" is communicated by returning the focused entry's
//! own FQM — the React side detects "stay put" by comparing the
//! returned FQM to the previous focused FQM. Torn state (unknown FQM,
//! orphan parent reference) emits `tracing::error!` and echoes the
//! input FQM so the call site has a valid result. There is no
//! [`Option`] or [`Result`] on these APIs; silence is impossible. See
//! the [`navigate`] module docs for the full contract and the
//! `tests/no_silent_none.rs` integration suite for the regression
//! guard that pins each path.
//!
//! # Modules
//!
//! - [`types`] — newtype wrappers ([`WindowLabel`], [`SegmentMoniker`],
//!   [`FullyQualifiedMoniker`], [`LayerName`], [`Pixels`]), the
//!   [`Rect`] value type, and the [`Direction`] enum used by the
//!   spatial-nav surface. Every public signature uses these newtypes —
//!   never bare `String` or `f64`.
//!
//! - [`scope`] — the single registered struct type [`FocusScope`] that
//!   describes one point in the spatial-nav tree. Whether a scope is a
//!   leaf or a container is a runtime property of the registry.
//!
//! - [`layer`] — the modal-boundary primitive [`FocusLayer`]. Layers form
//!   a per-window forest; spatial nav, fallback resolution, and scope
//!   tree walks never cross a layer.
//!
//! - [`registry`] — the headless [`SpatialRegistry`] that stores scopes
//!   and layers. Tree / forest structure is derived from `parent_zone`
//!   and `parent` fields rather than stored separately.
//!
//! - [`state`] — the per-window focus tracker [`SpatialState`] plus the
//!   [`FocusChangedEvent`] value adapters emit to the frontend on every
//!   focus mutation.
//!
//! - [`navigate`] — the [`NavStrategy`] trait plus [`BeamNavStrategy`],
//!   the default Android-beam-search algorithm. Pluggable so consumers
//!   can swap in alternate strategies for tests or specialised layouts.
//!
//! - [`observer`] — the [`FocusEventSink`] trait plus [`NoopSink`] and
//!   [`RecordingSink`] for adapters that prefer push-based event delivery
//!   over consuming the [`Option<FocusChangedEvent>`] return value of the
//!   [`SpatialState`] mutators.
//!
//! # Wire format
//!
//! All public types use `serde` with stable JSON shapes — string newtypes
//! serialize transparently as bare strings, [`Pixels`] serializes as a
//! bare number. The frontend mirrors these as branded TypeScript types
//! so a `WindowLabel` and a `Moniker` cannot be mixed up at the Tauri
//! boundary.

pub mod layer;
pub mod navigate;
pub mod observer;
pub mod registry;
pub mod scope;
pub mod snapshot;
pub mod state;
pub mod types;

pub use layer::FocusLayer;
pub use navigate::{pick_target_via_view, BeamNavStrategy, NavScopeView, NavStrategy};
pub use observer::{FocusEventSink, NoopSink, RecordingSink};
pub use registry::{RegisterEntry, SpatialRegistry};
pub use scope::FocusScope;
pub use snapshot::{FocusOverrides, IndexedSnapshot, NavSnapshot, SnapshotScope};
pub use state::{FallbackResolution, FocusChangedEvent, LostFocusContext, SpatialState};
pub use types::{
    Direction, FullyQualifiedMoniker, LayerName, Pixels, Rect, SegmentMoniker, WindowLabel,
};
