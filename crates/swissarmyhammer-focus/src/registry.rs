//! `SpatialRegistry` — the kernel-resident layer store and
//! cross-snapshot focus memory.
//!
//! After the spatial-nav redesign cutover, the registry holds only:
//!
//! - `layers: HashMap<FullyQualifiedMoniker, FocusLayer>` — every
//!   registered layer node, keyed by its FQM. Layer hierarchy is
//!   derived from each layer's `parent` field.
//! - `last_focused_by_fq: HashMap<FQM, FQM>` — most-recently focused
//!   descendant per ancestor scope, keyed by the ancestor's FQM.
//!   Populated by [`SpatialRegistry::record_focus`] for every scope
//!   ancestor walked during a focus event; read by the
//!   `FallbackParentZoneLastFocused` arm of the fallback resolver.
//!
//! Scope geometry is **never** held here — callers ship a fresh
//! [`crate::snapshot::NavSnapshot`] per decision, and the kernel reads
//! the layer-scoped scope set out of that snapshot. Two systems can no
//! longer drift because there is only one source of truth for scope
//! state (the React side), and the kernel sees that state only at the
//! moment of a decision.
//!
//! ## Threading model
//!
//! `SpatialRegistry` is plain data — not `Sync` on its own. Callers wrap
//! it in a `Mutex`/`RwLock` when they need shared mutable access. The
//! kanban-app `AppState` already serializes spatial commands behind a
//! `tokio::sync::Mutex`, so no additional locking lives here.

use std::collections::{HashMap, HashSet};

use super::layer::FocusLayer;
use super::snapshot::IndexedSnapshot;
use super::types::{FullyQualifiedMoniker, WindowLabel};

/// Kernel-resident layer store and cross-snapshot focus memory.
///
/// Holds the layer forest (one `FocusLayer` per registered layer FQM)
/// plus the `last_focused_by_fq` map that records the most-recently
/// focused descendant per ancestor scope. Scope geometry rides on each
/// IPC as a [`crate::snapshot::NavSnapshot`] — the registry holds no
/// scope replica.
#[derive(Debug, Default, Clone)]
pub struct SpatialRegistry {
    /// All registered layers keyed by their canonical
    /// [`FullyQualifiedMoniker`]. Layer hierarchy is derived from each
    /// layer's `parent` field, not stored here.
    layers: HashMap<FullyQualifiedMoniker, FocusLayer>,
    /// Most-recent focused descendant under each ancestor scope, keyed
    /// by the ancestor's [`FullyQualifiedMoniker`]. Populated by
    /// [`Self::record_focus`] for every scope ancestor walked during a
    /// focus event.
    pub last_focused_by_fq: HashMap<FullyQualifiedMoniker, FullyQualifiedMoniker>,
}

impl SpatialRegistry {
    /// Construct an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record `fq` as the most recently focused descendant on every
    /// scope ancestor and every layer ancestor in the chain rooted at
    /// `fq`.
    ///
    /// Phase 1 walks the snapshot's `parent_zone` chain and writes each
    /// visited ancestor's slot in `last_focused_by_fq`. Phase 2 walks
    /// the layer ancestor chain via `FocusLayer::parent`, starting at
    /// the snapshot's layer, and sets each visited layer's
    /// `last_focused` to `fq`. The focused FQM itself is not written
    /// into its own slot — both maps record memory for ancestors only.
    pub fn record_focus(&mut self, fq: &FullyQualifiedMoniker, snapshot: &IndexedSnapshot<'_>) {
        for ancestor in snapshot.parent_zone_chain(fq) {
            self.last_focused_by_fq
                .insert(ancestor.fq.clone(), fq.clone());
        }

        let mut next_layer = Some(snapshot.layer_fq().clone());
        let mut visited_layers: HashSet<FullyQualifiedMoniker> = HashSet::new();
        while let Some(layer_fq) = next_layer {
            if !visited_layers.insert(layer_fq.clone()) {
                tracing::error!(
                    op = "record_focus",
                    cycle_fq = %layer_fq,
                    "layer parent chain cycle detected; breaking walk"
                );
                break;
            }
            let Some(layer) = self.layers.get_mut(&layer_fq) else {
                break;
            };
            layer.last_focused = Some(fq.clone());
            next_layer = layer.parent.clone();
        }
    }

