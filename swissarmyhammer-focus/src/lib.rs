//! Spatial focus and keyboard navigation engine.
//!
//! This crate provides the headless spatial-navigation kernel used by GUI
//! consumers (Tauri adapters, CLI front-ends, tests) to track keyboard
//! focus across a 2-D layout and move it in cardinal directions.
//!
//! The crate is **generic and domain-free**: nothing in here knows about
//! kanban tasks, columns, projects, or any other application concept.
//! Identities are opaque [`Moniker`] strings produced by the consumer; the
//! kernel only sees rectangles, layers, and zones.
//!
//! # No-silent-dropout contract
//!
//! Nav and drill APIs always return a [`Moniker`]. "No motion possible"
//! is communicated by returning the focused entry's own moniker — the
//! React side detects "stay put" by comparing the returned moniker to
//! the previous focused moniker. Torn state (unknown key, orphan
//! parent reference) emits `tracing::error!` and echoes the input
//! moniker so the call site has a valid result. There is no [`Option`]
//! or [`Result`] on these APIs; silence is impossible. See the
//! [`navigate`] module docs for the full contract and the
//! `tests/no_silent_none.rs` integration suite for the regression
//! guard that pins each path.
//!
//! # Modules
//!
//! - [`types`] — newtype wrappers ([`WindowLabel`], [`SpatialKey`],
//!   [`LayerKey`], [`Moniker`], [`LayerName`], [`Pixels`]), the [`Rect`]
//!   value type, and the [`Direction`] enum used by the spatial-nav
//!   surface. Every public signature uses these newtypes — never bare
//!   `String` or `f64`.
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
pub use registry::{BatchRegisterError, ChildScope, RegisterEntry, ScopeKind, SpatialRegistry};
pub use scope::{FocusScope, FocusZone};
pub use state::{FallbackResolution, FocusChangedEvent, SpatialState};
pub use types::{Direction, LayerKey, LayerName, Moniker, Pixels, Rect, SpatialKey, WindowLabel};
