//! Per-window focus state.
//!
//! `SpatialState` is the **headless** focus tracker that backs the spatial-
//! nav Tauri commands. It owns exactly one piece of mutable state:
//!
//! - A per-window focus map: `HashMap<WindowLabel, FullyQualifiedMoniker>`.
//!   Every Tauri window has its own focused element; focus moves in
//!   window A do not perturb window B's slot.
//!
//! Layer membership and layer ancestry come from [`SpatialRegistry`]; the
//! per-decision scope geometry rides on every focus-mutating call as a
//! [`crate::snapshot::NavSnapshot`]. There is no per-FQM "entry" map on
//! `SpatialState`.
//!
//! Mutating methods return [`Option<FocusChangedEvent>`] **instead of**
//! emitting on a Tauri channel directly. This keeps the focus crate
//! testable without a Tauri runtime and pushes the side-effect (`emit`)
//! up to the adapter layer in `kanban-app/src/commands.rs`.
//!
//! ## Threading model
//!
//! `SpatialState` is plain data — not `Sync` on its own. Callers wrap it
//! in a `Mutex`/`RwLock` when they need shared mutable access. Adapters
//! that mutate both the registry and the state should hold both locks
//! together for the duration of the transaction so observers cannot
//! see a half-applied registration.
//!
//! [`SpatialRegistry`]: crate::registry::SpatialRegistry

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::registry::SpatialRegistry;
use super::snapshot::{IndexedSnapshot, NavSnapshot};
use super::types::{
    pixels_cmp, Direction, FullyQualifiedMoniker, Pixels, Rect, SegmentMoniker, WindowLabel,
};

/// Payload emitted to React whenever the focused FQM for a window
/// changes.
///
/// The frontend's claim registry (`Map<FullyQualifiedMoniker, (focused) => void>`)
/// dispatches `false` to `prev_fq` and `true` to `next_fq`, so the wire
/// shape is exactly what one cell on either side of a focus move needs to
/// re-render. `next_segment` is included so consumers that key off the
/// relative segment (rather than the FQM) can update without an extra
/// IPC round-trip.
///
/// `prev_fq` is `None` when the window had no prior focus (cold-start, or
/// the previously focused scope was just unregistered). `next_fq` is
/// `None` when focus is being cleared (e.g. the focused scope unmounted
/// and there is no obvious replacement). Both fields independent — focus
/// transfer (`Some(prev) → Some(next)`), focus acquisition
/// (`None → Some(next)`), and focus clear (`Some(prev) → None`) all flow
/// through the same payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusChangedEvent {
    /// Window in which the focus changed. Mirrors the [`WindowLabel`]
    /// the originating Tauri command derived from its `tauri::Window`
    /// parameter, so the frontend's per-window claim registry can
    /// ignore events for other windows.
    pub window_label: WindowLabel,
    /// Previously focused FQM in this window, if any.
    pub prev_fq: Option<FullyQualifiedMoniker>,
    /// Newly focused FQM in this window, if any.
    pub next_fq: Option<FullyQualifiedMoniker>,
    /// Relative segment of the newly focused entity, if `next_fq.is_some()`.
    /// Derived from the trailing path component of `next_fq`.
    pub next_segment: Option<SegmentMoniker>,
}

