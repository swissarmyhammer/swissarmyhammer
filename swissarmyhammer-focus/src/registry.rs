//! `SpatialRegistry` — the headless store for spatial scopes and layers.
//!
//! The registry holds two flat maps:
//!
//! - `scopes: HashMap<FullyQualifiedMoniker, RegisteredScope>` — every
//!   registered leaf or container, keyed by its canonical FQM. The
//!   discriminator between leaves and zones lives on an internal enum
//!   ([`super::scope::RegisteredScope`]); the public API exposes the
//!   two typed structs ([`FocusScope`], [`FocusZone`]) directly.
//! - `layers: HashMap<FullyQualifiedMoniker, FocusLayer>` — every
//!   registered layer node, keyed by its FQM.
//!
//! Tree / forest structure is **derived**, not stored: zone hierarchy
//! comes from each scope's `parent_zone`, layer hierarchy from each
//! layer's `parent`. This keeps mutation simple (one map insert per mount)
//! and makes the structural queries (`children_of_zone`, `ancestor_zones`,
//! `children_of_layer`, `ancestors_of_layer`) the source of truth for
//! "what's inside what".
//!
//! ## Path-monikers identifier model
//!
//! The kernel uses **one** identifier shape per primitive: the
//! [`FullyQualifiedMoniker`]. The path through the focus hierarchy IS
//! the spatial key. A consumer constructing a `<FocusZone>` declares
//! a relative [`SegmentMoniker`]; the React adapter composes the FQM
//! through `FullyQualifiedMonikerContext` and ships it through IPC.
//! There is no UUID-based `SpatialKey` and no flat `Moniker`.
//!
//! Path-as-key eliminates the structural bug where a board card field
//! and an inspector panel field share a `SegmentMoniker` (e.g.
//! `field:T1.title`) and end up registered under the same flat key —
//! the FQMs `/window/board/.../card:T1/field:T1.title` and
//! `/window/inspector/field:T1.title` are distinct by construction.
//!
//! ## Three peers, not four
//!
//! The kernel exposes three peer types: [`super::layer::FocusLayer`],
//! [`FocusZone`], and [`FocusScope`]. There is no public sum-type enum
//! that conflates leaves and zones — consumers iterate the registry via
//! the variant-aware helpers ([`leaves_in_layer`], [`zones_in_layer`],
//! [`leaves_iter`], [`zones_iter`]) which yield the typed structs. This
//! mirrors the React side, where `<FocusLayer>`, `<FocusZone>`, and
//! `<FocusScope>` are the three components.
//!
//! [`leaves_in_layer`]: SpatialRegistry::leaves_in_layer
//! [`zones_in_layer`]: SpatialRegistry::zones_in_layer
//! [`leaves_iter`]: SpatialRegistry::leaves_iter
//! [`zones_iter`]: SpatialRegistry::zones_iter
//!
//! ## Threading model
//!
//! `SpatialRegistry` is plain data — not `Sync` on its own. Callers wrap
//! it in a `Mutex`/`RwLock` when they need shared mutable access. The
//! kanban-app `AppState` already serializes spatial commands behind a
//! `tokio::sync::Mutex`, so no additional locking lives here.
//!
//! ## Relationship to `SpatialState`
//!
//! [`super::state::SpatialState`] tracks per-window focus (the
//! `focus_by_window` map) and emits [`super::state::FocusChangedEvent`]s.
//! `SpatialRegistry` tracks the geometry / layer / zone structure that
//! the navigator reads to compute the next focus target. The two are
//! intentionally separate: focus state mutates frequently (every
//! keystroke), structural data mutates only on mount / unmount / resize.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::layer::FocusLayer;
use super::scope::{FocusScope, FocusZone, RegisteredScope};
use super::types::{
    pixels_cmp, Direction, FullyQualifiedMoniker, Rect, SegmentMoniker, WindowLabel,
};

/// Headless store for spatial scopes and layers.
///
/// See module docs for the threading model and the split between scopes
/// and layers. `Default` produces an empty registry; `new` is provided
/// for symmetry with `SpatialState::new`.
#[derive(Debug, Default, Clone)]
pub struct SpatialRegistry {
    /// All registered focus points keyed by their canonical
    /// [`FullyQualifiedMoniker`]. Both [`FocusScope`] leaves and
    /// [`FocusZone`] containers live here behind the internal
    /// [`RegisteredScope`] enum — the public API exposes the typed
    /// structs only.
    scopes: HashMap<FullyQualifiedMoniker, RegisteredScope>,
    /// All registered layers keyed by their canonical
    /// [`FullyQualifiedMoniker`]. Layer hierarchy is derived from each
    /// layer's `parent` field, not stored here.
    layers: HashMap<FullyQualifiedMoniker, FocusLayer>,
}

impl SpatialRegistry {
    /// Construct an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    // ---------------------------------------------------------------------
    // Scope ops
    // ---------------------------------------------------------------------

