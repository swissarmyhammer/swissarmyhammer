//! Per-window focus state.
//!
//! `SpatialState` is the **headless** focus tracker that backs the spatial-
//! nav Tauri commands. It owns exactly one piece of mutable state:
//!
//! - A per-window focus map: `HashMap<WindowLabel, SpatialKey>`. Every
//!   Tauri window has its own focused element; focus moves in window A do
//!   not perturb window B's slot.
//!
//! Everything else — the moniker bound to a key, the window the key
//! lives in, the rect, the layer / zone hierarchy — lives in
//! [`SpatialRegistry`] and is read on demand. There is no per-key
//! "entry" map on `SpatialState`: a single source of truth (the
//! registry) eliminates the drift surface that an earlier dual-store
//! design exposed (see review note "Duplicate (SpatialKey, Moniker)
//! pair").
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

use super::registry::SpatialRegistry;
use super::scope::RegisteredScope;
use super::types::{
    pixels_cmp, Direction, LayerKey, Moniker, Pixels, Rect, SpatialKey, WindowLabel,
};

/// Payload emitted to React whenever the focused [`SpatialKey`] for a
/// window changes.
///
/// The frontend's claim registry (`Map<SpatialKey, (focused) => void>`)
/// dispatches `false` to `prev_key` and `true` to `next_key`, so the wire
/// shape is exactly what one cell on either side of a focus move needs to
/// re-render. `next_moniker` is included so consumers that key off the
/// entity identity (rather than the spatial key) can update without an
/// extra IPC round-trip.
///
/// `prev_key` is `None` when the window had no prior focus (cold-start, or
/// the previously focused scope was just unregistered). `next_key` is
/// `None` when focus is being cleared (e.g. the focused scope unmounted
/// and there is no obvious replacement). Both fields independent — focus
/// transfer (`Some(prev) → Some(next)`), focus acquisition
/// (`None → Some(next)`), and focus clear (`Some(prev) → None`) all flow
/// through the same payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusChangedEvent {
    /// Window in which the focus changed. Mirrors the `WindowLabel` the
    /// originating Tauri command derived from its `tauri::Window`
    /// parameter, so the frontend's per-window claim registry can ignore
    /// events for other windows.
    pub window_label: WindowLabel,
    /// Previously focused [`SpatialKey`] in this window, if any.
    pub prev_key: Option<SpatialKey>,
    /// Newly focused [`SpatialKey`] in this window, if any.
    pub next_key: Option<SpatialKey>,
    /// Moniker of the newly focused entity, if `next_key.is_some()`. Read
    /// from the registry at event-construction time so React consumers do
    /// not need to look it up.
    pub next_moniker: Option<Moniker>,
}

/// Result of a zone-aware focus fallback computation.
///
/// Produced by [`SpatialState::resolve_fallback`] when the focused entry
/// is about to be unregistered. Each "found" variant carries the resolved
/// target's [`SpatialKey`] and [`Moniker`] — the focus tracker uses them
/// to update `focus_by_window`, and the adapter uses them to emit the
/// outgoing [`FocusChangedEvent`]. Variant carries newtypes throughout;
/// no raw strings on the kernel surface.
///
/// The variant communicates **how** the resolver arrived at the target
/// so consumers (mostly tests, and tracing in the adapter) can reason
/// about the precise rule that applied. The five "found" variants
/// correspond 1:1 to the cascade documented on
/// [`SpatialState::resolve_fallback`]:
///
/// - [`FallbackResolution::FallbackSiblingInZone`] — rule 1.
/// - [`FallbackResolution::FallbackParentZoneLastFocused`] — rule 2
///   preferred path.
/// - [`FallbackResolution::FallbackParentZoneNearest`] — rule 2
///   fallback when `last_focused` is stale or absent.
/// - [`FallbackResolution::FallbackParentLayerLastFocused`] — rule 4
///   preferred path: the ancestor layer's `last_focused` is still
///   registered.
/// - [`FallbackResolution::FallbackParentLayerNearest`] — rule 4
///   fallback: the layer's `last_focused` is stale or absent and the
///   resolver picked the nearest live scope in that layer.
/// - [`FallbackResolution::NoFocus`] — rule 5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackResolution {
    /// Target is a sibling of the lost entry in the same zone (rule 1).
    FallbackSiblingInZone(SpatialKey, Moniker),
    /// Target is the parent zone's `last_focused` slot, still
    /// registered (rule 2 preferred).
    FallbackParentZoneLastFocused(SpatialKey, Moniker),
    /// Target is the nearest entry in an ancestor zone, used when the
    /// preferred `last_focused` is stale or absent (rule 2 fallback).
    FallbackParentZoneNearest(SpatialKey, Moniker),
    /// Target is the ancestor layer's `last_focused` slot, still
    /// registered (rule 4 preferred path).
    FallbackParentLayerLastFocused(SpatialKey, Moniker),
    /// Target is the nearest live scope in an ancestor layer, used when
    /// the layer's `last_focused` is stale or absent (rule 4 fallback).
    /// The candidate set covers every scope in the ancestor layer
    /// regardless of `parent_zone`, since rule 4 is layer-scoped, not
    /// zone-scoped.
    FallbackParentLayerNearest(SpatialKey, Moniker),
    /// No live fallback target exists in the lost entry's window
    /// (rule 5). The caller clears the window's focus slot.
    NoFocus,
}

