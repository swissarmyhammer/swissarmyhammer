//! `FocusScope` — the single registered struct type that describes one
//! point in the spatial-nav tree.
//!
//! `FocusScope` peers with the React `<FocusScope>` component. Whether a
//! given scope is a leaf (atomic, no children) or a container (has
//! navigable children) is a **runtime property** of the registry
//! (something registered under it), not a type-level distinction. UI
//! authoring stays simple: the consumer mounts a `<FocusScope>` and the
//! kernel decides what role it plays based on what else mounts beneath
//! it.
//!
//! Together with [`super::layer::FocusLayer`] these form the **two peer
//! types** the spatial-nav kernel exposes. A scope that has children
//! acts as a navigable container; a scope with no children acts as a
//! leaf. The same struct serves both roles — see
//! [`SpatialRegistry::children_of`] for the "has children" query.
//!
//! ## Identity model — the FQM is the key
//!
//! Every scope carries two identity fields:
//!
//! - `fq: FullyQualifiedMoniker` — the canonical path and registry
//!   key. Composed by the consumer side (React `FullyQualifiedMonikerContext`)
//!   from the parent FQM and the consumer's declared segment. Two
//!   registrations under the same FQM are a programmer mistake; the
//!   kernel surfaces the duplicate via `tracing::error!` and lets the
//!   second registration replace the first (idempotent on remount).
//! - `segment: SegmentMoniker` — the relative segment the consumer
//!   declared (e.g. `field:T1.title`, `card:T1`, `inspector`). Carried
//!   for human-readable logging only — the kernel never keys lookups on
//!   the segment, since the same segment can legitimately appear at two
//!   distinct FQMs (a board card field and an inspector panel field, for
//!   example).
//!
//! [`SpatialRegistry`]: super::registry::SpatialRegistry
//! [`SpatialRegistry::children_of`]: super::registry::SpatialRegistry::children_of

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::types::{Direction, FullyQualifiedMoniker, Rect, SegmentMoniker};

/// A focus scope — a registered point in the spatial-nav tree.
///
/// Every consumer-mounted `<FocusScope>` shows up here. Whether the
/// scope is a leaf (no registered children) or a navigable container
/// (something registered under it) is a runtime property of the
/// registry, not a type-level distinction.
///
/// Each `FocusScope` has a canonical [`FullyQualifiedMoniker`] `fq`
/// (the registry key, e.g.
/// `/window/board/column:todo/card:T1/field:T1.title`), a relative
/// `segment` (e.g. `field:T1.title`, used for human-readable logs
/// only), a screen-coordinate `rect`, and a `layer_fq` pointing at
/// the [`super::layer::FocusLayer`] it lives in.
///
/// `parent_zone` is the FQM of the immediate enclosing scope, or
/// `None` if this scope is registered directly under its layer root.
/// `overrides` per-direction lets the React side hard-wire a navigation
/// target (or a `None` "wall") without round-tripping through beam
/// search.
///
/// `last_focused` is drill-out / fallback memory: the FQM of the most
/// recently focused descendant inside this scope. Populated by the
/// kernel as focus moves — see
/// [`super::registry::SpatialRegistry::record_focus`], invoked by
/// [`super::state::SpatialState::focus`] (and any other code path that
/// mutates the per-window focus slot) on every successful focus
/// transition. On a leaf scope (no children) the slot stays `None` for
/// the lifetime of the registration. The kernel also preserves an
/// existing value across re-registration via
/// [`super::registry::SpatialRegistry::register_scope`] so drill-out
/// memory survives the placeholder/real-mount swap.
///
/// This struct is the Rust peer of the React `<FocusScope>` component.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FocusScope {
    /// Canonical FQM — the registry key. Composed by the consumer from
    /// the parent FQM and the consumer's declared segment.
    pub fq: FullyQualifiedMoniker,
    /// Relative segment the consumer declared. Carried for
    /// human-readable logging only; the kernel keys on `fq`.
    pub segment: SegmentMoniker,
    /// Bounding rect in viewport coordinates. Drives beam-search distance
    /// and overlap math; updated via [`super::registry::SpatialRegistry::update_rect`]
    /// when ResizeObserver fires on the React side.
    pub rect: Rect,
    /// FQM of the layer this scope belongs to. Beam search and
    /// ancestor walks never cross a layer boundary.
    pub layer_fq: FullyQualifiedMoniker,
    /// Immediate enclosing scope's FQM, if any. `None` means this scope
    /// is registered directly under the layer root.
    pub parent_zone: Option<FullyQualifiedMoniker>,
    /// Drill-out / fallback memory: the FQM of the most recently focused
    /// descendant inside this scope. `None` when the scope has never
    /// captured focus inside (or when it is a leaf with no children at
    /// all). Populated by the kernel as focus moves —
    /// [`super::registry::SpatialRegistry::record_focus`] writes this
    /// slot for every scope ancestor of a newly focused FQM. The kernel
    /// also preserves an existing value across re-registration in
    /// [`super::registry::SpatialRegistry::register_scope`], so the
    /// drill-out memory survives the placeholder/real-mount swap.
    pub last_focused: Option<FullyQualifiedMoniker>,
    /// Per-direction navigation overrides. `Some(target_fq)` redirects
    /// nav to the named FQM; `None` is an explicit "wall" that blocks
    /// navigation in that direction. Missing key means "fall through to
    /// beam search".
    pub overrides: HashMap<Direction, Option<FullyQualifiedMoniker>>,
}

#[cfg(test)]
mod tests {
    //! Unit-level coverage for [`FocusScope`] field defaults. Mirrors the
    //! integration tests in `tests/focus_registry.rs` so contract drift
    //! is caught at the inner-crate compile step.

    use super::*;
    use crate::types::Pixels;

    fn rect_zero() -> Rect {
        Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(0.0),
            height: Pixels::new(0.0),
        }
    }

    #[test]
    fn focus_scope_holds_path_and_segment() {
        let scope = FocusScope {
            fq: FullyQualifiedMoniker::from_string("/L/k"),
            segment: SegmentMoniker::from_string("k"),
            rect: rect_zero(),
            layer_fq: FullyQualifiedMoniker::from_string("/L"),
            parent_zone: None,
            last_focused: None,
            overrides: HashMap::new(),
        };

        assert_eq!(scope.fq, FullyQualifiedMoniker::from_string("/L/k"));
        assert_eq!(scope.segment, SegmentMoniker::from_string("k"));
        assert_eq!(scope.last_focused, None);
    }
}