    /// Register a [`FocusScope`] leaf.
    ///
    /// Replaces any prior scope under the same FQM. A duplicate FQM is
    /// a programmer mistake (two `<FocusScope>` mounts whose composed
    /// paths collide); the kernel surfaces the duplication via
    /// `tracing::error!` and lets the second registration replace the
    /// first so re-mounts under the same path stay idempotent.
    pub fn register_scope(&mut self, f: FocusScope) {
        if self.scopes.contains_key(&f.fq) {
            tracing::error!(
                op = "register_scope",
                fq = %f.fq,
                "duplicate FQM registration replaces prior scope — \
                 a real duplicate FQM is a programmer mistake (two primitives \
                 whose composed paths collide)"
            );
        }
        self.scopes.insert(f.fq.clone(), RegisteredScope::Scope(f));
    }

    /// Register a [`FocusZone`] container.
    ///
    /// Replaces any prior scope under the same FQM. Same semantics as
    /// [`register_scope`].
    ///
    /// [`register_scope`]: SpatialRegistry::register_scope
    pub fn register_zone(&mut self, z: FocusZone) {
        if self.scopes.contains_key(&z.fq) {
            tracing::error!(
                op = "register_zone",
                fq = %z.fq,
                "duplicate FQM registration replaces prior zone — \
                 a real duplicate FQM is a programmer mistake (two primitives \
                 whose composed paths collide)"
            );
        }
        self.scopes.insert(z.fq.clone(), RegisteredScope::Zone(z));
    }

    /// Remove a scope from the registry.
    ///
    /// No-op if the FQM is unknown. The registry does **not** know about
    /// focus claims — adapters that maintain a [`SpatialState`] alongside
    /// the registry must also call
    /// [`SpatialState::handle_unregister`](crate::state::SpatialState::handle_unregister)
    /// on the same FQM so the per-window focus slot is cleared and a
    /// `Some → None` event is emitted for any claim that was active.
    pub fn unregister_scope(&mut self, fq: &FullyQualifiedMoniker) {
        self.scopes.remove(fq);
    }

    /// Update the bounding rect of a registered scope.
    ///
    /// No-op if the FQM is unknown. Called from the React side via
    /// `spatial_update_rect` when ResizeObserver fires.
    pub fn update_rect(&mut self, fq: &FullyQualifiedMoniker, rect: Rect) {
        if let Some(scope) = self.scopes.get_mut(fq) {
            *scope.rect_mut() = rect;
        }
    }

    /// Borrow a leaf [`FocusScope`] by FQM, or `None` if the FQM is
    /// unregistered or registered as a zone.
    ///
    /// Use [`zone`](Self::zone) to look up zones, [`is_registered`](Self::is_registered)
    /// for variant-blind presence checks.
    pub fn scope(&self, fq: &FullyQualifiedMoniker) -> Option<&FocusScope> {
        self.scopes.get(fq).and_then(RegisteredScope::as_scope)
    }

    /// Borrow a [`FocusZone`] by FQM, or `None` if the FQM is
    /// unregistered or registered as a leaf.
    ///
    /// `last_focused` is populated at registration (the kernel preserves
    /// it across re-registers via [`apply_batch`](Self::apply_batch));
    /// the registry does not mutate it after the fact.
    pub fn zone(&self, fq: &FullyQualifiedMoniker) -> Option<&FocusZone> {
        self.scopes.get(fq).and_then(RegisteredScope::as_zone)
    }

    /// `true` when **any** scope (leaf or zone) is registered under
    /// `fq`. Convenience for callers that don't care which variant —
    /// the navigator uses this to validate a starting FQM before
    /// consulting a strategy.
    pub fn is_registered(&self, fq: &FullyQualifiedMoniker) -> bool {
        self.scopes.contains_key(fq)
    }

