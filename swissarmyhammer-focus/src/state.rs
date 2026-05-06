//! Per-window focus state.
//!
//! `SpatialState` is the **headless** focus tracker that backs the spatial-
//! nav Tauri commands. It owns exactly one piece of mutable state:
//!
//! - A per-window focus map: `HashMap<WindowLabel, FullyQualifiedMoniker>`.
//!   Every Tauri window has its own focused element; focus moves in
//!   window A do not perturb window B's slot.
//!
//! Everything else — the segment bound to an FQM, the window the FQM
//! lives in, the rect, the layer / scope hierarchy — lives in
//! [`SpatialRegistry`] and is read on demand. There is no per-FQM
//! "entry" map on `SpatialState`: a single source of truth (the
//! registry) eliminates the drift surface that an earlier dual-store
//! design exposed.
//!
//! Mutating methods return [`Option<FocusChangedEvent>`] **instead of**
//! emitting on a Tauri channel directly. This keeps the focus crate
//! testable without a Tauri runtime and pushes the side-effect (`emit`)
//! up to the adapter layer in `kanban-app/src/commands.rs`. Tests in
//! `tests/focus_state.rs` exercise the returned events; the GUI
//! integration is tested via the existing Tauri command-dispatch path.
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

use super::navigate::{NavScopeView, RegistryLayerView};
use super::registry::SpatialRegistry;
use super::scope::FocusScope;
use super::snapshot::IndexedSnapshot;
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
    /// Read from the registry at event-construction time so React
    /// consumers do not need to look it up.
    pub next_segment: Option<SegmentMoniker>,
}

/// Result of a scope-aware focus fallback computation.
///
/// Produced by [`SpatialState::resolve_fallback`] when the focused entry
/// is about to be unregistered. Each "found" variant carries the resolved
/// target's [`FullyQualifiedMoniker`] and [`SegmentMoniker`] — the focus
/// tracker uses them to update `focus_by_window`, and the adapter uses
/// them to emit the outgoing [`FocusChangedEvent`]. Variants carry
/// newtypes throughout; no raw strings on the kernel surface.
///
/// The variant communicates **how** the resolver arrived at the target
/// so consumers (mostly tests, and tracing in the adapter) can reason
/// about the precise rule that applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackResolution {
    /// Target is a sibling of the lost entry in the same parent scope
    /// (rule 1).
    FallbackSiblingInZone(FullyQualifiedMoniker, SegmentMoniker),
    /// Target is the parent scope's `last_focused` slot, still
    /// registered (rule 2 preferred).
    FallbackParentZoneLastFocused(FullyQualifiedMoniker, SegmentMoniker),
    /// Target is the nearest entry in an ancestor scope, used when the
    /// preferred `last_focused` is stale or absent (rule 2 fallback).
    FallbackParentZoneNearest(FullyQualifiedMoniker, SegmentMoniker),
    /// Target is the ancestor layer's `last_focused` slot, still
    /// registered (rule 4 preferred path).
    FallbackParentLayerLastFocused(FullyQualifiedMoniker, SegmentMoniker),
    /// Target is the nearest live scope in an ancestor layer, used when
    /// the layer's `last_focused` is stale or absent (rule 4 fallback).
    /// The candidate set covers every scope in the ancestor layer
    /// regardless of `parent_zone`, since rule 4 is layer-scoped, not
    /// zone-scoped.
    FallbackParentLayerNearest(FullyQualifiedMoniker, SegmentMoniker),
    /// No live fallback target exists in the lost entry's window
    /// (rule 5). The caller clears the window's focus slot.
    NoFocus,
}

