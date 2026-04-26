//! `SpatialRegistry` — the headless store for spatial scopes and layers.
//!
//! The registry holds two flat maps:
//!
//! - `scopes: HashMap<SpatialKey, FocusScope>` — every registered leaf or
//!   container, keyed by its stable mount key.
//! - `layers: HashMap<LayerKey, FocusLayer>` — every registered layer
//!   node, keyed by its stable mount key.
//!
//! Tree / forest structure is **derived**, not stored: zone hierarchy
//! comes from each scope's `parent_zone`, layer hierarchy from each
//! layer's `parent`. This keeps mutation simple (one map insert per mount)
//! and makes the structural queries (`children_of_zone`, `ancestor_zones`,
//! `children_of_layer`, `ancestors_of_layer`) the source of truth for
//! "what's inside what".
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
//! the navigator (separate card `01KNQXXF5W...`) reads to compute the
//! next focus target. The two are intentionally separate: focus state
//! mutates frequently (every keystroke), structural data mutates only
//! on mount / unmount / resize.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::layer::FocusLayer;
use super::scope::{FocusScope, FocusZone, Focusable};
use super::types::{pixels_cmp, Direction, LayerKey, Moniker, Rect, SpatialKey, WindowLabel};

/// Headless store for spatial scopes and layers.
///
/// See module docs for the threading model and the split between scopes
/// and layers. `Default` produces an empty registry; `new` is provided
/// for symmetry with `SpatialState::new`.
#[derive(Debug, Default, Clone)]
pub struct SpatialRegistry {
    /// All registered focus points keyed by [`SpatialKey`]. Both
    /// [`Focusable`] leaves and [`FocusZone`] containers live here behind
    /// the [`FocusScope`] enum.
    scopes: HashMap<SpatialKey, FocusScope>,
    /// All registered layers keyed by [`LayerKey`]. Layer hierarchy is
    /// derived from each layer's `parent` field, not stored here.
    layers: HashMap<LayerKey, FocusLayer>,
}

impl SpatialRegistry {
    /// Construct an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    // ---------------------------------------------------------------------
    // Scope ops
    // ---------------------------------------------------------------------

    /// Register a [`Focusable`] leaf.
    ///
    /// Replaces any prior scope under the same key. Replacement semantics
    /// are intentional: a hot-reload that reuses a key cannot strand
    /// stale metadata, and re-mounting the same component is idempotent.
    pub fn register_focusable(&mut self, f: Focusable) {
        self.scopes.insert(f.key.clone(), FocusScope::Focusable(f));
    }

    /// Register a [`FocusZone`] container.
    ///
    /// Replaces any prior scope under the same key. Same semantics as
    /// [`register_focusable`].
    ///
    /// [`register_focusable`]: SpatialRegistry::register_focusable
    pub fn register_zone(&mut self, z: FocusZone) {
        self.scopes.insert(z.key.clone(), FocusScope::Zone(z));
    }

    /// Remove a scope from the registry.
    ///
    /// No-op if the key is unknown. The registry does **not** know about
    /// focus claims — adapters that maintain a [`SpatialState`] alongside
    /// the registry must also call
    /// [`SpatialState::handle_unregister`](crate::state::SpatialState::handle_unregister)
    /// on the same key so the per-window focus slot is cleared and a
    /// `Some → None` event is emitted for any claim that was active.
    ///
    /// Now that `SpatialState` no longer mirrors a per-key entry table
    /// (see review note "Duplicate (SpatialKey, Moniker) pair"), there
    /// is nothing on the focus tracker that *requires* a registry
    /// unregister to be paired — but the focus slot still needs to be
    /// cleared, otherwise the event the React claim registry needs to
    /// release the visual focus never fires. Hence the doc reminder
    /// rather than a hard coupling.
    pub fn unregister_scope(&mut self, key: &SpatialKey) {
        self.scopes.remove(key);
    }

    /// Update the bounding rect of a registered scope.
    ///
    /// No-op if the key is unknown. Called from the React side via
    /// `spatial_update_rect` when ResizeObserver fires.
    pub fn update_rect(&mut self, key: &SpatialKey, rect: Rect) {
        if let Some(scope) = self.scopes.get_mut(key) {
            *scope.rect_mut() = rect;
        }
    }