    /// Look up a registered scope by its canonical FQM.
    ///
    /// This is the **only** lookup-by-identifier API on the registry.
    /// The FQM is the canonical key; there is no leaf-form fallback,
    /// no UUID sidecar, no topmost-layer heuristic. An unknown FQM
    /// returns `None` and the higher-level caller emits
    /// `tracing::error!` per the no-silent-dropout contract.
    ///
    /// Returns a public [`FocusEntry`] view — a variant-aware borrow
    /// over the registered scope so callers that need leaf-vs-zone
    /// variant access can branch via
    /// [`FocusEntry::as_scope`](FocusEntry::as_scope) /
    /// [`FocusEntry::as_zone`](FocusEntry::as_zone). For most consumers,
    /// [`scope`](Self::scope) / [`zone`](Self::zone) are the
    /// variant-typed shorthands.
    pub fn find_by_fq(&self, fq: &FullyQualifiedMoniker) -> Option<FocusEntry<'_>> {
        self.scopes.get(fq).map(FocusEntry::from_registered)
    }

    /// Crate-internal accessor returning the discriminated entry directly.
    ///
    /// External callers should use [`find_by_fq`](Self::find_by_fq),
    /// [`scope`](Self::scope), or [`zone`](Self::zone). The
    /// internal navigator and focus-state code pattern-match on the
    /// entry variant; rather than expose that enum publicly (the kernel
    /// has three peers, not four), we keep the match site inside the
    /// crate.
    pub(crate) fn entry(&self, fq: &FullyQualifiedMoniker) -> Option<&RegisteredScope> {
        self.scopes.get(fq)
    }

    /// Iterate every registered [`FocusScope`] leaf in the registry,
    /// regardless of layer.
    pub fn leaves_iter(&self) -> impl Iterator<Item = &FocusScope> + '_ {
        self.scopes.values().filter_map(RegisteredScope::as_scope)
    }

    /// Iterate every registered [`FocusZone`] container in the registry,
    /// regardless of layer.
    pub fn zones_iter(&self) -> impl Iterator<Item = &FocusZone> + '_ {
        self.scopes.values().filter_map(RegisteredScope::as_zone)
    }

    /// Iterate over the direct children of a zone — scopes whose
    /// `parent_zone` equals `zone_fq`.
    ///
    /// Direct children only; grandchildren whose `parent_zone` points at
    /// some other zone are excluded. Yields a small variant-aware view
    /// (`ChildScope::Leaf` or `ChildScope::Zone`) so callers that need
    /// to distinguish leaf vs container do so without pattern-matching
    /// a public enum.
    pub fn children_of_zone(
        &self,
        zone_fq: &FullyQualifiedMoniker,
    ) -> impl Iterator<Item = ChildScope<'_>> + '_ {
        let zone_fq = zone_fq.clone();
        self.scopes.values().filter_map(move |s| {
            if s.parent_zone() == Some(&zone_fq) {
                Some(child_scope_from_entry(s))
            } else {
                None
            }
        })
    }

    /// Crate-internal version of [`children_of_zone`](Self::children_of_zone)
    /// that yields the discriminated entry directly. Used by the
    /// navigator and state, which already pattern-match internally.
    pub(crate) fn child_entries_of_zone(
        &self,
        zone_fq: &FullyQualifiedMoniker,
    ) -> impl Iterator<Item = &RegisteredScope> + '_ {
        let zone_fq = zone_fq.clone();
        self.scopes
            .values()
            .filter(move |s| s.parent_zone() == Some(&zone_fq))
    }

    /// Walk the `parent_zone` chain from the scope at `fq` upward,
    /// collecting each ancestor [`FocusZone`] in innermost-first order.
    ///
    /// The scope at `fq` is **not** included in the result — only its
    /// ancestors. If `fq` is unknown, returns an empty vector. The walk
    /// stops at the first ancestor that is not itself a zone (which
    /// should not happen in a well-formed registry but is handled
    /// defensively rather than panicking).
    pub fn ancestor_zones(&self, fq: &FullyQualifiedMoniker) -> Vec<&FocusZone> {
        let mut chain = Vec::new();
        let Some(start) = self.scopes.get(fq) else {
            return chain;
        };

        let mut next = start.parent_zone().cloned();
        while let Some(parent_fq) = next {
            let Some(parent) = self.scopes.get(&parent_fq) else {
                break;
            };
            let Some(zone) = parent.as_zone() else {
                // A scope's parent_zone always names a Zone; if the
                // registry is in an inconsistent state, stop walking
                // rather than misclassifying the chain.
                break;
            };
            chain.push(zone);
            next = zone.parent_zone.clone();
        }
        chain
    }

    /// Iterate every leaf [`FocusScope`] in `layer_fq`'s layer.
    ///
    /// Used by the navigator when computing beam-search candidate sets
    /// — leaves outside the active layer are filtered out at this
    /// boundary rather than during scoring.
    pub fn leaves_in_layer(
        &self,
        layer_fq: &FullyQualifiedMoniker,
    ) -> impl Iterator<Item = &FocusScope> + '_ {
        let layer_fq = layer_fq.clone();
        self.scopes.values().filter_map(move |s| match s {
            RegisteredScope::Scope(f) if f.layer_fq == layer_fq => Some(f),
            _ => None,
        })
    }

    /// Iterate every [`FocusZone`] in `layer_fq`'s layer.
    pub fn zones_in_layer(
        &self,
        layer_fq: &FullyQualifiedMoniker,
    ) -> impl Iterator<Item = &FocusZone> + '_ {
        let layer_fq = layer_fq.clone();
        self.scopes.values().filter_map(move |s| match s {
            RegisteredScope::Zone(z) if z.layer_fq == layer_fq => Some(z),
            _ => None,
        })
    }

    /// Crate-internal: iterate every entry (leaf or zone) in
    /// `layer_fq`'s layer.
    pub(crate) fn entries_in_layer(
        &self,
        layer_fq: &FullyQualifiedMoniker,
    ) -> impl Iterator<Item = &RegisteredScope> + '_ {
        let layer_fq = layer_fq.clone();
        self.scopes
            .values()
            .filter(move |s| s.layer_fq() == &layer_fq)
    }

    // ---------------------------------------------------------------------
    // Drill-in / drill-out — explicit zone descent / ascent
    // ---------------------------------------------------------------------

    /// Pick the [`FullyQualifiedMoniker`] to focus when the user drills
    /// *into* the scope at `fq`.
    ///
    /// The semantics are zone-aware:
    ///
    /// - **Zone with a live `last_focused`** — returns that descendant's
    ///   FQM, restoring the user's last position inside the zone
    ///   across drill-out / drill-in cycles.
    /// - **Zone with a stale or absent `last_focused`** — falls back to
    ///   the first child by rect top-left ordering (topmost wins; ties
    ///   broken by leftmost). Matches `Direction::First` ordering so the
    ///   keyboard model stays consistent.
    /// - **Zone with no children** — returns `focused_fq`. The caller
    ///   compares the result against the focused FQM; equal means "no
    ///   descent happened, fall through to edit / no-op".
    /// - **[`FocusScope`] leaf** — returns `focused_fq`. Leaves do not
    ///   have children to drill into; the React side decides
    ///   separately whether the leaf has an inline-edit affordance.
    /// - **Unknown `fq`** — emits `tracing::error!` and returns
    ///   `focused_fq`. The error is observable in logs; the React side
    ///   stays put visually.
    ///
    /// Pure registry query — does not mutate state. The Tauri adapter
    /// translates the returned FQM into a `SpatialState::focus` call
    /// (or back into `setFocus` on the React side). See the
    /// [no-silent-dropout contract] on the `navigate` module for the
    /// reasoning behind echoing `focused_fq` rather than returning
    /// `Option<FullyQualifiedMoniker>`.
    ///
    /// [no-silent-dropout contract]: crate::navigate
    pub fn drill_in(
        &self,
        fq: FullyQualifiedMoniker,
        focused_fq: &FullyQualifiedMoniker,
    ) -> FullyQualifiedMoniker {
        let Some(entry) = self.scopes.get(&fq) else {
            // Torn state: caller passed an FQM with no registry entry.
            // Trace and echo the input FQM.
            tracing::error!(
                op = "drill_in",
                focused_fq = %fq,
                focused = %focused_fq,
                "unknown FQM passed to SpatialRegistry::drill_in"
            );
            return focused_fq.clone();
        };

        let Some(zone) = entry.as_zone() else {
            // Leaf focused — leaves have no children to descend into.
            // Semantic "stay put"; React falls through to inline edit.
            return focused_fq.clone();
        };

        // Honor the zone's remembered position when it still resolves to
        // a registered scope. A `last_focused` whose target was since
        // unregistered is treated the same as no memory at all.
        if let Some(remembered) = &zone.last_focused {
            if let Some(remembered_entry) = self.scopes.get(remembered) {
                return remembered_entry.fq().clone();
            }
        }

        // Cold-start fallback: first child by rect top-left. Tie-break on
        // `left()` so two rows at the same `top` produce a deterministic
        // winner. Borrows from the registry; only the chosen FQM is
        // cloned out. When the zone has no children at all, echo the
        // focused FQM so the caller's no-descent fall-through fires.
        self.child_entries_of_zone(&zone.fq)
            .min_by(|a, b| {
                pixels_cmp(a.rect().top(), b.rect().top())
                    .then(pixels_cmp(a.rect().left(), b.rect().left()))
            })
            .map(|s| s.fq().clone())
            .unwrap_or_else(|| focused_fq.clone())
    }

    /// Pick the [`FullyQualifiedMoniker`] to focus when the user drills
    /// *out of* the scope at `fq`.
    ///
    /// Returns the FQM of the scope's `parent_zone`. Works the same for
    /// both [`FocusScope`] leaves and nested [`FocusZone`] containers —
    /// the result is always the enclosing zone, so a repeated drill-out
    /// walks the zone chain toward the layer root.
    ///
    /// Returns `focused_fq` when:
    /// - the scope at `fq` has no `parent_zone` (sits directly under
    ///   the layer root) — semantic "stay put"; the React side compares
    ///   the result against the focused FQM, equal means "fall
    ///   through to `app.dismiss` (close topmost modal layer)". No
    ///   tracing — this is a well-formed edge.
    /// - `fq` is unknown — torn registry state; emits
    ///   `tracing::error!` and returns the input FQM.
    /// - the `parent_zone` reference points at a scope that is no
    ///   longer registered — torn state; emits `tracing::error!` and
    ///   returns the input FQM.
    ///
    /// Pure registry query — does not mutate state. See the
    /// [no-silent-dropout contract] on the `navigate` module for the
    /// reasoning behind echoing `focused_fq` rather than returning
    /// `Option<FullyQualifiedMoniker>`.
    ///
    /// [no-silent-dropout contract]: crate::navigate
    pub fn drill_out(
        &self,
        fq: FullyQualifiedMoniker,
        focused_fq: &FullyQualifiedMoniker,
    ) -> FullyQualifiedMoniker {
        let Some(entry) = self.scopes.get(&fq) else {
            // Torn state: caller passed an FQM with no registry entry.
            tracing::error!(
                op = "drill_out",
                focused_fq = %fq,
                focused = %focused_fq,
                "unknown FQM passed to SpatialRegistry::drill_out"
            );
            return focused_fq.clone();
        };
        let Some(parent_zone_fq) = entry.parent_zone() else {
            // Layer-root edge — no enclosing zone. Well-formed; the
            // React adapter dispatches `app.dismiss` on the
            // FQM-equality fall-through.
            return focused_fq.clone();
        };
        let Some(parent_entry) = self.scopes.get(parent_zone_fq) else {
            // `parent_zone` names an FQM, but nothing is registered
            // there. Torn state.
            tracing::error!(
                op = "drill_out",
                focused_fq = %fq,
                focused = %focused_fq,
                parent_zone_fq = %parent_zone_fq,
                "parent_zone references unregistered scope"
            );
            return focused_fq.clone();
        };
        parent_entry.fq().clone()
    }

    // ---------------------------------------------------------------------
    // Layer ops
    // ---------------------------------------------------------------------

    /// Register a layer.
    ///
    /// Replaces any prior layer under the same FQM. The "stack" framing
    /// is on the React side (palette opens push, palette closes pop);
    /// the kanban-side store is just a flat map keyed by
    /// [`FullyQualifiedMoniker`].
    pub fn push_layer(&mut self, l: FocusLayer) {
        self.layers.insert(l.fq.clone(), l);
    }

    /// Remove a layer from the registry.
    ///
    /// No-op if the FQM is unknown. Does not cascade to scopes that name
    /// this layer in their `layer_fq` — the React side unmounts those
    /// scopes first via `spatial_unregister_scope`, so the registry
    /// state remains consistent without a GC pass.
    pub fn remove_layer(&mut self, fq: &FullyQualifiedMoniker) {
        self.layers.remove(fq);
    }

    /// Borrow a layer by FQM.
    pub fn layer(&self, fq: &FullyQualifiedMoniker) -> Option<&FocusLayer> {
        self.layers.get(fq)
    }

    /// Direct children of a layer — layers whose `parent` equals `fq`.
    ///
    /// Returns `Vec<&FocusLayer>` rather than `impl Iterator` because
    /// callers typically need to count or sort the children, and the
    /// child set per layer is small (one inspector + maybe one dialog).
    pub fn children_of_layer(&self, fq: &FullyQualifiedMoniker) -> Vec<&FocusLayer> {
        self.layers
            .values()
            .filter(|l| l.parent.as_ref() == Some(fq))
            .collect()
    }

    /// The window root layer for `label` — a layer with the matching
    /// `window_label` and `parent = None`.
    ///
    /// Returns `None` if the window has not registered a root layer yet
    /// (cold start before the React side has called `spatial_push_layer`
    /// for the window). Per the layer-forest invariant there is at most
    /// one root per window; if the registry contains more than one due
    /// to a bug, the first found is returned. The invariant is enforced
    /// with `debug_assert!` in dev builds and a `tracing::warn!` in
    /// release builds so the corruption is visible without panicking
    /// on a user.
    pub fn root_for_window(&self, label: &WindowLabel) -> Option<&FocusLayer> {
        let roots: Vec<&FocusLayer> = self
            .layers
            .values()
            .filter(|l| l.parent.is_none() && &l.window_label == label)
            .collect();

        // Per the layer-forest invariant there is at most one root per
        // window. Two roots means an adapter pushed a second window-
        // root layer without first popping the previous one — a bug
        // worth surfacing rather than silently picking one.
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
    /// ancestors. The walk stops at the window root (whose `parent` is
    /// `None`) or at a missing layer reference, whichever comes first.
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

    // ---------------------------------------------------------------------
    // Batch registration
    // ---------------------------------------------------------------------

    /// Apply a batch of [`RegisterEntry`] values to the registry under a
    /// single mutable borrow.
    ///
    /// This is the headless counterpart to the Tauri
    /// `spatial_register_batch` adapter. The virtualizer in
    /// `kanban-app/ui/src/components/column-view.tsx` constructs a
    /// `Vec<RegisterEntry>` (one entry per off-screen placeholder) and
    /// ships it through a single IPC invoke; the adapter holds the
    /// registry lock once and forwards the slice here.
    ///
    /// Iteration order is the order of the input vector. Each entry is
    /// validated before mutating any existing scope: if any entry fails
    /// the kind-stability check (an FQM registered as one variant being
    /// re-registered as the other), the call returns
    /// [`BatchRegisterError::KindMismatch`] **without** applying any
    /// part of the batch. Successful batches are atomic at the registry
    /// boundary — observers see all-or-nothing.
    ///
    /// # Errors
    /// - [`BatchRegisterError::KindMismatch`] when an entry's variant
    ///   disagrees with the variant already registered under the same
    ///   FQM. The placeholder/real-mount swap relies on
    ///   `register_scope` and `register_zone` being **idempotent on
    ///   FQM but not silently variant-changing**, so the error surface
    ///   is the kernel's contract enforcement point.
    pub fn apply_batch(&mut self, entries: Vec<RegisterEntry>) -> Result<(), BatchRegisterError> {
        // Validate every entry up front so a partial application cannot
        // leave the registry in a half-applied state. Cheap because the
        // current scope set is read-only here — we only check the variant
        // discriminator.
        for entry in &entries {
            let fq = entry.fq();
            if let Some(existing) = self.scopes.get(fq) {
                let existing_is_zone = existing.is_zone();
                let entry_is_zone = matches!(entry, RegisterEntry::Zone { .. });
                if existing_is_zone != entry_is_zone {
                    return Err(BatchRegisterError::KindMismatch {
                        fq: fq.clone(),
                        existing_kind: if existing_is_zone {
                            ScopeKind::Zone
                        } else {
                            ScopeKind::Scope
                        },
                        requested_kind: if entry_is_zone {
                            ScopeKind::Zone
                        } else {
                            ScopeKind::Scope
                        },
                    });
                }
            }
        }

        // Validation passed — apply each entry. The registry's per-FQM
        // overwrite semantics handle the placeholder→real-mount rect
        // refresh transparently; zones preserve their `last_focused`
        // slot across re-registers (rather than resetting it on every
        // virtualizer pass) so drill-out memory survives the swap.
        for entry in entries {
            match entry {
                RegisterEntry::Scope {
                    fq,
                    segment,
                    rect,
                    layer_fq,
                    parent_zone,
                    overrides,
                } => {
                    self.register_scope(FocusScope {
                        fq,
                        segment,
                        rect,
                        layer_fq,
                        parent_zone,
                        overrides,
                    });
                }
                RegisterEntry::Zone {
                    fq,
                    segment,
                    rect,
                    layer_fq,
                    parent_zone,
                    overrides,
                } => {
                    // Preserve any existing `last_focused` so a real-mount
                    // swap from a placeholder doesn't lose drill-out memory
                    // accumulated while the placeholder was live. New zones
                    // start with `None` as before.
                    let last_focused = self
                        .scopes
                        .get(&fq)
                        .and_then(|s| s.as_zone())
                        .and_then(|z| z.last_focused.clone());
                    self.register_zone(FocusZone {
                        fq,
                        segment,
                        rect,
                        layer_fq,
                        parent_zone,
                        last_focused,
                        overrides,
                    });
                }
            }
        }

        Ok(())
    }
}

