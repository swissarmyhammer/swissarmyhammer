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
//! sees rectangles, layers, and zones.
//!
//! # Navigation rules
//!
//! Cardinal navigation obeys one load-bearing contract: **within a
//! parent [`FocusZone`], child [`FocusScope`] leaves and child
//! [`FocusZone`] containers are siblings.** Iter 0 of the cascade
//! considers any-kind in-zone candidates and lets the Android beam
//! score pick the geometrically best one. Only iter 1 (escalation to
//! peer-zone level) restricts candidates to zones — that's structural,
//! not a kind policy, because the parent IS a zone.
//!
//! Edge commands ([`Direction::First`], [`Direction::Last`],
//! [`Direction::RowStart`], [`Direction::RowEnd`]) keep level-bounded
//! same-kind semantics — `Home` in a row of cells means "first cell",
//! not "row container".
//!
//! See the crate README (`swissarmyhammer-focus/README.md`) for the
//! prose contract with diagrams, the anti-pattern callout against
//! re-introducing kind filters at iter 0, and the cascade walkthrough.
//! The [`navigate`] module docs cover the full algorithm and the
//! `tests/in_zone_any_kind_first.rs` integration suite pins each
//! iter-0 trajectory.
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
//! - [`scope`] — the two registered struct types that describe a point in
//!   the spatial-nav tree: [`FocusScope`] leaves and [`FocusZone`]
//!   containers. There is no public sum-type enum; the registry stores
//!   them via an internal discriminator and exposes typed accessors.
//!
//! - [`layer`] — the modal-boundary primitive [`FocusLayer`]. Layers form
//!   a per-window forest; spatial nav, fallback resolution, and zone tree
//!   walks never cross a layer.
//!
//! - [`registry`] — the headless [`SpatialRegistry`] that stores scopes
//!   and layers. Tree / forest structure is derived from `parent_zone` and
//!   `parent` fields rather than stored separately.
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
//! bare number, [`RegisterEntry`] uses a `kind` discriminator with
//! `snake_case` rename. The frontend mirrors these as branded TypeScript
//! types so a `WindowLabel` and a `Moniker` cannot be mixed up at the
//! Tauri boundary.

pub mod layer;
pub mod navigate;
pub mod observer;
pub mod registry;
pub mod scope;
pub mod state;
pub mod types;

pub use layer::FocusLayer;
pub use navigate::{BeamNavStrategy, NavStrategy};
pub use observer::{FocusEventSink, NoopSink, RecordingSink};
pub use registry::{
    BatchRegisterError, ChildScope, FocusEntry, RegisterEntry, ScopeKind, SpatialRegistry,
};
pub use scope::{FocusScope, FocusZone};
pub use state::{FallbackResolution, FocusChangedEvent, SpatialState};
pub use types::{
    Direction, FullyQualifiedMoniker, LayerName, Pixels, Rect, SegmentMoniker, WindowLabel,
};