/// Pick the nearest entry in `layer_key` whose `parent_zone == zone_key`,
/// excluding `lost_key` from the candidate set.
///
/// "Nearest" is measured by Euclidean-square distance between rect
/// origins; ties break by `top` then `left` so the choice is
/// deterministic on identical-position rects (a common pattern in tests
/// and in placeholder grids). Returns `None` when no candidate exists.
///
/// When `prefer_variant` is `Some`, candidates of that variant
/// (Leaf or Zone) are preferred over the other; the cheapest available
/// match wins within the preferred variant before any candidate of the
/// other variant is considered. This implements the spec's "prefer
/// matching variant" hint — losing a leaf prefers a leaf as the
/// fallback target, losing a zone prefers a zone. When `None`, all
/// candidates are ranked together.
fn nearest_in_zone(
    registry: &SpatialRegistry,
    layer_key: &LayerKey,
    zone_key: &Option<SpatialKey>,
    lost_key: &SpatialKey,
    origin_rect: Rect,
    prefer_variant: Option<ScopeVariant>,
) -> Option<(SpatialKey, Moniker)> {
    let candidates: Vec<&RegisteredScope> = registry
        .entries_in_layer(layer_key)
        .filter(|s| s.key() != lost_key)
        .filter(|s| match zone_key {
            Some(zk) => s.parent_zone() == Some(zk),
            None => s.parent_zone().is_none(),
        })
        .collect();

    if let Some(preferred) = prefer_variant {
        let matches_preferred = |s: &&RegisteredScope| match preferred {
            ScopeVariant::Scope => s.is_scope(),
            ScopeVariant::Zone => s.is_zone(),
        };
        if let Some(best) = candidates
            .iter()
            .copied()
            .filter(matches_preferred)
            .min_by(|a, b| {
                let da = squared_distance(origin_rect, *a.rect());
                let db = squared_distance(origin_rect, *b.rect());
                pixels_cmp(da, db)
                    .then(pixels_cmp(a.rect().top(), b.rect().top()))
                    .then(pixels_cmp(a.rect().left(), b.rect().left()))
            })
        {
            return Some((best.key().clone(), best.moniker().clone()));
        }
    }

    candidates
        .into_iter()
        .min_by(|a, b| {
            let da = squared_distance(origin_rect, *a.rect());
            let db = squared_distance(origin_rect, *b.rect());
            pixels_cmp(da, db)
                .then(pixels_cmp(a.rect().top(), b.rect().top()))
                .then(pixels_cmp(a.rect().left(), b.rect().left()))
        })
        .map(|s| (s.key().clone(), s.moniker().clone()))
}