/// Public variant-aware view over a registered scope, returned by
/// [`SpatialRegistry::entry_for`] (the canonical FQM-keyed lookup).
///
/// Provides the leaf-vs-container split without exposing the internal
/// [`RegisteredScope`] enum. Most consumers use the variant-typed
/// shorthands [`SpatialRegistry::scope`] / [`SpatialRegistry::zone`];
/// [`FocusEntry`] is for callers that need a uniform handle through
/// which to inspect the FQM, segment, or rect regardless of variant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FocusEntry<'a> {
    /// The entry is a leaf [`FocusScope`].
    Leaf(&'a FocusScope),
    /// The entry is a [`FocusZone`] container.
    Zone(&'a FocusZone),
}

impl<'a> FocusEntry<'a> {
    /// Map an internal [`RegisteredScope`] to the public view.
    pub(crate) fn from_registered(reg: &'a RegisteredScope) -> Self {
        match reg {
            RegisteredScope::Scope(f) => Self::Leaf(f),
            RegisteredScope::Zone(z) => Self::Zone(z),
        }
    }

    /// Canonical FQM of the entry, regardless of variant.
    pub fn fq(&self) -> &FullyQualifiedMoniker {
        match self {
            Self::Leaf(f) => &f.fq,
            Self::Zone(z) => &z.fq,
        }
    }

    /// Relative segment of the entry, regardless of variant. For
    /// human-readable logging.
    pub fn segment(&self) -> &SegmentMoniker {
        match self {
            Self::Leaf(f) => &f.segment,
            Self::Zone(z) => &z.segment,
        }
    }

    /// Owning layer's FQM, regardless of variant.
    pub fn layer_fq(&self) -> &FullyQualifiedMoniker {
        match self {
            Self::Leaf(f) => &f.layer_fq,
            Self::Zone(z) => &z.layer_fq,
        }
    }

    /// Bounding rect of the entry, regardless of variant.
    pub fn rect(&self) -> Rect {
        match self {
            Self::Leaf(f) => f.rect,
            Self::Zone(z) => z.rect,
        }
    }

    /// Borrow the inner [`FocusScope`] when the entry is a leaf, else
    /// `None`. The variant-typed counterpart of
    /// [`SpatialRegistry::scope`](SpatialRegistry::scope) for callers
    /// that already hold a [`FocusEntry`].
    pub fn as_scope(&self) -> Option<&'a FocusScope> {
        match self {
            Self::Leaf(f) => Some(*f),
            Self::Zone(_) => None,
        }
    }

    /// Borrow the inner [`FocusZone`] when the entry is a zone, else
    /// `None`. The variant-typed counterpart of
    /// [`SpatialRegistry::zone`](SpatialRegistry::zone) for callers
    /// that already hold a [`FocusEntry`].
    pub fn as_zone(&self) -> Option<&'a FocusZone> {
        match self {
            Self::Zone(z) => Some(*z),
            Self::Leaf(_) => None,
        }
    }
}