/// Result of a scope-aware focus fallback computation.
///
/// Produced by [`SpatialState::resolve_fallback`] when the focused entry
/// is about to be unregistered. Each "found" variant carries the resolved
/// target's [`FullyQualifiedMoniker`] and [`SegmentMoniker`] — the focus
/// tracker uses them to update `focus_by_window`, and the adapter uses
/// them to emit the outgoing [`FocusChangedEvent`].
///
/// The variant communicates **how** the resolver arrived at the target
/// so consumers (mostly tests, and tracing in the adapter) can reason
/// about the precise rule that applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackResolution {
    /// Target is a sibling of the lost entry in the same parent scope
    /// (rule 1).
    FallbackSiblingInZone(FullyQualifiedMoniker, SegmentMoniker),
    /// Target is the parent scope's `last_focused_by_fq` slot, still
    /// present in the snapshot (rule 2 preferred).
    FallbackParentZoneLastFocused(FullyQualifiedMoniker, SegmentMoniker),
    /// Target is the nearest entry in an ancestor scope, used when the
    /// preferred `last_focused_by_fq` is stale or absent (rule 2 fallback).
    FallbackParentZoneNearest(FullyQualifiedMoniker, SegmentMoniker),
    /// Target is the ancestor layer's `last_focused` slot, still
    /// reachable through the parent layer's snapshot context (rule 4
    /// preferred path).
    FallbackParentLayerLastFocused(FullyQualifiedMoniker, SegmentMoniker),
    /// Rule 4's nearest-in-layer fallback. The kernel emits this variant
    /// only when an ancestor layer's `last_focused` was set but the
    /// stored target is no longer reachable; the snapshot only carries
    /// the lost layer's scopes, so the kernel cannot pick a replacement
    /// inside an ancestor layer on its own. The kernel returns
    /// [`Self::NoFocus`] in that case and the React side may issue a
    /// fresh `spatial_focus` against an ancestor-layer snapshot.
    FallbackParentLayerNearest(FullyQualifiedMoniker, SegmentMoniker),
    /// No live fallback target exists in the lost entry's window
    /// (rule 5). The caller clears the window's focus slot.
    NoFocus,
}

/// Carries the lost FQM's metadata that the snapshot cannot supply on
/// its own.
///
/// The snapshot enumerates only live scopes; the lost FQM has already
/// been removed by React before the IPC fires, so its `parent_zone`,
/// `layer_fq`, and `rect` ride alongside the snapshot rather than inside
/// it.
#[derive(Debug, Clone)]
pub struct LostFocusContext<'a> {
    /// Layer-scoped view of the live scopes in the lost FQM's layer.
    pub view: &'a IndexedSnapshot<'a>,
    /// FQM of the layer the lost FQM lived in. Drives the layer-tree
    /// walk in phase 2 and the per-window barrier check.
    pub lost_layer_fq: FullyQualifiedMoniker,
    /// `parent_zone` of the lost FQM, or `None` when it was registered
    /// directly under the layer root. Seeds the phase 1 walk.
    pub lost_parent_zone: Option<FullyQualifiedMoniker>,
    /// Bounding rect of the lost FQM at the moment it was unregistered.
    /// Used for the nearest-neighbor distance scoring.
    pub lost_rect: Rect,
}

/// Pick the nearest live FQM in `view` whose `parent_zone == zone_fq`,
/// excluding `lost_fq` from the candidate set.
///
/// "Nearest" is Euclidean-square distance between rect origins; ties
/// break by `top` then `left` so the choice is deterministic on
/// identical-position rects.
fn nearest_in_zone_view(
    view: &IndexedSnapshot<'_>,
    zone_fq: &Option<FullyQualifiedMoniker>,
    lost_fq: &FullyQualifiedMoniker,
    origin_rect: Rect,
) -> Option<FullyQualifiedMoniker> {
    view.scopes()
        .iter()
        .filter(|s| &s.fq != lost_fq)
        .filter(|s| match zone_fq {
            Some(zk) => s.parent_zone.as_ref() == Some(zk),
            None => s.parent_zone.is_none(),
        })
        .min_by(|a, b| {
            let da = squared_distance(origin_rect, a.rect);
            let db = squared_distance(origin_rect, b.rect);
            pixels_cmp(da, db)
                .then(pixels_cmp(a.rect.top(), b.rect.top()))
                .then(pixels_cmp(a.rect.left(), b.rect.left()))
        })
        .map(|s| s.fq.clone())
}

/// Resolve the parent_zone of `fq` through `view`. Returns `None` when
/// `fq` is not in the view or when its `parent_zone` is `None`.
fn parent_zone_of(
    view: &IndexedSnapshot<'_>,
    fq: &FullyQualifiedMoniker,
) -> Option<FullyQualifiedMoniker> {
    view.get(fq).and_then(|s| s.parent_zone.clone())
}