    // ---------------------------------------------------------------------
    // Layer ops
    // ---------------------------------------------------------------------

    /// Register a layer.
    ///
    /// Replaces any prior layer under the same FQM. The "stack" framing
    /// is on the React side (palette opens push, palette closes pop);
    /// the kernel-side store is a flat map keyed by
    /// [`FullyQualifiedMoniker`].
    ///
    /// Re-pushing an existing FQM preserves the prior layer's
    /// `last_focused` slot when the new layer arrives with `None` — the
    /// drill-out memory accumulated by [`Self::record_focus`] survives
    /// StrictMode double-mount, palette open/close cycles, and
    /// IPC re-batches.
    pub fn push_layer(&mut self, mut l: FocusLayer) {
        if l.last_focused.is_none() {
            if let Some(existing) = self.layers.get(&l.fq) {
                l.last_focused = existing.last_focused.clone();
            }
        }

        self.layers.insert(l.fq.clone(), l);
    }

    /// Remove a layer from the registry. No-op if the FQM is unknown.
    pub fn remove_layer(&mut self, fq: &FullyQualifiedMoniker) {
        self.layers.remove(fq);
    }

    /// Borrow a layer by FQM.
    pub fn layer(&self, fq: &FullyQualifiedMoniker) -> Option<&FocusLayer> {
        self.layers.get(fq)
    }

    /// Direct children of a layer — layers whose `parent` equals `fq`.
    pub fn children_of_layer(&self, fq: &FullyQualifiedMoniker) -> Vec<&FocusLayer> {
        self.layers
            .values()
            .filter(|l| l.parent.as_ref() == Some(fq))
            .collect()
    }

    /// The window root layer for `label` — a layer with the matching
    /// `window_label` and `parent = None`.
    ///
    /// Returns `None` if the window has not registered a root layer
    /// yet. Per the layer-forest invariant there is at most one root
    /// per window; if the registry contains more than one due to a bug,
    /// the first found is returned. The invariant is enforced with
    /// `debug_assert!` in dev builds and a `tracing::warn!` in release
    /// builds so the corruption is visible without panicking on a user.
    pub fn root_for_window(&self, label: &WindowLabel) -> Option<&FocusLayer> {
        let roots: Vec<&FocusLayer> = self
            .layers
            .values()
            .filter(|l| l.parent.is_none() && &l.window_label == label)
            .collect();

        debug_assert!(
            roots.len() <= 1,
            "registry corruption: window {label} has {} root layers (expected ≤ 1)",
            roots.len()
        );
        if roots.len() > 1 {
            tracing::warn!(
                window_label = %label,
                root_count = roots.len(),
                "registry corruption: window has multiple root layers; returning first"
            );
        }

        roots.into_iter().next()
    }

    /// Walk the `parent` chain from the layer at `fq` upward, collecting
    /// each ancestor [`FocusLayer`] in innermost-first order.
    ///
    /// The layer at `fq` is **not** included in the result — only its
    /// ancestors. The walk stops at the window root or at a missing
    /// layer reference, whichever comes first.
    pub fn ancestors_of_layer(&self, fq: &FullyQualifiedMoniker) -> Vec<&FocusLayer> {
        let mut chain = Vec::new();
        let Some(start) = self.layers.get(fq) else {
            return chain;
        };

        let mut next = start.parent.clone();
        while let Some(parent_fq) = next {
            let Some(parent) = self.layers.get(&parent_fq) else {
                break;
            };
            chain.push(parent);
            next = parent.parent.clone();
        }
        chain
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::FocusLayer;
    use crate::snapshot::{NavSnapshot, SnapshotScope};
    use crate::types::{FullyQualifiedMoniker, LayerName, Pixels, Rect, SegmentMoniker};
    use std::collections::HashMap;

    fn rect_zero() -> Rect {
        Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        }
    }