/// Variant-aware view returned by [`SpatialRegistry::children_of_zone`].
///
/// Provides the leaf vs container split without exposing the internal
/// [`RegisteredScope`] enum. Consumers that only need the shared fields
/// (`fq`, `segment`, `rect`, `parent_zone`) can use the accessor methods;
/// consumers that need a typed view of one variant pattern-match on the
/// enum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChildScope<'a> {
    /// A leaf [`FocusScope`] child.
    Leaf(&'a FocusScope),
    /// A nested [`FocusZone`] child.
    Zone(&'a FocusZone),
}

impl<'a> ChildScope<'a> {
    /// Canonical FQM of the child, regardless of variant.
    pub fn fq(&self) -> &FullyQualifiedMoniker {
        match self {
            Self::Leaf(f) => &f.fq,
            Self::Zone(z) => &z.fq,
        }
    }

    /// Relative segment of the child, regardless of variant.
    pub fn segment(&self) -> &SegmentMoniker {
        match self {
            Self::Leaf(f) => &f.segment,
            Self::Zone(z) => &z.segment,
        }
    }

    /// Bounding rect of the child, regardless of variant.
    pub fn rect(&self) -> Rect {
        match self {
            Self::Leaf(f) => f.rect,
            Self::Zone(z) => z.rect,
        }
    }
}

/// Adapter from the internal [`RegisteredScope`] enum to the public
/// [`ChildScope`] variant-aware view. Crate-private so the internal
/// enum stays hidden.
fn child_scope_from_entry(entry: &RegisteredScope) -> ChildScope<'_> {
    match entry {
        RegisteredScope::Scope(f) => ChildScope::Leaf(f),
        RegisteredScope::Zone(z) => ChildScope::Zone(z),
    }
}