/// Squared Euclidean distance between two rect origins, in
/// `Pixels`-typed scalar form.
///
/// Squared (rather than rooted) because we only care about ordering and
/// `sqrt` would buy nothing while spending a transcendental op per
/// candidate. `Pixels * f64` keeps the arithmetic in newtype-land.
fn squared_distance(a: Rect, b: Rect) -> Pixels {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    Pixels::new(dx.value() * dx.value() + dy.value() * dy.value())
}

/// Headless per-window focus tracker.
///
/// Owned by `AppState` (in `kanban-app`), consulted by every spatial-nav
/// Tauri command. The struct is not `Sync` on its own — callers wrap it in
/// a `Mutex`/`RwLock` if they need shared mutable access (the GUI side
/// already serializes spatial commands behind `AppState`'s lock, so the
/// inner type is intentionally just the data).
#[derive(Debug, Default, Clone)]
pub struct SpatialState {
    /// The currently focused [`FullyQualifiedMoniker`] **per window**.
    /// Looking up a `WindowLabel` that does not appear here yields no
    /// focus for that window — distinct from "focus is the same FQM in
    /// two windows", which is impossible because each window owns its
    /// own slot.
    focus_by_window: HashMap<WindowLabel, FullyQualifiedMoniker>,
}

impl SpatialState {
    /// Construct an empty `SpatialState`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Move focus to `fq`, scoped to `window`.
    ///
    /// `window` is the **authoritative** owning window — the label the
    /// adapter derived from the calling `tauri::Window`. Pass `Some(label)`
    /// from any window-aware caller; the kernel uses it verbatim for the
    /// emitted event and the per-window focus slot.
    ///
    /// `None` falls back to the layer-derived window
    /// (`registry.layer(snapshot.layer_fq).window_label`) for callers with
    /// no ambient window (the MCP face). **The fallback is unreliable
    /// across windows**: every window's root layer FQM is `/window`, so the
    /// registry — keyed by FQM — holds whichever window pushed last, and a
    /// second window clobbers the first's `window_label`. That is the
    /// "navigation moves focus in the wrong window / both windows" bug.
    /// The Tauri path always passes `Some(window)` so it never hits the
    /// ambiguous fallback.
    ///
    /// The segment is the trailing path component of `fq`. The walk
    /// invoked by [`SpatialRegistry::record_focus`] reads scope ancestry
    /// from `snapshot` and layer ancestry from `registry`.
    ///
    /// Returns `None` when:
    /// - `fq` is not present in `snapshot.scopes` (torn payload), or
    /// - the snapshot's `layer_fq` does not resolve to a registered
    ///   layer (cold start before the React side pushed the layer), or
    /// - the resolved FQM is already focused in its window (no-op so
    ///   adapters do not emit redundant `focus-changed` events).
    pub fn focus(
        &mut self,
        registry: &mut SpatialRegistry,
        snapshot: &NavSnapshot,
        fq: FullyQualifiedMoniker,
        window: Option<WindowLabel>,
    ) -> Option<FocusChangedEvent> {
        let Some(layer) = registry.layer(&snapshot.layer_fq) else {
            // The snapshot names a layer the kernel has never seen (or has
            // already popped). This is the silent-drop path that made the
            // window-root focus regression invisible: every board / toolbar
            // click commits against `layer_fq = /window`, and if the window
            // layer is not in the registry the commit vanishes with no event.
            // Log it at `warn` so a layer/registry desync surfaces in
            // `just logs` instead of presenting as "clicks do nothing".
            tracing::warn!(
                op = "focus",
                focused_fq = %fq,
                layer_fq = %snapshot.layer_fq,
                "focus snapshot names an unregistered layer; dropping commit \
                 (window-root layer not pushed, or popped out from under a live scope)"
            );
            return None;
        };
        // The calling window is authoritative; fall back to the layer's
        // (cross-window-ambiguous) label only for window-unaware callers.
        let window = window.unwrap_or_else(|| layer.window_label.clone());

        let indexed = IndexedSnapshot::new(snapshot);
        if indexed.get(&fq).is_none() {
            tracing::debug!(
                op = "focus",
                focused_fq = %fq,
                layer_fq = %snapshot.layer_fq,
                "focus target missing from snapshot; dropping commit"
            );
            return None;
        }

        let prev_fq = self.focus_by_window.get(&window).cloned();
        if prev_fq.as_ref() == Some(&fq) {
            return None;
        }

        let segment = fq.last_segment();
        self.focus_by_window.insert(window.clone(), fq.clone());
        registry.record_focus(&fq, &indexed);
        Some(FocusChangedEvent {
            window_label: window,
            prev_fq,
            next_fq: Some(fq),
            next_segment: Some(segment),
        })
    }

