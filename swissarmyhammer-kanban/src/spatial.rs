//! Spatial navigation provider trait.
//!
//! Cardinal and edge navigation commands (`nav.up`, `nav.down`, `nav.left`,
//! `nav.right`, `nav.first`, `nav.last`, `nav.rowStart`, `nav.rowEnd`) run
//! through the same unified command dispatch pipeline as `ui.inspect` or
//! `task.move`. Their Rust handlers (in
//! [`crate::commands::nav_commands`]) depend on a small provider abstraction
//! so they can drive `SpatialState::navigate` on the correct per-window
//! instance and emit the corresponding `focus-changed` event without
//! depending on any Tauri-specific types.
//!
//! The Tauri binary (`kanban-app`) implements this trait against its
//! `AppState` + `AppHandle`, and installs it on every
//! [`swissarmyhammer_commands::CommandContext`] as a typed extension via
//! [`SpatialNavigatorExt`]. Integration tests and fixtures implement it
//! against an in-memory `SpatialState` + a test-side event sink.
//!
//! This indirection is why nav commands can live in `swissarmyhammer-kanban`:
//! the trait takes only primitives (window label, direction) and returns a
//! moniker string — no `WebviewWindow`, no `AppHandle`, no `Arc<AppState>`
//! leak into the command-layer crate.

use std::sync::Arc;
use swissarmyhammer_spatial_nav::Direction;

/// Provider for per-window spatial navigation.
///
/// Implementations hold (or can resolve) the `SpatialState` for each window
/// label and are responsible for both driving navigation and fanning out the
/// resulting `focus-changed` event to the correct window.
///
/// Returned value: the moniker of the newly focused entry when focus
/// actually moved, or `None` when the navigation was a no-op (no active
/// layer, blocked by override, or focus was already at an edge).
#[async_trait::async_trait]
pub trait SpatialNavigator: Send + Sync {
    /// Navigate focus in the given direction for the named window.
    ///
    /// The provider reads the window's current focused key as the source,
    /// applies `SpatialState::navigate`, emits a `focus-changed` event
    /// scoped to that window if focus moved, and returns the moniker of
    /// the new focus (or `None` when no target was resolved).
    ///
    /// # Errors
    ///
    /// Propagates errors from `SpatialState::navigate` as strings (e.g. a
    /// beam-test internal failure). Returns `Ok(None)` — not an error —
    /// when the navigation itself resolved no target.
    async fn navigate(
        &self,
        window_label: &str,
        direction: Direction,
    ) -> Result<Option<String>, String>;
}

/// Newtype wrapper for `Arc<dyn SpatialNavigator>` so it can be stored as a
/// `CommandContext` extension.
///
/// `CommandContext::set_extension` keys by `TypeId`, which requires a sized
/// concrete type. Trait objects behind `Arc` are unsized, so we wrap them
/// in a struct and store `Arc<SpatialNavigatorExt>`.
pub struct SpatialNavigatorExt(pub Arc<dyn SpatialNavigator>);