/// One entry in a batch registration.
///
/// The wire-shape companion to [`FocusScope`] / [`FocusZone`] —
/// reuses the same fields and the same newtypes so the IPC boundary
/// can be a single `Vec<RegisterEntry>` payload. The discriminator
/// uses a `kind` tag with `snake_case` rename so the React side reads
/// the variant the same way it reads other tagged enums in the kernel.
///
/// `last_focused` is intentionally **not** carried on the wire for
/// the `Zone` variant: registration is the React side's "this scope
/// just mounted" signal, and `last_focused` is server-owned drill-out
/// memory that the navigator populates as focus moves. The registry
/// preserves any existing `last_focused` slot when a zone is
/// re-registered (the placeholder/real-mount swap), so the lack of a
/// wire field is correct rather than lossy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RegisterEntry {
    /// A leaf focus scope — see [`FocusScope`].
    Scope {
        /// Canonical FQM for this mount.
        fq: FullyQualifiedMoniker,
        /// Relative segment the consumer declared.
        segment: SegmentMoniker,
        /// Bounding rect in viewport coordinates.
        rect: Rect,
        /// Owning layer's FQM.
        layer_fq: FullyQualifiedMoniker,
        /// Immediate enclosing zone's FQM, if any.
        parent_zone: Option<FullyQualifiedMoniker>,
        /// Per-direction overrides.
        overrides: HashMap<Direction, Option<FullyQualifiedMoniker>>,
    },
    /// A navigable container — see [`FocusZone`].
    Zone {
        /// Canonical FQM for this mount.
        fq: FullyQualifiedMoniker,
        /// Relative segment the consumer declared.
        segment: SegmentMoniker,
        /// Bounding rect in viewport coordinates.
        rect: Rect,
        /// Owning layer's FQM.
        layer_fq: FullyQualifiedMoniker,
        /// Immediate enclosing zone's FQM, if any.
        parent_zone: Option<FullyQualifiedMoniker>,
        /// Per-direction overrides.
        overrides: HashMap<Direction, Option<FullyQualifiedMoniker>>,
    },
}