    /// React to the focused scope unmounting on the React side, computing
    /// a snapshot-driven focus fallback.
    ///
    /// React calls this from its layer registry's deletion path when the
    /// scope being unmounted equals the currently focused FQM in the
    /// window. The lost FQM is **not** present in `snapshot.scopes`
    /// (already removed before the snapshot was built), so its
    /// `parent_zone`, owning layer FQM, and bounding rect arrive on the
    /// wire alongside the snapshot.
    ///
    /// If `lost_fq` is no longer the focused slot for any window, this is
    /// a no-op returning `None`.
    pub fn focus_lost(
        &mut self,
        registry: &mut SpatialRegistry,
        snapshot: &NavSnapshot,
        lost_fq: &FullyQualifiedMoniker,
        lost_parent_zone: Option<&FullyQualifiedMoniker>,
        lost_layer_fq: &FullyQualifiedMoniker,
        lost_rect: Rect,
        window: Option<WindowLabel>,
    ) -> Option<FocusChangedEvent> {
        // The calling window is authoritative when supplied: only react if
        // `lost_fq` is actually the focused slot for *that* window. Falling
        // back to a value-search over `focus_by_window` is ambiguous because
        // FQMs are not unique across windows — the same card FQM can be the
        // focused slot in two windows, and the search would pick an arbitrary
        // one. The window-aware (Tauri) path always passes `Some(window)`.
        let window = match window {
            Some(w) => {
                if self.focus_by_window.get(&w) != Some(lost_fq) {
                    return None;
                }
                w
            }
            None => self
                .focus_by_window
                .iter()
                .find(|(_, focused)| *focused == lost_fq)
                .map(|(w, _)| w.clone())?,
        };

        let indexed = IndexedSnapshot::new(snapshot);
        let ctx = LostFocusContext {
            view: &indexed,
            lost_layer_fq: lost_layer_fq.clone(),
            lost_parent_zone: lost_parent_zone.cloned(),
            lost_rect,
        };
        let resolution = self.resolve_fallback(registry, lost_fq, &ctx);
        Some(self.commit_fallback_resolution(registry, &window, lost_fq, resolution, &indexed))
    }

    /// Commit a [`FallbackResolution`] to `focus_by_window` and produce
    /// the matching [`FocusChangedEvent`].
    fn commit_fallback_resolution(
        &mut self,
        registry: &mut SpatialRegistry,
        window: &WindowLabel,
        prev_fq: &FullyQualifiedMoniker,
        resolution: FallbackResolution,
        snapshot_view: &IndexedSnapshot<'_>,
    ) -> FocusChangedEvent {
        match resolution {
            FallbackResolution::NoFocus => {
                self.focus_by_window.remove(window);
                FocusChangedEvent {
                    window_label: window.clone(),
                    prev_fq: Some(prev_fq.clone()),
                    next_fq: None,
                    next_segment: None,
                }
            }
            FallbackResolution::FallbackSiblingInZone(next_fq, next_segment)
            | FallbackResolution::FallbackParentZoneLastFocused(next_fq, next_segment)
            | FallbackResolution::FallbackParentZoneNearest(next_fq, next_segment)
            | FallbackResolution::FallbackParentLayerLastFocused(next_fq, next_segment)
            | FallbackResolution::FallbackParentLayerNearest(next_fq, next_segment) => {
                self.focus_by_window.insert(window.clone(), next_fq.clone());
                registry.record_focus(&next_fq, snapshot_view);
                FocusChangedEvent {
                    window_label: window.clone(),
                    prev_fq: Some(prev_fq.clone()),
                    next_fq: Some(next_fq),
                    next_segment: Some(next_segment),
                }
            }
        }
    }