/// Pick the nearest entry in `layer_key`, regardless of `parent_zone`,
/// excluding `lost_key` from the candidate set.
///
/// Used by [`SpatialState::resolve_fallback`]'s rule 4 fallback: when a
/// layer's `last_focused` is stale or absent, we still want to land on
/// any live scope in the ancestor layer — not just bare leaves at the
/// layer root. "Nearest" uses the same Euclidean-square distance and
/// tie-break ordering as [`nearest_in_zone`] so the cascade reads
/// consistently. Variant preference is intentionally not exposed here
/// because rule 4 does not apply it (see the spec on
/// [`SpatialState::resolve_fallback`]).
fn nearest_in_layer(
    registry: &SpatialRegistry,
    layer_key: &LayerKey,
    lost_key: &SpatialKey,
    origin_rect: Rect,
) -> Option<(SpatialKey, Moniker)> {
    registry
        .entries_in_layer(layer_key)
        .filter(|s| s.key() != lost_key)
        .min_by(|a, b| {
            let da = squared_distance(origin_rect, *a.rect());
            let db = squared_distance(origin_rect, *b.rect());
            pixels_cmp(da, db)
                .then(pixels_cmp(a.rect().top(), b.rect().top()))
                .then(pixels_cmp(a.rect().left(), b.rect().left()))
        })
        .map(|s| (s.key().clone(), s.moniker().clone()))
}

/// Variant tag for [`nearest_in_zone`]'s "prefer matching variant" knob.
///
/// Tag-only counterpart of the internal [`RegisteredScope`] variants so
/// the resolver can carry "what variant did we lose?" through nested
/// borrows without holding a reference into the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScopeVariant {
    /// A leaf (`RegisteredScope::Scope`).
    Scope,
    /// A zone (`RegisteredScope::Zone`).
    Zone,
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
    scope: &RegisteredScope,
    expected_window: &WindowLabel,
) -> bool {
    registry
        .layer(scope.layer_key())
        .map(|l| &l.window_label == expected_window)
        .unwrap_or(false)
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
    /// The currently focused [`SpatialKey`] **per window**. Looking up a
    /// `WindowLabel` that does not appear here yields no focus for that
    /// window — distinct from "focus is the same key in two windows",
    /// which is impossible because each window owns its own slot.
    focus_by_window: HashMap<WindowLabel, SpatialKey>,
}

impl SpatialState {
    /// Construct an empty `SpatialState`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Move focus to `key`, scoped to the window the registered scope
    /// belongs to.
    ///
    /// The window and moniker are derived from `registry`: a scope's
    /// owning window is `registry.layer(scope.layer_key()).window_label`,
    /// and its moniker is `scope.moniker()`. The registry is the single
    /// source of truth — no entry mirror lives on `SpatialState`.
    ///
    /// Returns `None` when:
    /// - `key` is not registered in `registry` (the caller's
    ///   `<FocusScope>` is racing its own register call), or
    /// - the scope's `layer_key` does not resolve to a layer (the
    ///   registry is in a torn state — should not happen via the
    ///   adapter, but we degrade silently rather than panic), or
    /// - the resolved key is already focused in its window (no-op so
    ///   adapters do not emit redundant `focus-changed` events).
    pub fn focus(
        &mut self,
        registry: &SpatialRegistry,
        key: SpatialKey,
    ) -> Option<FocusChangedEvent> {
        let entry = registry.entry(&key)?;
        let layer = registry.layer(entry.layer_key())?;
        let window = layer.window_label.clone();
        let moniker = entry.moniker().clone();

        let prev_key = self.focus_by_window.get(&window).cloned();
        if prev_key.as_ref() == Some(&key) {
            return None;
        }

        self.focus_by_window.insert(window.clone(), key.clone());
        Some(FocusChangedEvent {
            window_label: window,
            prev_key,
            next_key: Some(key),
            next_moniker: Some(moniker),
        })
    }