impl RegisterEntry {
    /// Read the entry's [`FullyQualifiedMoniker`] regardless of variant.
    pub fn fq(&self) -> &FullyQualifiedMoniker {
        match self {
            Self::Scope { fq, .. } | Self::Zone { fq, .. } => fq,
        }
    }
}

/// Discriminator for the [`BatchRegisterError::KindMismatch`] error
/// payload. The variant-on-the-wire `kind` tag in [`RegisterEntry`]
/// uses `snake_case`; this enum is internal to the error surface so
/// it can stay in PascalCase for ergonomic `match` arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// Matches [`RegisterEntry::Scope`] — a leaf [`FocusScope`].
    Scope,
    /// Matches [`RegisterEntry::Zone`] — a [`FocusZone`].
    Zone,
}

/// Errors produced by [`SpatialRegistry::apply_batch`].
///
/// The batch entry point validates kind-stability before mutating any
/// scope, so a returned error guarantees the registry is unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchRegisterError {
    /// An entry's variant disagrees with the variant already
    /// registered under the same FQM.
    ///
    /// The placeholder/real-mount swap requires that whoever generates
    /// the placeholder uses the same FQM **and** the same kind as the
    /// eventual real mount. A mismatch indicates a bug on the React
    /// side (e.g. a zone placeholder for a card that mounts as a leaf),
    /// which the kernel surfaces rather than silently converting types.
    KindMismatch {
        /// The offending FQM.
        fq: FullyQualifiedMoniker,
        /// Kind currently registered under that FQM.
        existing_kind: ScopeKind,
        /// Kind requested by the entry.
        requested_kind: ScopeKind,
    },
}