    /// Compute the fallback for a lost FQM using a [`NavSnapshot`] for
    /// the live-scope walk.
    ///
    /// The lost FQM is not present in the snapshot (already unregistered
    /// on the React side), so its `parent_zone`, `layer_fq`, and `rect`
    /// ride alongside in [`LostFocusContext`].
    pub fn resolve_fallback(
        &self,
        registry: &SpatialRegistry,
        lost_fq: &FullyQualifiedMoniker,
        ctx: &LostFocusContext<'_>,
    ) -> FallbackResolution {
        let Some(lost_window) = registry
            .layer(&ctx.lost_layer_fq)
            .map(|l| l.window_label.clone())
        else {
            return FallbackResolution::NoFocus;
        };

        let mut current_zone: Option<FullyQualifiedMoniker> = ctx.lost_parent_zone.clone();

        // Phase 1: scope-tree walk inside the lost layer.
        let mut is_first_iteration = true;
        loop {
            let on_lost_zone = is_first_iteration;
            if !on_lost_zone {
                if let Some(zone_fq) = &current_zone {
                    if let Some(remembered) = registry.last_focused_by_fq.get(zone_fq).cloned() {
                        if &remembered != lost_fq {
                            if let Some(scope) = ctx.view.get(&remembered) {
                                return FallbackResolution::FallbackParentZoneLastFocused(
                                    scope.fq.clone(),
                                    scope.fq.last_segment(),
                                );
                            }
                        }
                    }
                }
            }

            if let Some(next_fq) =
                nearest_in_zone_view(ctx.view, &current_zone, lost_fq, ctx.lost_rect)
            {
                let segment = next_fq.last_segment();
                return if on_lost_zone {
                    FallbackResolution::FallbackSiblingInZone(next_fq, segment)
                } else {
                    FallbackResolution::FallbackParentZoneNearest(next_fq, segment)
                };
            }

            let Some(zone_fq) = current_zone else {
                break;
            };
            current_zone = parent_zone_of(ctx.view, &zone_fq);
            is_first_iteration = false;
        }

        // Phase 2: layer-tree walk. Layer edges live on the registry.
        // The snapshot is layer-scoped, so a parent-layer last_focused
        // can only commit if the recorded target is still in the lost
        // layer's snapshot — which is the common drill-out case (the
        // parent zone's recorded child happens to live in the same
        // layer as the lost focus).
        let mut current_layer_parent = registry
            .layer(&ctx.lost_layer_fq)
            .and_then(|l| l.parent.clone());
        while let Some(parent_layer_fq) = current_layer_parent {
            let Some(parent_layer) = registry.layer(&parent_layer_fq) else {
                break;
            };
            if parent_layer.window_label != lost_window {
                break;
            }

            if let Some(remembered) = &parent_layer.last_focused {
                if remembered != lost_fq {
                    if let Some(scope) = ctx.view.get(remembered) {
                        return FallbackResolution::FallbackParentLayerLastFocused(
                            scope.fq.clone(),
                            scope.fq.last_segment(),
                        );
                    }
                }
            }

            current_layer_parent = parent_layer.parent.clone();
        }

        FallbackResolution::NoFocus
    }

    /// Move focus relative to `from` in `direction`, running pathfinding
    /// against `snapshot`.
    ///
    /// Returns `None` when:
    /// - `from` is not present in `snapshot.scopes`, or
    /// - the resolved target is not present in `snapshot.scopes` (torn
    ///   state), or
    /// - the resolved target is already focused in its window (the
    ///   common "stay put" outcome under the no-silent-dropout
    ///   contract — pathfinding echoed the input FQM), or
    /// - the snapshot's `layer_fq` does not resolve to a registered
    ///   layer.
    pub fn navigate(
        &mut self,
        registry: &mut SpatialRegistry,
        snapshot: &NavSnapshot,
        from: FullyQualifiedMoniker,
        direction: Direction,
        window: Option<WindowLabel>,
    ) -> Option<FocusChangedEvent> {
        let view = IndexedSnapshot::new(snapshot);
        let focused_segment = from.last_segment();
        let target_fq = crate::navigate::pick_target(&view, &from, &focused_segment, direction);
        view.get(&target_fq)?;
        // `from` is the authoritative current focus the caller resolved (pulled
        // from the UI on the host-driven path; supplied on the wire on the
        // inline path). Reconcile the kernel's per-window slot to it BEFORE the
        // commit so the emitted event reports the true `prev_fq` (and the slot
        // stays in sync with the UI-owned focus instead of a stale/empty value).
        if let Some(w) = window.clone() {
            self.focus_by_window.insert(w, from.clone());
        }
        self.focus(registry, snapshot, target_fq, window)
    }