    /// Move focus to the scope identified by `moniker`, resolving the
    /// `(SpatialKey, Moniker)` pair against `registry`.
    ///
    /// This is the moniker-keyed counterpart of [`Self::focus`]. The
    /// React side owns moniker identity (`"task:01ABC"`,
    /// `"field:task:01ABC.title"`); the kernel owns spatial-key
    /// identity (ULIDs minted per mount). When the React side wants to
    /// move focus by moniker — e.g. `setFocus("field:task:01ABC.title")`
    /// after the inspector mounts — the kernel resolves the moniker
    /// once, advances `focus_by_window`, and emits the resulting
    /// [`FocusChangedEvent`].
    ///
    /// Mirrors the no-silent-dropout contract elsewhere in the kernel:
    /// when the moniker is unknown, this method emits
    /// `tracing::error!` and returns `None`. The adapter forwards the
    /// `None` to the React caller as an `Err(_)` so the React side's
    /// `setFocus` dispatch can `console.error` for dev visibility.
    /// "Already focused" returns `None` for the same reason
    /// [`Self::focus`] does — adapters need not emit redundant
    /// `focus-changed` events.
    ///
    /// Returns `None` when:
    /// - no registered scope has the given moniker (kernel logs
    ///   `tracing::error!` for the unknown-moniker case), or
    /// - the resolved scope's layer is missing (torn registry —
    ///   should not happen via the adapter, but we degrade silently
    ///   rather than panic), or
    /// - the resolved key is already focused in its window (no-op so
    ///   adapters do not emit redundant `focus-changed` events).
    pub fn focus_by_moniker(
        &mut self,
        registry: &SpatialRegistry,
        moniker: &Moniker,
    ) -> Option<FocusChangedEvent> {
        let Some(key) = registry.find_by_moniker(moniker).cloned() else {
            // Unknown moniker — under the no-silent-dropout contract
            // the kernel surfaces a tracing error so the regression is
            // observable in logs. The React adapter forwards the
            // adapter-level `Err(_)` to a console.error for dev mode.
            tracing::error!(
                op = "focus_by_moniker",
                moniker = %moniker,
                "unknown moniker passed to SpatialState::focus_by_moniker"
            );
            return None;
        };
        self.focus(registry, key)
    }

    /// React to a scope being unregistered from the registry, computing
    /// a zone-aware focus fallback.
    ///
    /// Adapters call this **before** `SpatialRegistry::unregister_scope`
    /// so the lost entry's metadata (`layer_key`, `parent_zone`, owning
    /// window) is still readable. The resolver walks outward through
    /// the zone tree, then up the layer tree, looking for a live
    /// candidate; the search is bounded by the lost entry's
    /// [`WindowLabel`] so fallback never crosses windows. See
    /// [`Self::resolve_fallback`] for the precise rule cascade and the
    /// returned [`FallbackResolution`] variants.
    ///
    /// If `key` is the focused slot for some window:
    /// - When the resolution is anything other than
    ///   [`FallbackResolution::NoFocus`], the window's focus slot is
    ///   updated to the resolved key and a [`FocusChangedEvent`]
    ///   describing the transition is returned.
    /// - When the resolution is [`FallbackResolution::NoFocus`], the
    ///   window's focus slot is cleared and a `Some → None` event is
    ///   returned so the React claim registry can release the focus
    ///   visual.
    ///
    /// If `key` is **not** focused in any window, this is a no-op
    /// returning `None` — `unregister_scope` for an unfocused entry has
    /// nothing to do at the focus-state layer.
    pub fn handle_unregister(
        &mut self,
        registry: &SpatialRegistry,
        key: &SpatialKey,
    ) -> Option<FocusChangedEvent> {
        // Owning window is found by walking `focus_by_window` for a value
        // equal to `key`. O(num_windows), and num_windows is in single
        // digits, so cheaper than maintaining a reverse index. Critically,
        // returning `None` when the key is not focused anywhere means the
        // unfocused-unregister path is free of registry / fallback work.
        let window = self
            .focus_by_window
            .iter()
            .find(|(_, focused)| *focused == key)
            .map(|(w, _)| w.clone())?;

        let resolution = self.resolve_fallback(registry, key);
        match resolution {
            FallbackResolution::NoFocus => {
                self.focus_by_window.remove(&window);
                Some(FocusChangedEvent {
                    window_label: window,
                    prev_key: Some(key.clone()),
                    next_key: None,
                    next_moniker: None,
                })
            }
            FallbackResolution::FallbackSiblingInZone(next_key, next_moniker)
            | FallbackResolution::FallbackParentZoneLastFocused(next_key, next_moniker)
            | FallbackResolution::FallbackParentZoneNearest(next_key, next_moniker)
            | FallbackResolution::FallbackParentLayerLastFocused(next_key, next_moniker)
            | FallbackResolution::FallbackParentLayerNearest(next_key, next_moniker) => {
                self.focus_by_window
                    .insert(window.clone(), next_key.clone());
                Some(FocusChangedEvent {
                    window_label: window,
                    prev_key: Some(key.clone()),
                    next_key: Some(next_key),
                    next_moniker: Some(next_moniker),
                })
            }
        }
    }

