//! Spatial (XY) focus navigation state machine and beam-test algorithm.
//!
//! This crate provides two layers:
//!
//! - [`spatial_nav`] — pure function module implementing the Android
//!   FocusFinder beam test and scoring algorithm for cardinal direction
//!   navigation, plus positional edge commands (First/Last/RowStart/RowEnd).
//! - [`spatial_state`] — a thread-safe state machine that owns the entry
//!   registry, layer stack, and focused key, and exposes `navigate()` on
//!   top of the pure algorithm.
//!
//! The crate is consumer-agnostic: it knows nothing about Tauri, kanban,
//! or specific entity types. Its only meaningful dependency is `serde`
//! for serialising wire types like [`FocusChanged`].

pub mod spatial_nav;
pub mod spatial_state;

pub use spatial_nav::{Direction, ParseDirectionError};
pub use spatial_state::{BatchEntry, FocusChanged, LayerEntry, Rect, SpatialEntry, SpatialState};