    fn make_layer(fq: &str, window: &str, parent: Option<&str>) -> FocusLayer {
        FocusLayer {
            fq: FullyQualifiedMoniker::from_string(fq),
            segment: SegmentMoniker::from_string(fq.rsplit('/').next().unwrap_or(fq)),
            name: LayerName::from_string("window"),
            parent: parent.map(FullyQualifiedMoniker::from_string),
            window_label: WindowLabel::from_string(window),
            last_focused: None,
        }
    }

    #[test]
    fn root_for_window_finds_window_root() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer("/win-a", "win-a", None));
        reg.push_layer(make_layer("/win-a/ins", "win-a", Some("/win-a")));

        let root = reg
            .root_for_window(&WindowLabel::from_string("win-a"))
            .unwrap();
        assert_eq!(root.fq, FullyQualifiedMoniker::from_string("/win-a"));
    }

    #[test]
    fn ancestors_of_layer_walks_chain() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer("/win", "main", None));
        reg.push_layer(make_layer("/win/ins", "main", Some("/win")));
        reg.push_layer(make_layer("/win/ins/dlg", "main", Some("/win/ins")));

        let chain: Vec<_> = reg
            .ancestors_of_layer(&FullyQualifiedMoniker::from_string("/win/ins/dlg"))
            .into_iter()
            .map(|l| l.fq.as_str().to_string())
            .collect();
        assert_eq!(chain, vec!["/win/ins".to_string(), "/win".to_string()]);
    }

    #[test]
    fn record_focus_writes_last_focused_by_fq_for_each_ancestor() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer("/L", "main", None));

        let snapshot = NavSnapshot {
            layer_fq: FullyQualifiedMoniker::from_string("/L"),
            scopes: vec![
                SnapshotScope {
                    fq: FullyQualifiedMoniker::from_string("/L/zone"),
                    rect: rect_zero(),
                    parent_zone: None,
                    nav_override: HashMap::new(),
                    focusable: true,
                },
                SnapshotScope {
                    fq: FullyQualifiedMoniker::from_string("/L/zone/leaf"),
                    rect: rect_zero(),
                    parent_zone: Some(FullyQualifiedMoniker::from_string("/L/zone")),
                    nav_override: HashMap::new(),
                    focusable: true,
                },
            ],
        };
        let indexed = IndexedSnapshot::new(&snapshot);
        let leaf = FullyQualifiedMoniker::from_string("/L/zone/leaf");
        reg.record_focus(&leaf, &indexed);

        assert_eq!(
            reg.last_focused_by_fq
                .get(&FullyQualifiedMoniker::from_string("/L/zone")),
            Some(&leaf),
            "ancestor zone records the focused descendant",
        );
        assert_eq!(
            reg.layer(&FullyQualifiedMoniker::from_string("/L"))
                .and_then(|l| l.last_focused.clone()),
            Some(leaf),
            "layer records the focused descendant",
        );
    }

    #[test]
    fn push_layer_preserves_last_focused_on_re_push_with_none() {
        let mut reg = SpatialRegistry::new();
        let mut layer = make_layer("/L", "main", None);
        layer.last_focused = Some(FullyQualifiedMoniker::from_string("/L/leaf"));
        reg.push_layer(layer);

        // Re-push with `last_focused = None` — the slot survives.
        reg.push_layer(make_layer("/L", "main", None));
        assert_eq!(
            reg.layer(&FullyQualifiedMoniker::from_string("/L"))
                .and_then(|l| l.last_focused.clone()),
            Some(FullyQualifiedMoniker::from_string("/L/leaf")),
        );
    }
}