    /// Compute the zone-aware focus fallback for `lost_key`.
    ///
    /// Pure registry query — does not mutate any focus state. The lost
    /// entry **must still be registered** so the resolver can read its
    /// `parent_zone`, `layer_key`, and owning window. Adapters call this
    /// before calling [`SpatialRegistry::unregister_scope`].
    ///
    /// The resolution walks outward through the zone tree, then up the
    /// layer tree, in priority order:
    ///
    /// 1. **Sibling in same zone** — the nearest live entry whose
    ///    `parent_zone` matches the lost entry's `parent_zone` in the
    ///    same layer. "Lost" candidates (the entry itself) are
    ///    excluded. The variant of the lost entry (`Focusable` /
    ///    `Zone`) is preferred — losing a leaf prefers a sibling leaf,
    ///    losing a zone prefers a sibling zone — but only at this rule.
    ///    Returns [`FallbackResolution::FallbackSiblingInZone`].
    /// 2. **Walk up parent zones** — at each ancestor zone, prefer the
    ///    zone's `last_focused` if it still resolves to a live scope;
    ///    otherwise pick the nearest entry inside that zone (excluding
    ///    the lost key). Variant preference does **not** apply at this
    ///    rule — the nearest live candidate wins regardless of variant.
    ///    Returns
    ///    [`FallbackResolution::FallbackParentZoneLastFocused`] or
    ///    [`FallbackResolution::FallbackParentZoneNearest`].
    /// 3. **Walk up to layer root** — the walk continues until a zone
    ///    has any live candidate or the layer root is reached.
    /// 4. **Walk up the layer tree** — when the layer root has no
    ///    remaining entries, walk `layer.parent`. At each ancestor
    ///    layer, prefer the layer's `last_focused` if it is still
    ///    registered (returns
    ///    [`FallbackResolution::FallbackParentLayerLastFocused`]);
    ///    otherwise pick the nearest entry **anywhere** in that layer
    ///    (any `parent_zone`, including zone-nested leaves) and return
    ///    [`FallbackResolution::FallbackParentLayerNearest`].
    /// 5. **No-focus** — when the walk exhausts the layer chain without
    ///    finding a live candidate, returns
    ///    [`FallbackResolution::NoFocus`].
    ///
    /// Fallback is **bounded by `WindowLabel`**: the layer-tree walk
    /// stops if it would cross into a different window. Layers in a
    /// well-formed forest share their root's `window_label`, but the
    /// resolver re-reads each visited layer's window to enforce the
    /// barrier defensively.
    ///
    /// Returns [`FallbackResolution::NoFocus`] when `lost_key` is not
    /// registered (the caller already unregistered it, or it never
    /// existed) — there is no metadata to start the walk from, so
    /// fallback cannot meaningfully resolve.
    pub fn resolve_fallback(
        &self,
        registry: &SpatialRegistry,
        lost_key: &SpatialKey,
    ) -> FallbackResolution {
        // Snapshot the lost entry's metadata into owned values so the
        // immutable borrow can be released before we walk the registry
        // (the walk does its own short-lived borrows and would otherwise
        // collide with this one in some borrow-checker paths).
        //
        // If the lost key is already gone, we have no metadata to drive
        // the walk; degrade to NoFocus. Adapters that want a meaningful
        // fallback must call this before `unregister_scope`.
        let (lost_layer, lost_parent_zone, lost_rect, lost_variant) = {
            let Some(lost) = registry.entry(lost_key) else {
                return FallbackResolution::NoFocus;
            };
            (
                lost.layer_key().clone(),
                lost.parent_zone().cloned(),
                *lost.rect(),
                if lost.is_zone() {
                    ScopeVariant::Zone
                } else {
                    ScopeVariant::Scope
                },
            )
        };

        let Some(lost_window) = registry.layer(&lost_layer).map(|l| l.window_label.clone()) else {
            // Layer missing — registry is in a torn state. Degrade to
            // no-focus rather than panic.
            return FallbackResolution::NoFocus;
        };

        let mut current_zone = lost_parent_zone.clone();

        // ── Phase 1: zone-tree walk inside the lost layer.
        //
        // At each level, candidates are scopes in the current layer
        // whose `parent_zone == current_zone`. The lost key itself is
        // excluded so a stale registration doesn't ghost-block the
        // walk. The first non-empty level wins; siblings are picked
        // by nearest-rect to the lost rect.
        //
        // The first iteration of the loop is rule 1 — `current_zone`
        // is the lost entry's *own* `parent_zone`, so we look for a
        // direct sibling and apply the variant preference. Subsequent
        // iterations are rule 2 — we have walked up to an ancestor
        // zone, so the ancestor's `last_focused` is consulted first
        // and variant preference is dropped.
        let mut is_first_iteration = true;
        loop {
            let on_lost_zone = is_first_iteration;
            // Rule 2's "preferred" step: consult the *ancestor* zone's
            // `last_focused`. Only applies on iterations 2+ (the lost
            // entry's own zone is rule 1, sibling-only). At the layer
            // root (`current_zone is None`) there is no enclosing zone
            // to consult, so this step is also skipped.
            if !on_lost_zone {
                if let Some(zone_key) = &current_zone {
                    if let Some(zone) = registry.zone(zone_key) {
                        if let Some(remembered) = &zone.last_focused {
                            if remembered != lost_key {
                                if let Some(scope) = registry.entry(remembered) {
                                    if same_window(registry, scope, &lost_window) {
                                        return FallbackResolution::FallbackParentZoneLastFocused(
                                            scope.key().clone(),
                                            scope.moniker().clone(),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Pick the nearest live sibling in this zone (or, when
            // current_zone is None, the nearest scope directly under the
            // layer root). On the first iteration only, prefer the variant
            // matching the lost entry so a deleted leaf resolves to a leaf
            // and a deleted zone resolves to a zone when both are
            // available siblings. On rule 2 (ancestor zones) the spec
            // calls for "the nearest entry" without any variant filter.
            let prefer = if on_lost_zone {
                Some(lost_variant)
            } else {
                None
            };
            if let Some((key, moniker)) = nearest_in_zone(
                registry,
                &lost_layer,
                &current_zone,
                lost_key,
                lost_rect,
                prefer,
            ) {
                // First iteration: same zone as the lost entry → rule 1.
                // Subsequent iterations: zone above the (now empty) one
                // the lost entry was in → rule 2 nearest.
                let resolution = if on_lost_zone {
                    FallbackResolution::FallbackSiblingInZone(key, moniker)
                } else {
                    FallbackResolution::FallbackParentZoneNearest(key, moniker)
                };
                return resolution;
            }

            // Move up one zone. If we were already at the layer root
            // (`current_zone == None`), exit the zone-tree phase.
            let Some(zone_key) = current_zone else {
                break;
            };
            let parent = registry.zone(&zone_key).and_then(|z| z.parent_zone.clone());
            current_zone = parent;
            is_first_iteration = false;
        }

        // ── Phase 2: layer-tree walk.
        //
        // The lost entry's layer has no remaining live scopes (any zone
        // walk would have returned). Walk `layer.parent`, bounded by the
        // window: a layer whose `window_label` differs from the lost
        // entry's window is an invariant violation, but we treat it as
        // a barrier and stop rather than crossing.
        let mut current_layer_parent = registry.layer(&lost_layer).and_then(|l| l.parent.clone());
        while let Some(parent_layer_key) = current_layer_parent {
            let Some(parent_layer) = registry.layer(&parent_layer_key) else {
                break;
            };
            if parent_layer.window_label != lost_window {
                // Crossing windows would violate the per-window invariant;
                // bail rather than return a foreign target.
                break;
            }

            // Prefer the layer's `last_focused` if still registered.
            if let Some(remembered) = &parent_layer.last_focused {
                if remembered != lost_key {
                    if let Some(scope) = registry.entry(remembered) {
                        if same_window(registry, scope, &lost_window) {
                            return FallbackResolution::FallbackParentLayerLastFocused(
                                scope.key().clone(),
                                scope.moniker().clone(),
                            );
                        }
                    }
                }
            }

            // Otherwise pick the nearest live scope **anywhere** in the
            // ancestor layer. Rule 4 is layer-scoped, not zone-scoped:
            // a leaf nested inside an ancestor zone is just as valid a
            // fallback target as a leaf hanging directly under the
            // layer root, so the candidate set ignores `parent_zone`
            // entirely. Variant preference does not apply at this rule.
            if let Some((key, moniker)) =
                nearest_in_layer(registry, &parent_layer.key, lost_key, lost_rect)
            {
                return FallbackResolution::FallbackParentLayerNearest(key, moniker);
            }

            current_layer_parent = parent_layer.parent.clone();
        }

        // ── Phase 3: no-focus.
        //
        // Walked the entire layer chain bounded by the lost window
        // without finding a live candidate. The window's focus slot
        // will be cleared by the caller.
        FallbackResolution::NoFocus
    }

    /// Move focus relative to `from` in `direction`, delegating the
    /// "where do we go next?" decision to a pluggable [`NavStrategy`].
    ///
    /// The strategy is consulted with the supplied [`SpatialRegistry`]
    /// (geometry / hierarchy backing store), the focused
    /// [`SpatialKey`], and the focused entry's [`Moniker`] (read from
    /// the registry by `from`). The strategy always returns a
    /// [`Moniker`] (never `None` — see the no-silent-dropout contract
    /// on [`crate::navigate`]). When that moniker resolves to a scope
    /// distinct from `from`, this method emits a [`FocusChangedEvent`]
    /// in the same shape [`Self::focus`] would. When it resolves back
    /// to `from` (semantic "stay put") or fails to resolve at all, this
    /// method returns `None` so the adapter does not emit a redundant
    /// focus-changed event.
    ///
    /// Returns `None` when:
    /// - `from` is not registered in `registry`, or
    /// - the strategy returns a moniker for which no scope is
    ///   registered, or
    /// - the resolved key is already focused in its window (the
    ///   common "stay put" outcome under the no-silent-dropout
    ///   contract — the strategy echoed `focused_moniker`).
    ///
    /// This is the seam used by [`crate::navigate::BeamNavStrategy`] —
    /// adapters that want the default Android-beam-search behavior pass
    /// `&BeamNavStrategy::new()`; tests and specialised layouts can
    /// pass a custom impl.
    ///
    /// [`NavStrategy`]: crate::navigate::NavStrategy
    pub fn navigate_with(
        &mut self,
        registry: &SpatialRegistry,
        strategy: &dyn crate::navigate::NavStrategy,
        from: SpatialKey,
        direction: Direction,
    ) -> Option<FocusChangedEvent> {
        // Validate the starting point belongs to the registry. A
        // strategy invocation on an unknown key would otherwise stamp
        // a focus event into a window that has no record of the move.
        // The strategy itself also handles unknown keys (echoes the
        // input moniker with a tracing::error!), but at the
        // `navigate_with` boundary we read the focused moniker from
        // the registry, which requires a real entry.
        let focused_moniker = registry.entry(&from)?.moniker().clone();

        let target_moniker = strategy.next(registry, &from, &focused_moniker, direction);
        // The strategy speaks in monikers; we focus by SpatialKey. The
        // registry is keyed by SpatialKey, so we walk values to find a
        // scope whose moniker matches. The scope set is small (one per
        // mounted scope per window), so a linear scan is cheap relative
        // to a Tauri IPC round-trip.
        let target_key = registry
            .entries_iter()
            .find(|s| s.moniker() == &target_moniker)
            .map(|s| s.key().clone())?;
        // `focus` short-circuits when the resolved key already holds
        // focus — that is the common "stay put" outcome under the new
        // contract (the strategy returned `focused_moniker`). No
        // additional check is required here.
        self.focus(registry, target_key)
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
    /// This is the explicit-clear counterpart of [`Self::focus`] /
    /// [`Self::focus_by_moniker`]. It exists so the React-side
    /// `setFocus(null)` path can dispatch through the kernel and let
    /// the bridge handle the store write — keeping the
    /// "store is a pure projection" invariant from card
    /// `01KQD0WK54G0FRD7SZVZASA9ST`. Without this method, `setFocus(null)`
    /// would have to mutate the React store synchronously to clear
    /// focus, producing exactly the kernel/React drift the card was
    /// filed to eliminate.
    ///
    /// Related: [`Self::handle_unregister`] also produces a
    /// `Some(prev) → None` event when its fallback resolution is
    /// [`FallbackResolution::NoFocus`]. The shape is the same; the
    /// difference is the trigger — `handle_unregister` runs on
    /// scope-deregistration, `clear_focus` runs on an explicit
    /// React-side request.
    pub fn clear_focus(&mut self, window: &WindowLabel) -> Option<FocusChangedEvent> {
        let prev_key = self.focus_by_window.remove(window)?;
        Some(FocusChangedEvent {
            window_label: window.clone(),
            prev_key: Some(prev_key),
            next_key: None,
            next_moniker: None,
        })
    }

    /// Read the focused [`SpatialKey`] for `window`, if any.
    pub fn focused_in(&self, window: &WindowLabel) -> Option<&SpatialKey> {
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
    use crate::types::{LayerKey, LayerName, Pixels, Rect};
    use std::collections::HashMap;

    fn rect_zero() -> Rect {
        Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(0.0),
            height: Pixels::new(0.0),
        }
    }

    /// Build a single-layer registry with one focus scope leaf bound to
    /// `(window, moniker)`.
    fn registry_with_scope(window: &str, layer: &str, key: &str, moniker: &str) -> SpatialRegistry {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(FocusLayer {
            key: LayerKey::from_string(layer),
            name: LayerName::from_string("window"),
            parent: None,
            window_label: WindowLabel::from_string(window),
            last_focused: None,
        });
        reg.register_scope(FocusScope {
            key: SpatialKey::from_string(key),
            moniker: Moniker::from_string(moniker),
            rect: rect_zero(),
            layer_key: LayerKey::from_string(layer),
            parent_zone: None,
            overrides: HashMap::new(),
        });
        reg
    }

    #[test]
    fn focus_returns_event_with_window_and_moniker() {
        let registry = registry_with_scope("main", "L", "k1", "task:01");
        let mut state = SpatialState::new();
        let key = SpatialKey::from_string("k1");

        let event = state
            .focus(&registry, key.clone())
            .expect("focus emits an event");
        assert_eq!(event.window_label, WindowLabel::from_string("main"));
        assert_eq!(event.prev_key, None);
        assert_eq!(event.next_key, Some(key));
        assert_eq!(event.next_moniker, Some(Moniker::from_string("task:01")));
    }

    #[test]
    fn focus_unknown_key_is_noop() {
        let registry = SpatialRegistry::new();
        let mut state = SpatialState::new();
        assert!(state
            .focus(&registry, SpatialKey::from_string("ghost"))
            .is_none());
    }

    #[test]
    fn focus_same_key_twice_emits_once() {
        let registry = registry_with_scope("main", "L", "k1", "task:01");
        let mut state = SpatialState::new();
        let key = SpatialKey::from_string("k1");

        assert!(state.focus(&registry, key.clone()).is_some());
        assert!(state.focus(&registry, key).is_none());
    }
}