    /// Commit focus to a precomputed `target`, first reconciling the per-window
    /// slot to `from` so the emitted [`FocusChangedEvent`]'s `prev_fq` reflects
    /// the true source (and the slot stays in sync with the UI-authoritative
    /// focus). The drill handlers use this: they compute `target` via
    /// [`crate::navigate::drill_in`] / [`crate::navigate::drill_out`] (pure
    /// compute over the snapshot) and then commit + emit through here — exactly
    /// as [`Self::navigate`] does for directional moves. Returns `None` when
    /// the focus does not change (a drill no-op, `target == from`).
    pub fn focus_from(
        &mut self,
        registry: &mut SpatialRegistry,
        snapshot: &NavSnapshot,
        from: FullyQualifiedMoniker,
        target: FullyQualifiedMoniker,
        window: Option<WindowLabel>,
    ) -> Option<FocusChangedEvent> {
        if let Some(w) = window.clone() {
            self.focus_by_window.insert(w, from);
        }
        self.focus(registry, snapshot, target, window)
    }

    /// Clear focus for `window`.
    ///
    /// Removes the per-window focus slot and returns a
    /// [`FocusChangedEvent`] describing the `Some(prev) → None`
    /// transition so the React side's `focus-changed` projection can
    /// flip the entity-focus store back to `null`. When `window` had no
    /// prior focus, returns `None` (no-op).
    pub fn clear_focus(&mut self, window: &WindowLabel) -> Option<FocusChangedEvent> {
        let prev_fq = self.focus_by_window.remove(window)?;
        Some(FocusChangedEvent {
            window_label: window.clone(),
            prev_fq: Some(prev_fq),
            next_fq: None,
            next_segment: None,
        })
    }

    /// Read the focused [`FullyQualifiedMoniker`] for `window`, if any.
    pub fn focused_in(&self, window: &WindowLabel) -> Option<&FullyQualifiedMoniker> {
        self.focus_by_window.get(window)
    }
}

#[cfg(test)]
mod tests {
    //! Unit-level coverage that lives alongside the implementation.

    use super::*;
    use crate::layer::FocusLayer;
    use crate::snapshot::SnapshotScope;
    use crate::types::{FullyQualifiedMoniker, LayerName, Pixels, Rect, SegmentMoniker};
    use std::collections::HashMap;

