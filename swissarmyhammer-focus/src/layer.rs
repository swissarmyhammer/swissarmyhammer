//! `FocusLayer` — the modal-boundary primitive of the spatial-nav kernel.
//!
//! A layer is a **hard modal boundary**. Spatial nav, fallback resolution,
//! and zone-tree walks **never cross a layer**. Layers form a forest:
//! each Tauri window has its own root layer (`name = "window"`,
//! `parent = None`); inspector / dialog / palette overlays are stacked
//! child layers under their parent layer.
//!
//! Examples:
//! - `window` — root layer, one per Tauri webview.
//! - `inspector` — one per window when any inspector panel is open.
//! - `dialog` — modal dialogs.
//! - `palette` — the command palette overlay.
//!
//! A layer is *not* itself focusable — you don't navigate "to" a layer;
//! you navigate within the active focus's layer. Layers are stored in
//! [`super::registry::SpatialRegistry`] keyed by their
//! [`FullyQualifiedMoniker`].

use serde::{Deserialize, Serialize};

use super::types::{FullyQualifiedMoniker, LayerName, SegmentMoniker, WindowLabel};

/// One node in the layer forest.
///
/// `fq` is the canonical [`FullyQualifiedMoniker`] for the layer (e.g.
/// `/window`, `/window/inspector`); `segment` is the relative segment
/// the consumer declared (e.g. `window`, `inspector`); `name` is the
/// role discriminator (`"window"`, `"inspector"`, `"dialog"`,
/// `"palette"`); `parent` is the stacking parent's FQM, or `None` for a
/// window root. `window_label` ties the layer to its Tauri webview so
/// [`super::registry::SpatialRegistry::root_for_window`] can find the
/// right root for a per-window operation.
///
/// `last_focused` is the drill-out / fallback memory: when a layer is
/// dismissed (palette closed, dialog accepted), the navigator restores
/// focus to the layer's parent and consults the parent's `last_focused`
/// to land somewhere meaningful.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FocusLayer {
    /// Canonical FQM for this layer mount.
    pub fq: FullyQualifiedMoniker,
    /// Relative segment the consumer declared (used for human-readable
    /// logs only — the kernel keys on `fq`).
    pub segment: SegmentMoniker,
    /// Role discriminator: `"window"`, `"inspector"`, `"dialog"`,
    /// `"palette"`. Free-form on the wire so future overlay kinds don't
    /// require a Rust-side enum bump.
    pub name: LayerName,
    /// Stacking parent's FQM. `None` for a window root.
    pub parent: Option<FullyQualifiedMoniker>,
    /// Tauri window label this layer lives in. Every layer in a forest
    /// path back to a root shares the same `window_label` — the registry
    /// does not validate this invariant, but breaking it would let nav
    /// cross windows by accident.
    pub window_label: WindowLabel,
    /// Drill-out / fallback memory: most recently focused scope inside
    /// this layer, keyed by FQM. Populated by the navigator when focus
    /// changes within the layer; consulted on layer dismissal to restore
    /// focus.
    pub last_focused: Option<FullyQualifiedMoniker>,
}