    /// Borrow a scope by key.
    pub fn scope(&self, key: &SpatialKey) -> Option<&FocusScope> {
        self.scopes.get(key)
    }

    /// Iterate over every registered scope in the registry, regardless
    /// of layer. Used by the navigator when resolving a moniker the
    /// strategy returned back to the [`SpatialKey`] it was registered
    /// under.
    pub fn scopes_iter(&self) -> impl Iterator<Item = &FocusScope> + '_ {
        self.scopes.values()
    }

    /// Iterate over the direct children of a zone — scopes whose
    /// `parent_zone` equals `zone_key`.
    ///
    /// Direct children only; grandchildren whose `parent_zone` points at
    /// some other zone are excluded. The returned iterator borrows from
    /// the registry; clone the keys you care about if you need to outlive
    /// the borrow.
    pub fn children_of_zone(
        &self,
        zone_key: &SpatialKey,
    ) -> impl Iterator<Item = &FocusScope> + '_ {
        let zone_key = zone_key.clone();
        self.scopes
            .values()
            .filter(move |s| s.parent_zone() == Some(&zone_key))
    }

    /// Walk the `parent_zone` chain from the scope at `key` upward,
    /// collecting each ancestor [`FocusZone`] in innermost-first order.
    ///
    /// The scope at `key` is **not** included in the result — only its
    /// ancestors. If `key` is unknown, returns an empty vector. The walk
    /// stops at the first ancestor that is not itself a zone (which
    /// should not happen in a well-formed registry but is handled
    /// defensively rather than panicking).
    pub fn ancestor_zones(&self, key: &SpatialKey) -> Vec<&FocusZone> {
        let mut chain = Vec::new();
        let Some(start) = self.scopes.get(key) else {
            return chain;
        };

        let mut next = start.parent_zone().cloned();
        while let Some(parent_key) = next {
            let Some(parent) = self.scopes.get(&parent_key) else {
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

    /// Iterate over every scope in `key`'s layer.
    ///
    /// Returns both `Focusable` and `Zone` variants whose `layer_key`
    /// matches the queried layer. Used by the navigator when computing
    /// beam-search candidate sets — candidates outside the active layer
    /// are filtered out at this boundary rather than during scoring.
    pub fn scopes_in_layer(&self, key: &LayerKey) -> impl Iterator<Item = &FocusScope> + '_ {
        let key = key.clone();
        self.scopes.values().filter(move |s| s.layer_key() == &key)
    }

    // ---------------------------------------------------------------------
    // Drill-in / drill-out — explicit zone descent / ascent
    // ---------------------------------------------------------------------

    /// Pick the [`Moniker`] to focus when the user drills *into* the scope
    /// at `key`.
    ///
    /// The semantics are zone-aware:
    ///
    /// - **Zone with a live `last_focused`** — returns that descendant's
    ///   moniker, restoring the user's last position inside the zone
    ///   across drill-out / drill-in cycles.
    /// - **Zone with a stale or absent `last_focused`** — falls back to
    ///   the first child by rect top-left ordering (topmost wins; ties
    ///   broken by leftmost). Matches `Direction::First` ordering so the
    ///   keyboard model stays consistent.
    /// - **Zone with no children** — returns `None`. Frontend stays put.
    /// - **`Focusable` leaf** — returns `None`. Leaves do not have
    ///   children to drill into; the React side decides separately
    ///   whether the leaf has an inline-edit affordance to invoke.
    /// - **Unknown `key`** — returns `None`. The frontend falls through
    ///   to the next command in the chain.
    ///
    /// Pure registry query — does not mutate state. The Tauri adapter
    /// translates the returned moniker into a `SpatialState::focus` call
    /// (or back into `setFocus` on the React side).
    pub fn drill_in(&self, key: SpatialKey) -> Option<Moniker> {
        let scope = self.scope(&key)?;
        let zone = scope.as_zone()?;

        // Honor the zone's remembered position when it still resolves to
        // a registered scope. A `last_focused` whose target was since
        // unregistered is treated the same as no memory at all.
        if let Some(remembered) = &zone.last_focused {
            if let Some(remembered_scope) = self.scope(remembered) {
                return Some(remembered_scope.moniker().clone());
            }
        }

        // Cold-start fallback: first child by rect top-left. Tie-break on
        // `left()` so two rows at the same `top` produce a deterministic
        // winner. Borrows from the registry; only the chosen moniker is
        // cloned out.
        self.children_of_zone(&zone.key)
            .min_by(|a, b| {
                pixels_cmp(a.rect().top(), b.rect().top())
                    .then(pixels_cmp(a.rect().left(), b.rect().left()))
            })
            .map(|s| s.moniker().clone())
    }

    /// Pick the [`Moniker`] to focus when the user drills *out of* the
    /// scope at `key`.
    ///
    /// Returns the [`Moniker`] of the scope's `parent_zone`. Works the
    /// same for both [`Focusable`] leaves and nested [`FocusZone`]
    /// containers — the result is always the enclosing zone, so a
    /// repeated drill-out walks the zone chain toward the layer root.
    ///
    /// Returns `None` when:
    /// - `key` is unknown — the frontend falls through to the next
    ///   command in the chain (typically `app.dismiss`).
    /// - the scope at `key` has no `parent_zone` (sits directly under
    ///   the layer root) — same fall-through behavior.
    /// - the `parent_zone` reference points at a scope that is no longer
    ///   registered (torn registry state) — degraded gracefully rather
    ///   than panicking.
    ///
    /// Pure registry query — does not mutate state.
    pub fn drill_out(&self, key: SpatialKey) -> Option<Moniker> {
        let scope = self.scope(&key)?;
        let parent_zone_key = scope.parent_zone()?;
        self.scope(parent_zone_key).map(|s| s.moniker().clone())
    }

    // ---------------------------------------------------------------------
    // Layer ops
    // ---------------------------------------------------------------------

    /// Register a layer.
    ///
    /// Replaces any prior layer under the same key. The "stack" framing
    /// is on the React side (palette opens push, palette closes pop);
    /// the kanban-side store is just a flat map keyed by [`LayerKey`].
    pub fn push_layer(&mut self, l: FocusLayer) {
        self.layers.insert(l.key.clone(), l);
    }

    /// Remove a layer from the registry.
    ///
    /// No-op if the key is unknown. Does not cascade to scopes that name
    /// this layer in their `layer_key` — the React side unmounts those
    /// scopes first via `spatial_unregister_scope`, so the registry
    /// state remains consistent without a GC pass.
    pub fn remove_layer(&mut self, key: &LayerKey) {
        self.layers.remove(key);
    }

    /// Borrow a layer by key.
    pub fn layer(&self, key: &LayerKey) -> Option<&FocusLayer> {
        self.layers.get(key)
    }

    /// Direct children of a layer — layers whose `parent` equals `key`.
    ///
    /// Returns `Vec<&FocusLayer>` rather than `impl Iterator` because
    /// callers typically need to count or sort the children, and the
    /// child set per layer is small (one inspector + maybe one dialog).
    pub fn children_of_layer(&self, key: &LayerKey) -> Vec<&FocusLayer> {
        self.layers
            .values()
            .filter(|l| l.parent.as_ref() == Some(key))
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

    /// Walk the `parent` chain from the layer at `key` upward, collecting
    /// each ancestor [`FocusLayer`] in innermost-first order.
    ///
    /// The layer at `key` is **not** included in the result — only its
    /// ancestors. The walk stops at the window root (whose `parent` is
    /// `None`) or at a missing layer reference, whichever comes first.
    pub fn ancestors_of_layer(&self, key: &LayerKey) -> Vec<&FocusLayer> {
        let mut chain = Vec::new();
        let Some(start) = self.layers.get(key) else {
            return chain;
        };

        let mut next = start.parent.clone();
        while let Some(parent_key) = next {
            let Some(parent) = self.layers.get(&parent_key) else {
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
    /// the kind-stability check (a key registered as one variant being
    /// re-registered as the other), the call returns
    /// [`BatchRegisterError::KindMismatch`] **without** applying any
    /// part of the batch. Successful batches are atomic at the registry
    /// boundary — observers see all-or-nothing.
    ///
    /// # Errors
    /// - [`BatchRegisterError::KindMismatch`] when an entry's variant
    ///   disagrees with the variant already registered under the same
    ///   `SpatialKey`. The placeholder/real-mount swap relies on
    ///   `register_focusable` and `register_zone` being **idempotent on
    ///   key but not silently variant-changing**, so the error surface
    ///   is the kernel's contract enforcement point.
    pub fn apply_batch(&mut self, entries: Vec<RegisterEntry>) -> Result<(), BatchRegisterError> {
        // Validate every entry up front so a partial application cannot
        // leave the registry in a half-applied state. Cheap because the
        // current scope set is read-only here — we only check the variant
        // discriminator.
        for entry in &entries {
            let key = entry.key();
            if let Some(existing) = self.scopes.get(key) {
                let existing_is_zone = existing.is_zone();
                let entry_is_zone = matches!(entry, RegisterEntry::Zone { .. });
                if existing_is_zone != entry_is_zone {
                    return Err(BatchRegisterError::KindMismatch {
                        key: key.clone(),
                        existing_kind: if existing_is_zone {
                            ScopeKind::Zone
                        } else {
                            ScopeKind::Focusable
                        },
                        requested_kind: if entry_is_zone {
                            ScopeKind::Zone
                        } else {
                            ScopeKind::Focusable
                        },
                    });
                }
            }
        }

        // Validation passed — apply each entry. The registry's per-key
        // overwrite semantics handle the placeholder→real-mount rect
        // refresh transparently; zones preserve their `last_focused`
        // slot across re-registers (rather than resetting it on every
        // virtualizer pass) so drill-out memory survives the swap.
        for entry in entries {
            match entry {
                RegisterEntry::Focusable {
                    key,
                    moniker,
                    rect,
                    layer_key,
                    parent_zone,
                    overrides,
                } => {
                    self.register_focusable(Focusable {
                        key,
                        moniker,
                        rect,
                        layer_key,
                        parent_zone,
                        overrides,
                    });
                }
                RegisterEntry::Zone {
                    key,
                    moniker,
                    rect,
                    layer_key,
                    parent_zone,
                    overrides,
                } => {
                    // Preserve any existing `last_focused` so a real-mount
                    // swap from a placeholder doesn't lose drill-out memory
                    // accumulated while the placeholder was live. New zones
                    // start with `None` as before.
                    let last_focused = self
                        .scopes
                        .get(&key)
                        .and_then(|s| s.as_zone())
                        .and_then(|z| z.last_focused.clone());
                    self.register_zone(FocusZone {
                        key,
                        moniker,
                        rect,
                        layer_key,
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

/// One entry in a batch registration.
///
/// The wire-shape companion to [`Focusable`] / [`FocusZone`] —
/// reuses the same fields and the same newtypes so the IPC boundary
/// can be a single `Vec<RegisterEntry>` payload. The discriminator
/// uses a `kind` tag with `snake_case` rename so the React side reads
/// the variant the same way it reads `FocusScope` (which uses the
/// same shape).
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
    /// A leaf focusable point — see [`Focusable`].
    Focusable {
        /// Stable identity for this mount.
        key: SpatialKey,
        /// Entity-identity moniker for the leaf.
        moniker: Moniker,
        /// Bounding rect in viewport coordinates.
        rect: Rect,
        /// Owning layer.
        layer_key: LayerKey,
        /// Immediate enclosing zone, if any.
        parent_zone: Option<SpatialKey>,
        /// Per-direction overrides.
        overrides: HashMap<Direction, Option<Moniker>>,
    },
    /// A navigable container — see [`FocusZone`].
    Zone {
        /// Stable identity for this mount.
        key: SpatialKey,
        /// Entity-identity moniker for the zone.
        moniker: Moniker,
        /// Bounding rect in viewport coordinates.
        rect: Rect,
        /// Owning layer.
        layer_key: LayerKey,
        /// Immediate enclosing zone, if any.
        parent_zone: Option<SpatialKey>,
        /// Per-direction overrides.
        overrides: HashMap<Direction, Option<Moniker>>,
    },
}

impl RegisterEntry {
    /// Read the entry's [`SpatialKey`] regardless of variant.
    pub fn key(&self) -> &SpatialKey {
        match self {
            Self::Focusable { key, .. } | Self::Zone { key, .. } => key,
        }
    }
}

/// Discriminator for the [`BatchRegisterError::KindMismatch`] error
/// payload. The variant-on-the-wire `kind` tag in [`RegisterEntry`]
/// uses `snake_case`; this enum is internal to the error surface so
/// it can stay in PascalCase for ergonomic `match` arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// Matches [`RegisterEntry::Focusable`] and [`FocusScope::Focusable`].
    Focusable,
    /// Matches [`RegisterEntry::Zone`] and [`FocusScope::Zone`].
    Zone,
}

/// Errors produced by [`SpatialRegistry::apply_batch`].
///
/// The batch entry point validates kind-stability before mutating any
/// scope, so a returned error guarantees the registry is unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchRegisterError {
    /// An entry's variant disagrees with the variant already
    /// registered under the same [`SpatialKey`].
    ///
    /// The placeholder/real-mount swap requires that whoever generates
    /// the placeholder uses the same [`SpatialKey`] **and** the same
    /// kind as the eventual real mount. A mismatch indicates a bug on
    /// the React side (e.g. a zone placeholder for a card that mounts
    /// as a leaf), which the kernel surfaces rather than silently
    /// converting types.
    KindMismatch {
        /// The offending [`SpatialKey`].
        key: SpatialKey,
        /// Kind currently registered under that key.
        existing_kind: ScopeKind,
        /// Kind requested by the entry.
        requested_kind: ScopeKind,
    },
}

impl std::fmt::Display for BatchRegisterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KindMismatch {
                key,
                existing_kind,
                requested_kind,
            } => write!(
                f,
                "spatial key {key} already registered as {existing_kind:?}; cannot re-register as {requested_kind:?}",
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
    use crate::types::{LayerName, Moniker, Pixels};
    use std::collections::HashMap;

    fn rect() -> Rect {
        Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        }
    }

    fn focusable(key: &str, layer: &str, parent_zone: Option<&str>) -> Focusable {
        Focusable {
            key: SpatialKey::from_string(key),
            moniker: Moniker::from_string(format!("ui:{key}")),
            rect: rect(),
            layer_key: LayerKey::from_string(layer),
            parent_zone: parent_zone.map(SpatialKey::from_string),
            overrides: HashMap::new(),
        }
    }

    fn zone(key: &str, layer: &str, parent_zone: Option<&str>) -> FocusZone {
        FocusZone {
            key: SpatialKey::from_string(key),
            moniker: Moniker::from_string(format!("ui:{key}")),
            rect: rect(),
            layer_key: LayerKey::from_string(layer),
            parent_zone: parent_zone.map(SpatialKey::from_string),
            last_focused: None,
            overrides: HashMap::new(),
        }
    }

    fn layer(key: &str, window: &str, parent: Option<&str>) -> FocusLayer {
        FocusLayer {
            key: LayerKey::from_string(key),
            name: LayerName::from_string("window"),
            parent: parent.map(LayerKey::from_string),
            window_label: WindowLabel::from_string(window),
            last_focused: None,
        }
    }

    #[test]
    fn register_and_lookup() {
        let mut reg = SpatialRegistry::new();
        reg.register_focusable(focusable("k", "L", None));
        assert!(reg.scope(&SpatialKey::from_string("k")).is_some());
    }

    #[test]
    fn ancestor_zones_walks_chain() {
        let mut reg = SpatialRegistry::new();
        reg.register_zone(zone("outer", "L", None));
        reg.register_zone(zone("inner", "L", Some("outer")));
        reg.register_focusable(focusable("leaf", "L", Some("inner")));

        let chain: Vec<_> = reg
            .ancestor_zones(&SpatialKey::from_string("leaf"))
            .into_iter()
            .map(|z| z.key.as_str().to_string())
            .collect();
        assert_eq!(chain, vec!["inner".to_string(), "outer".to_string()]);
    }

    #[test]
    fn root_for_window_finds_window_root() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(layer("root", "win-a", None));
        reg.push_layer(layer("ins", "win-a", Some("root")));

        let root = reg
            .root_for_window(&WindowLabel::from_string("win-a"))
            .unwrap();
        assert_eq!(root.key, LayerKey::from_string("root"));
    }
}