impl std::fmt::Display for BatchRegisterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KindMismatch {
                fq,
                existing_kind,
                requested_kind,
            } => write!(
                f,
                "FQM {fq} already registered as {existing_kind:?}; cannot re-register as {requested_kind:?}",
            ),
        }
    }
}

impl std::error::Error for BatchRegisterError {}

#[cfg(test)]
mod tests {
    //! Unit-level coverage of the registry. Mirrors the integration
    //! coverage in `tests/focus_registry.rs` so contract drift is caught
    //! at the inner-crate compile step.

    use super::*;
    use crate::types::{FullyQualifiedMoniker, LayerName, Pixels, SegmentMoniker};
    use std::collections::HashMap;

    fn rect() -> Rect {
        Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        }
    }

    fn focus_scope(fq: &str, layer: &str, parent_zone: Option<&str>) -> FocusScope {
        FocusScope {
            fq: FullyQualifiedMoniker::from_string(fq),
            segment: SegmentMoniker::from_string(fq.rsplit('/').next().unwrap_or(fq)),
            rect: rect(),
            layer_fq: FullyQualifiedMoniker::from_string(layer),
            parent_zone: parent_zone.map(FullyQualifiedMoniker::from_string),
            overrides: HashMap::new(),
        }
    }

    fn zone(fq: &str, layer: &str, parent_zone: Option<&str>) -> FocusZone {
        FocusZone {
            fq: FullyQualifiedMoniker::from_string(fq),
            segment: SegmentMoniker::from_string(fq.rsplit('/').next().unwrap_or(fq)),
            rect: rect(),
            layer_fq: FullyQualifiedMoniker::from_string(layer),
            parent_zone: parent_zone.map(FullyQualifiedMoniker::from_string),
            last_focused: None,
            overrides: HashMap::new(),
        }
    }

    fn layer(fq: &str, window: &str, parent: Option<&str>) -> FocusLayer {
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
    fn register_and_lookup() {
        let mut reg = SpatialRegistry::new();
        reg.register_scope(focus_scope("/L/k", "/L", None));
        assert!(reg
            .scope(&FullyQualifiedMoniker::from_string("/L/k"))
            .is_some());
    }

    #[test]
    fn ancestor_zones_walks_chain() {
        let mut reg = SpatialRegistry::new();
        reg.register_zone(zone("/L/outer", "/L", None));
        reg.register_zone(zone("/L/outer/inner", "/L", Some("/L/outer")));
        reg.register_scope(focus_scope(
            "/L/outer/inner/leaf",
            "/L",
            Some("/L/outer/inner"),
        ));

        let chain: Vec<_> = reg
            .ancestor_zones(&FullyQualifiedMoniker::from_string("/L/outer/inner/leaf"))
            .into_iter()
            .map(|z| z.fq.as_str().to_string())
            .collect();
        assert_eq!(
            chain,
            vec!["/L/outer/inner".to_string(), "/L/outer".to_string()]
        );
    }

    #[test]
    fn root_for_window_finds_window_root() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(layer("/win-a", "win-a", None));
        reg.push_layer(layer("/win-a/ins", "win-a", Some("/win-a")));

        let root = reg
            .root_for_window(&WindowLabel::from_string("win-a"))
            .unwrap();
        assert_eq!(root.fq, FullyQualifiedMoniker::from_string("/win-a"));
    }
}