/// Carries the lost FQM's metadata that the snapshot path cannot
/// supply on its own.
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
    view: &dyn NavScopeView,
    zone_fq: &Option<FullyQualifiedMoniker>,
    lost_fq: &FullyQualifiedMoniker,
    origin_rect: Rect,
) -> Option<FullyQualifiedMoniker> {
    view.iter()
        .filter(|s| s.fq != lost_fq)
        .filter(|s| match zone_fq {
            Some(zk) => s.parent_zone == Some(zk),
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

/// Pick the nearest live FQM in `view`, regardless of `parent_zone`,
/// excluding `lost_fq` from the candidate set.
///
/// Used by [`SpatialState::resolve_fallback`]'s rule 4 fallback: when an
/// ancestor layer's `last_focused` is stale or absent, any live scope in
/// that layer is a valid landing target. Tie-break ordering matches
/// [`nearest_in_zone_view`] so the cascade reads consistently.
fn nearest_in_layer_view(
    view: &dyn NavScopeView,
    lost_fq: &FullyQualifiedMoniker,
    origin_rect: Rect,
) -> Option<FullyQualifiedMoniker> {
    view.iter()
        .filter(|s| s.fq != lost_fq)
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
    view: &dyn NavScopeView,
    fq: &FullyQualifiedMoniker,
) -> Option<FullyQualifiedMoniker> {
    view.get(fq).and_then(|s| s.parent_zone.cloned())
}

/// Build the `(fq, segment)` pair for a fallback target whose FQM is
/// already known to be live in `registry`. Returns `None` when the FQM
/// disappeared between the view-walk and this lookup — degrades to a
/// continued cascade rather than panicking.
fn resolve_target_segment(
    registry: &SpatialRegistry,
    fq: FullyQualifiedMoniker,
) -> Option<(FullyQualifiedMoniker, SegmentMoniker)> {
    let segment = registry.find_by_fq(&fq)?.segment.clone();
    Some((fq, segment))
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

/// `true` when `scope` belongs to a layer whose `window_label` matches
/// `expected_window`. Used to enforce the per-window barrier on every
/// candidate the fallback resolver returns.
fn same_window(
    registry: &SpatialRegistry,
    scope: &FocusScope,
    expected_window: &WindowLabel,
) -> bool {
    registry
        .layer(&scope.layer_fq)
        .map(|l| &l.window_label == expected_window)
        .unwrap_or(false)
}

/// Shared body of [`SpatialState::resolve_fallback`] and
/// [`SpatialState::resolve_fallback_with_snapshot`]. The in-layer scope
/// walk reads from `view`; layer-tree edges, per-scope `last_focused`
/// slots, and segment lookups still come from `registry`.
fn resolve_fallback_inner(
    registry: &SpatialRegistry,
    view: &dyn NavScopeView,
    lost_fq: &FullyQualifiedMoniker,
    lost_layer: &FullyQualifiedMoniker,
    lost_parent_zone: Option<&FullyQualifiedMoniker>,
    lost_rect: Rect,
) -> FallbackResolution {
    let Some(lost_window) = registry.layer(lost_layer).map(|l| l.window_label.clone()) else {
        return FallbackResolution::NoFocus;
    };

    let mut current_zone: Option<FullyQualifiedMoniker> = lost_parent_zone.cloned();

    // ── Phase 1: scope-tree walk inside the lost layer.
    //
    // At each level, candidates are scopes in `view` whose `parent_zone`
    // matches `current_zone`, with the lost FQM excluded so a stale
    // registry entry can't ghost-block the walk. The first non-empty
    // level wins; siblings are picked by nearest-rect to the lost rect.
    //
    // First iteration is rule 1 (sibling-only on the lost entry's own
    // zone); later iterations are rule 2 (ancestor's `last_focused`
    // first, then nearest fallback).
    let mut is_first_iteration = true;
    loop {
        let on_lost_zone = is_first_iteration;
        if !on_lost_zone {
            if let Some(zone_fq) = &current_zone {
                // Consult the top-level `last_focused_by_fq` map first;
                // fall back to the per-scope `FocusScope::last_focused`
                // mirror. The two stay synchronized via the dual-write
                // in `record_focus`, but the map is the authoritative
                // slot going forward as the per-scope mirror is retired.
                let remembered = registry
                    .last_focused_by_fq
                    .get(zone_fq)
                    .cloned()
                    .or_else(|| {
                        registry
                            .find_by_fq(zone_fq)
                            .and_then(|parent| parent.last_focused.clone())
                    });
                if let Some(remembered) = remembered {
                    if &remembered != lost_fq {
                        if let Some(scope) = registry.find_by_fq(&remembered) {
                            if same_window(registry, scope, &lost_window) {
                                return FallbackResolution::FallbackParentZoneLastFocused(
                                    scope.fq.clone(),
                                    scope.segment.clone(),
                                );
                            }
                        }
                    }
                }
            }
        }

        if let Some(next_fq) = nearest_in_zone_view(view, &current_zone, lost_fq, lost_rect) {
            if let Some((next_fq, next_segment)) = resolve_target_segment(registry, next_fq) {
                return if on_lost_zone {
                    FallbackResolution::FallbackSiblingInZone(next_fq, next_segment)
                } else {
                    FallbackResolution::FallbackParentZoneNearest(next_fq, next_segment)
                };
            }
        }

        // Move up one scope. The parent_zone link comes from `view` so
        // the snapshot path doesn't fall back to the registry. At the
        // layer root (`current_zone == None`) phase 1 is exhausted.
        let Some(zone_fq) = current_zone else {
            break;
        };
        current_zone = parent_zone_of(view, &zone_fq);
        is_first_iteration = false;
    }

    // ── Phase 2: layer-tree walk. Layer edges live on the registry.
    let mut current_layer_parent = registry.layer(lost_layer).and_then(|l| l.parent.clone());
    while let Some(parent_layer_fq) = current_layer_parent {
        let Some(parent_layer) = registry.layer(&parent_layer_fq) else {
            break;
        };
        if parent_layer.window_label != lost_window {
            break;
        }

        if let Some(remembered) = &parent_layer.last_focused {
            if remembered != lost_fq {
                if let Some(scope) = registry.find_by_fq(remembered) {
                    if same_window(registry, scope, &lost_window) {
                        return FallbackResolution::FallbackParentLayerLastFocused(
                            scope.fq.clone(),
                            scope.segment.clone(),
                        );
                    }
                }
            }
        }

        // Phase 2 nearest scans an ancestor layer, not the lost layer,
        // so the snapshot view (scoped to the lost layer) cannot serve
        // it; this branch reads the registry directly.
        let parent_view = RegistryLayerView::new(registry, &parent_layer.fq);
        if let Some(next_fq) = nearest_in_layer_view(&parent_view, lost_fq, lost_rect) {
            if let Some((next_fq, next_segment)) = resolve_target_segment(registry, next_fq) {
                return FallbackResolution::FallbackParentLayerNearest(next_fq, next_segment);
            }
        }

        current_layer_parent = parent_layer.parent.clone();
    }

    FallbackResolution::NoFocus
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

    /// Move focus to `fq`, scoped to the window the registered scope
    /// belongs to.
    ///
    /// The window and segment are derived from `registry`: a scope's
    /// owning window is `registry.layer(scope.layer_fq).window_label`,
    /// and its segment is `scope.segment`. The registry is the single
    /// source of truth — no entry mirror lives on `SpatialState`.
    ///
    /// On a successful focus transition this method also calls
    /// [`SpatialRegistry::record_focus`] so every scope ancestor and
    /// every layer ancestor of the new focus has its `last_focused`
    /// slot updated to `fq`. That walk is what makes the
    /// [`FallbackResolution::FallbackParentZoneLastFocused`] and
    /// [`FallbackResolution::FallbackParentLayerLastFocused`] cascade
    /// arms reachable in production — when the focused scope is later
    /// unregistered, the resolver consults the recorded slots to land
    /// the user back on a meaningful target. This is why `registry` is
    /// borrowed mutably here: focus is the trigger for the writer.
    ///
    /// Returns `None` when:
    /// - `fq` is not registered in `registry` (the caller's
    ///   `<FocusScope>` is racing its own register call), or
    /// - the scope's `layer_fq` does not resolve to a layer (the
    ///   registry is in a torn state — should not happen via the
    ///   adapter, but we degrade silently rather than panic), or
    /// - the resolved FQM is already focused in its window (no-op so
    ///   adapters do not emit redundant `focus-changed` events).
    pub fn focus(
        &mut self,
        registry: &mut SpatialRegistry,
        fq: FullyQualifiedMoniker,
    ) -> Option<FocusChangedEvent> {
        let entry = registry.find_by_fq(&fq)?;
        let layer = registry.layer(&entry.layer_fq)?;
        let window = layer.window_label.clone();
        let segment = entry.segment.clone();

        let prev_fq = self.focus_by_window.get(&window).cloned();
        if prev_fq.as_ref() == Some(&fq) {
            return None;
        }

        self.focus_by_window.insert(window.clone(), fq.clone());
        // Record the new focus on every ancestor scope and every
        // ancestor layer of `fq`. This is the kernel writer for
        // `FocusScope::last_focused`, `last_focused_by_fq`, and
        // `FocusLayer::last_focused`; see
        // `SpatialRegistry::record_focus` for the walk semantics.
        registry.record_focus(&fq, None);
        Some(FocusChangedEvent {
            window_label: window,
            prev_fq,
            next_fq: Some(fq),
            next_segment: Some(segment),
        })
    }

    /// React to a scope being unregistered from the registry, computing
    /// a scope-aware focus fallback.
    ///
    /// Adapters call this **before** `SpatialRegistry::unregister_scope`
    /// so the lost entry's metadata (`layer_fq`, `parent_zone`, owning
    /// window) is still readable. The resolver walks outward through
    /// the scope tree, then up the layer tree, looking for a live
    /// candidate; the search is bounded by the lost entry's
    /// [`WindowLabel`] so fallback never crosses windows. See
    /// [`Self::resolve_fallback`] for the precise rule cascade and the
    /// returned [`FallbackResolution`] variants.
    ///
    /// When `lost_ctx` is `Some`, the in-layer walk reads live scopes
    /// from the supplied snapshot instead of the registry; the lost FQM
    /// is not present in the snapshot, so its `parent_zone` and
    /// `layer_fq` ride alongside in the context.
    ///
    /// If `fq` is the focused slot for some window:
    /// - When the resolution is anything other than
    ///   [`FallbackResolution::NoFocus`], the window's focus slot is
    ///   updated to the resolved FQM and a [`FocusChangedEvent`]
    ///   describing the transition is returned.
    /// - When the resolution is [`FallbackResolution::NoFocus`], the
    ///   window's focus slot is cleared and a `Some → None` event is
    ///   returned so the React claim registry can release the focus
    ///   visual.
    ///
    /// If `fq` is **not** focused in any window, this is a no-op
    /// returning `None` — `unregister_scope` for an unfocused entry has
    /// nothing to do at the focus-state layer.
    ///
    /// On a successful fallback transition, this method also calls
    /// [`SpatialRegistry::record_focus`] on the new FQM so `last_focused`
    /// slots track the recovered focus — the same write hook
    /// [`Self::focus`] runs. This is why `registry` is taken by `&mut`.
    pub fn handle_unregister(
        &mut self,
        registry: &mut SpatialRegistry,
        fq: &FullyQualifiedMoniker,
        lost_ctx: Option<&LostFocusContext<'_>>,
    ) -> Option<FocusChangedEvent> {
        // Owning window is found by walking `focus_by_window` for a value
        // equal to `fq`. O(num_windows), and num_windows is in single
        // digits, so cheaper than maintaining a reverse index. Critically,
        // returning `None` when the FQM is not focused anywhere means the
        // unfocused-unregister path is free of registry / fallback work.
        let window = self
            .focus_by_window
            .iter()
            .find(|(_, focused)| *focused == fq)
            .map(|(w, _)| w.clone())?;

        let resolution = match lost_ctx {
            Some(ctx) => self.resolve_fallback_with_snapshot(registry, fq, ctx),
            None => self.resolve_fallback(registry, fq),
        };
        match resolution {
            FallbackResolution::NoFocus => {
                self.focus_by_window.remove(&window);
                Some(FocusChangedEvent {
                    window_label: window,
                    prev_fq: Some(fq.clone()),
                    next_fq: None,
                    next_segment: None,
                })
            }
            FallbackResolution::FallbackSiblingInZone(next_fq, next_segment)
            | FallbackResolution::FallbackParentZoneLastFocused(next_fq, next_segment)
            | FallbackResolution::FallbackParentZoneNearest(next_fq, next_segment)
            | FallbackResolution::FallbackParentLayerLastFocused(next_fq, next_segment)
            | FallbackResolution::FallbackParentLayerNearest(next_fq, next_segment) => {
                self.focus_by_window.insert(window.clone(), next_fq.clone());
                // Mirror `Self::focus`: any code path that mutates
                // `focus_by_window` to a new FQM also records the new
                // focus on the registry so the `last_focused` slots
                // stay in sync. The fallback target's ancestors get
                // the recorded path; the lost entry's slot is moot
                // (the caller unregisters it next).
                registry.record_focus(&next_fq, None);
                Some(FocusChangedEvent {
                    window_label: window,
                    prev_fq: Some(fq.clone()),
                    next_fq: Some(next_fq),
                    next_segment: Some(next_segment),
                })
            }
        }
    }

    /// Compute the scope-aware focus fallback for `lost_fq` against the
    /// live registry.
    ///
    /// Pure registry query — does not mutate any focus state. The lost
    /// entry **must still be registered** so the resolver can read its
    /// `parent_zone`, `layer_fq`, and owning window. Adapters call this
    /// before calling [`SpatialRegistry::unregister_scope`].
    ///
    /// The resolution walks outward through the scope tree, then up the
    /// layer tree, in priority order (see `FallbackResolution` for the
    /// rule cascade).
    ///
    /// Fallback is **bounded by `WindowLabel`**: the layer-tree walk
    /// stops if it would cross into a different window. Layers in a
    /// well-formed forest share their root's `window_label`, but the
    /// resolver re-reads each visited layer's window to enforce the
    /// barrier defensively.
    ///
    /// Returns [`FallbackResolution::NoFocus`] when `lost_fq` is not
    /// registered (the caller already unregistered it, or it never
    /// existed) — there is no metadata to start the walk from, so
    /// fallback cannot meaningfully resolve.
    pub fn resolve_fallback(
        &self,
        registry: &SpatialRegistry,
        lost_fq: &FullyQualifiedMoniker,
    ) -> FallbackResolution {
        let (lost_layer, lost_parent_zone, lost_rect) = {
            let Some(lost) = registry.find_by_fq(lost_fq) else {
                return FallbackResolution::NoFocus;
            };
            (lost.layer_fq.clone(), lost.parent_zone.clone(), lost.rect)
        };

        let view = RegistryLayerView::new(registry, &lost_layer);
        resolve_fallback_inner(
            registry,
            &view,
            lost_fq,
            &lost_layer,
            lost_parent_zone.as_ref(),
            lost_rect,
        )
    }

    /// Compute the fallback for a lost FQM using a
    /// [`crate::snapshot::NavSnapshot`] for the live-scope walk. The
    /// lost FQM is not present in the snapshot (already unregistered on
    /// the React side), so its `parent_zone`, `layer_fq`, and `rect`
    /// ride alongside in [`LostFocusContext`].
    ///
    /// Layer-tree edges and per-scope `last_focused` slots continue to
    /// come from `registry`; only the in-layer scope walk reads from
    /// the snapshot.
    pub fn resolve_fallback_with_snapshot(
        &self,
        registry: &SpatialRegistry,
        lost_fq: &FullyQualifiedMoniker,
        ctx: &LostFocusContext<'_>,
    ) -> FallbackResolution {
        resolve_fallback_inner(
            registry,
            ctx.view,
            lost_fq,
            &ctx.lost_layer_fq,
            ctx.lost_parent_zone.as_ref(),
            ctx.lost_rect,
        )
    }

    /// Move focus relative to `from` in `direction`, delegating the
    /// "where do we go next?" decision to a pluggable [`NavStrategy`].
    ///
    /// The strategy is consulted with the supplied [`SpatialRegistry`]
    /// (geometry / hierarchy backing store), the focused
    /// [`FullyQualifiedMoniker`], and the focused entry's
    /// [`SegmentMoniker`] (read from the registry by `from`). The
    /// strategy always returns an FQM (never `None` — see the
    /// no-silent-dropout contract on [`crate::navigate`]). When that
    /// FQM resolves to a scope distinct from `from`, this method emits
    /// a [`FocusChangedEvent`] in the same shape [`Self::focus`] would.
    /// When it resolves back to `from` (semantic "stay put") or fails
    /// to resolve at all, this method returns `None` so the adapter
    /// does not emit a redundant focus-changed event.
    ///
    /// Returns `None` when:
    /// - `from` is not registered in `registry`, or
    /// - the strategy returns an FQM for which no scope is registered
    ///   (torn state), or
    /// - the resolved FQM is already focused in its window (the
    ///   common "stay put" outcome under the no-silent-dropout
    ///   contract — the strategy echoed the focused FQM).
    ///
    /// This is the seam used by [`crate::navigate::BeamNavStrategy`] —
    /// adapters that want the default Android-beam-search behavior pass
    /// `&BeamNavStrategy::new()`; tests and specialised layouts can
    /// pass a custom impl.
    ///
    /// [`NavStrategy`]: crate::navigate::NavStrategy
    pub fn navigate_with(
        &mut self,
        registry: &mut SpatialRegistry,
        strategy: &dyn crate::navigate::NavStrategy,
        from: FullyQualifiedMoniker,
        direction: Direction,
    ) -> Option<FocusChangedEvent> {
        // Validate the starting point belongs to the registry. A
        // strategy invocation on an unknown FQM would otherwise stamp
        // a focus event into a window that has no record of the move.
        // The strategy itself also handles unknown FQMs (echoes the
        // input FQM with a tracing::error!), but at the
        // `navigate_with` boundary we read the focused segment from
        // the registry, which requires a real entry.
        let focused_segment = registry.find_by_fq(&from)?.segment.clone();

        let target_fq = strategy.next(registry, &from, &focused_segment, direction);
        // The strategy speaks in FQMs already — they ARE the registry
        // keys. Look up the target directly.
        if !registry.is_registered(&target_fq) {
            return None;
        }
        // `focus` short-circuits when the resolved FQM already holds
        // focus — that is the common "stay put" outcome under the new
        // contract (the strategy returned the focused FQM). No
        // additional check is required here.
        self.focus(registry, target_fq)
    }

    /// Clear focus for `window`.
    ///
    /// Removes the per-window focus slot and returns a
    /// [`FocusChangedEvent`] describing the `Some(prev) → None`
    /// transition so the React side's `focus-changed` projection can
    /// flip the entity-focus store back to `null`. When `window` had no
    /// prior focus, returns `None` (no-op — adapters do not need to
    /// emit a redundant event).
    ///
    /// This is the explicit-clear counterpart of [`Self::focus`].
    /// It exists so the React-side `setFocus(null)` path can dispatch
    /// through the kernel and let the bridge handle the store write —
    /// keeping the "store is a pure projection" invariant.
    ///
    /// Related: [`Self::handle_unregister`] also produces a
    /// `Some(prev) → None` event when its fallback resolution is
    /// [`FallbackResolution::NoFocus`]. The shape is the same; the
    /// difference is the trigger — `handle_unregister` runs on
    /// scope-deregistration, `clear_focus` runs on an explicit
    /// React-side request.
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
    //!
    //! The richer integration coverage in `tests/focus_state.rs` exercises
    //! the same surface from the public API; these tests catch regressions
    //! at compile time of the inner crate, before the integration-test
    //! binary links.

    use super::*;
    use crate::layer::FocusLayer;
    use crate::scope::FocusScope;
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

    /// Build a single-layer registry with one focus scope bound to
    /// `(window, segment)` at `fq`.
    fn registry_with_scope(window: &str, layer: &str, fq: &str, segment: &str) -> SpatialRegistry {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(FocusLayer {
            fq: FullyQualifiedMoniker::from_string(layer),
            segment: SegmentMoniker::from_string("window"),
            name: LayerName::from_string("window"),
            parent: None,
            window_label: WindowLabel::from_string(window),
            last_focused: None,
        });
        reg.register_scope(FocusScope {
            fq: FullyQualifiedMoniker::from_string(fq),
            segment: SegmentMoniker::from_string(segment),
            rect: rect_zero(),
            layer_fq: FullyQualifiedMoniker::from_string(layer),
            parent_zone: None,
            last_focused: None,
            overrides: HashMap::new(),
        });
        reg
    }

    #[test]
    fn focus_returns_event_with_window_and_segment() {
        let mut registry = registry_with_scope("main", "/L", "/L/k1", "task:01");
        let mut state = SpatialState::new();
        let fq = FullyQualifiedMoniker::from_string("/L/k1");

        let event = state
            .focus(&mut registry, fq.clone())
            .expect("focus emits an event");
        assert_eq!(event.window_label, WindowLabel::from_string("main"));
        assert_eq!(event.prev_fq, None);
        assert_eq!(event.next_fq, Some(fq));
        assert_eq!(
            event.next_segment,
            Some(SegmentMoniker::from_string("task:01"))
        );
    }

    #[test]
    fn focus_unknown_fq_is_noop() {
        let mut registry = SpatialRegistry::new();
        let mut state = SpatialState::new();
        assert!(state
            .focus(&mut registry, FullyQualifiedMoniker::from_string("/ghost"))
            .is_none());
    }

    #[test]
    fn focus_same_fq_twice_emits_once() {
        let mut registry = registry_with_scope("main", "/L", "/L/k1", "task:01");
        let mut state = SpatialState::new();
        let fq = FullyQualifiedMoniker::from_string("/L/k1");

        assert!(state.focus(&mut registry, fq.clone()).is_some());
        assert!(state.focus(&mut registry, fq).is_none());
    }
}