    fn rect_zero() -> Rect {
        Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(0.0),
            height: Pixels::new(0.0),
        }
    }

    /// Build a single-layer registry with a layer at `layer` bound to
    /// the given window label.
    fn registry_with_layer(window: &str, layer: &str) -> (SpatialRegistry, FullyQualifiedMoniker) {
        let mut reg = SpatialRegistry::new();
        let layer_fq = FullyQualifiedMoniker::from_string(layer);
        reg.push_layer(FocusLayer {
            fq: layer_fq.clone(),
            segment: SegmentMoniker::from_string("window"),
            name: LayerName::from_string("window"),
            parent: None,
            window_label: WindowLabel::from_string(window),
            last_focused: None,
        });
        (reg, layer_fq)
    }

    /// Build a snapshot for `layer_fq` containing a single scope at `fq`
    /// with the zero rect and no overrides / parent zone.
    fn snapshot_with_scope(layer_fq: FullyQualifiedMoniker, fq: &str) -> NavSnapshot {
        NavSnapshot {
            layer_fq,
            scopes: vec![SnapshotScope {
                fq: FullyQualifiedMoniker::from_string(fq),
                rect: rect_zero(),
                parent_zone: None,
                nav_override: HashMap::new(),
                focusable: true,
            }],
        }
    }

    #[test]
    fn focus_returns_event_with_window_and_segment() {
        let (mut registry, layer_fq) = registry_with_layer("main", "/L");
        let snapshot = snapshot_with_scope(layer_fq, "/L/k1");
        let mut state = SpatialState::new();
        let fq = FullyQualifiedMoniker::from_string("/L/k1");

        let event = state
            .focus(&mut registry, &snapshot, fq.clone(), None)
            .expect("focus emits an event");
        assert_eq!(event.window_label, WindowLabel::from_string("main"));
        assert_eq!(event.prev_fq, None);
        assert_eq!(event.next_fq, Some(fq));
        assert_eq!(event.next_segment, Some(SegmentMoniker::from_string("k1")));
    }

    #[test]
    fn focus_unknown_fq_is_noop() {
        let (mut registry, layer_fq) = registry_with_layer("main", "/L");
        let empty_snapshot = NavSnapshot {
            layer_fq,
            scopes: vec![],
        };
        let mut state = SpatialState::new();
        assert!(state
            .focus(
                &mut registry,
                &empty_snapshot,
                FullyQualifiedMoniker::from_string("/ghost"),
                None,
            )
            .is_none());
    }

    #[test]
    fn focus_same_fq_twice_emits_once() {
        let (mut registry, layer_fq) = registry_with_layer("main", "/L");
        let snapshot = snapshot_with_scope(layer_fq, "/L/k1");
        let mut state = SpatialState::new();
        let fq = FullyQualifiedMoniker::from_string("/L/k1");

        assert!(state
            .focus(&mut registry, &snapshot, fq.clone(), None)
            .is_some());
        assert!(state.focus(&mut registry, &snapshot, fq, None).is_none());
    }

    /// Two windows whose root layers share the FQM `/L` (every window's
    /// real root is `/window`, so the registry — keyed by FQM — holds only
    /// the *last* pushed layer). The second push clobbers the first's
    /// `window_label`, so the layer-derived window is "win-b" even for a
    /// commit that came from "win-a". Passing the authoritative calling
    /// window (`Some("win-a")`) must win: the event and the focus slot land
    /// in "win-a", never "win-b". This is the regression guard for
    /// "navigation moves focus in the wrong window / both windows".
    #[test]
    fn focus_uses_authoritative_window_over_clobbered_layer_label() {
        let (mut registry, layer_fq) = registry_with_layer("win-a", "/L");
        // Window B mounts and pushes the *same* root FQM — clobbering A's
        // layer entry (and its window_label) in the FQM-keyed registry.
        registry.push_layer(FocusLayer {
            fq: layer_fq.clone(),
            segment: SegmentMoniker::from_string("window"),
            name: LayerName::from_string("window"),
            parent: None,
            window_label: WindowLabel::from_string("win-b"),
            last_focused: None,
        });
        assert_eq!(
            registry.layer(&layer_fq).unwrap().window_label,
            WindowLabel::from_string("win-b"),
            "precondition: the registry layer now reports win-b (last push wins)",
        );

        let snapshot = snapshot_with_scope(layer_fq, "/L/k1");
        let mut state = SpatialState::new();
        let fq = FullyQualifiedMoniker::from_string("/L/k1");

        let event = state
            .focus(
                &mut registry,
                &snapshot,
                fq.clone(),
                Some(WindowLabel::from_string("win-a")),
            )
            .expect("focus emits an event");
        assert_eq!(
            event.window_label,
            WindowLabel::from_string("win-a"),
            "the authoritative calling window must win over the clobbered layer label",
        );
        assert_eq!(
            state.focused_in(&WindowLabel::from_string("win-a")),
            Some(&fq),
            "the focus slot must be recorded under the authoritative window",
        );
        assert_eq!(
            state.focused_in(&WindowLabel::from_string("win-b")),
            None,
            "the wrong (clobbered-label) window must not receive the focus",
        );
    }
}
