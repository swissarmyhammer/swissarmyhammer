//! `FocusScope` and `FocusZone` — the two registered struct types that
//! describe a single registered point in the spatial-nav tree.
//!
//! These structs **peer** with React components of the same names. UI is
//! authoritative for structure: `<FocusScope>` declares a leaf, `<FocusZone>`
//! declares a navigable container. Rust owns all spatial computation (beam
//! search, fallback, layer ops) and stores the registered metadata in
//! [`SpatialRegistry`].
//!
//! Together with [`super::layer::FocusLayer`] these form the **three peer
//! types** the spatial-nav kernel exposes. There is no public sum-type
//! enum spanning leaves and zones — the registry stores them via an
//! internal discriminator that is not part of the public API. Consumers
//! who need to iterate "any registered scope" use the registry's
//! variant-aware iterator helpers instead of pattern-matching on a public
//! enum.
//!
//! ## Why two structs instead of one
//!
//! Leaves and zones have a meaningful structural difference: zones own a
//! `last_focused: Option<SpatialKey>` slot used by drill-out / fallback
//! resolution, leaves don't. Modeling that with a single struct + a `kind`
//! field would force every leaf-only access path to either `unwrap()` or
//! ignore a meaningless field. The two distinct structs make the
//! zone-only fields type-checked and keep each peer's wire shape minimal.
//!
//! [`SpatialRegistry`]: super::registry::SpatialRegistry

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::types::{Direction, LayerKey, Moniker, Rect, SpatialKey};

/// A leaf focus scope — atomic, no children, no zone-level features.
///
/// Examples: task title text, status pill, tag pill, button, menu item,
/// breadcrumb item. Each `FocusScope` has a stable `key` (ULID minted per
/// mount on the React side), an entity `moniker` (e.g. `"task:01ABC"`,
/// `"ui:toolbar.new"`), a screen-coordinate `rect`, and a `layer_key`
/// pointing at the [`super::layer::FocusLayer`] it lives in.
///
/// `parent_zone` is the immediate enclosing [`FocusZone`], or `None` if
/// this leaf is registered directly under its layer root. `overrides`
/// per-direction lets the React side hard-wire a navigation target (or a
/// `None` "wall") without round-tripping through beam search.
///
/// This struct is the Rust peer of the React `<FocusScope>` component —
/// the leaf primitive that renders a focus indicator, takes click events,
/// and routes navigation. It used to be named `Focusable`; the rename
/// happened when the React `<Focusable>` and `<FocusScope>` were
/// collapsed into one component (see kanban card
/// `01KQ5PP55SAAVJ0V3HDJ1DGNBY`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FocusScope {
    /// Stable identity for this mount. Minted as a ULID on the React side
    /// so re-renders that don't unmount the component reuse the same key.
    pub key: SpatialKey,
    /// Entity-identity moniker (`"task:01ABC"`, `"ui:toolbar.new"`).
    /// Surfaced on `focus-changed` events as `next_moniker` so the
    /// frontend can drive moniker-keyed effects without an extra IPC
    /// round-trip.
    pub moniker: Moniker,
    /// Bounding rect in viewport coordinates. Drives beam-search distance
    /// and overlap math; updated via [`super::registry::SpatialRegistry::update_rect`]
    /// when ResizeObserver fires on the React side.
    pub rect: Rect,
    /// Layer this leaf belongs to. Beam search and ancestor-zone walks
    /// never cross a layer boundary.
    pub layer_key: LayerKey,
    /// Immediate enclosing zone, if any. `None` means the leaf is
    /// registered directly under the layer root.
    pub parent_zone: Option<SpatialKey>,
    /// Per-direction navigation overrides. `Some(target)` redirects nav
    /// to the named moniker; `None` is an explicit "wall" that blocks
    /// navigation in that direction. Missing key means "fall through to
    /// beam search".
    pub overrides: HashMap<Direction, Option<Moniker>>,
}

/// A navigable container within a layer.
///
/// Zones group leaves; the beam search prefers within-zone candidates
/// first (rule 1) before falling back across zones (rule 2). Each zone
/// owns its own `last_focused` slot for drill-out / fallback memory: when
/// focus leaves the zone and later re-enters it (e.g. via a parent
/// container's drill-in), it lands back on the most recently focused
/// leaf inside.
///
/// Zones form a tree within a layer, rooted at the layer root (`parent_zone
/// = None`). Examples: board container, column, card, inspector panel,
/// field row, nav bar, toolbar group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FocusZone {
    /// Stable identity for this mount. Minted as a ULID on the React side.
    pub key: SpatialKey,
    /// Entity-identity moniker. Zones are typically anchored to a
    /// container entity (e.g. `"column:doing"`, `"task:01ABC"`).
    pub moniker: Moniker,
    /// Bounding rect in viewport coordinates.
    pub rect: Rect,
    /// Layer this zone belongs to.
    pub layer_key: LayerKey,
    /// Immediate enclosing zone, if any. `None` means the zone is
    /// registered directly under the layer root.
    pub parent_zone: Option<SpatialKey>,
    /// Drill-out / fallback memory: the most recently focused descendant
    /// (leaf or child zone) inside this zone. Initialized to `None` and
    /// populated by the navigator when focus changes inside the zone.
    pub last_focused: Option<SpatialKey>,
    /// Per-direction navigation overrides. Same semantics as
    /// [`FocusScope::overrides`] — `Some(target)` redirects, `None` is a
    /// wall, missing falls through to beam search.
    pub overrides: HashMap<Direction, Option<Moniker>>,
}

/// Internal sum type the registry uses to store either a [`FocusScope`]
/// leaf or a [`FocusZone`] container under one [`SpatialKey`]-keyed map.
///
/// Not exported. The public surface is the two struct types only — the
/// kernel's three peer types are layer / zone / scope, with no public
/// enum that conflates leaves and zones. Consumers iterate the registry
/// via the variant-aware helpers (`leaves_in_layer`, `zones_in_layer`,
/// `scopes_iter`) which return the typed structs directly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum RegisteredScope {
    /// A leaf focus scope. See [`FocusScope`].
    Scope(FocusScope),
    /// A navigable container. See [`FocusZone`].
    Zone(FocusZone),
}

impl RegisteredScope {
    /// Stable identity of the scope, regardless of variant.
    pub(crate) fn key(&self) -> &SpatialKey {
        match self {
            Self::Scope(f) => &f.key,
            Self::Zone(z) => &z.key,
        }
    }

    /// Entity-identity moniker of the scope, regardless of variant.
    pub(crate) fn moniker(&self) -> &Moniker {
        match self {
            Self::Scope(f) => &f.moniker,
            Self::Zone(z) => &z.moniker,
        }
    }

    /// Bounding rect of the scope, regardless of variant.
    pub(crate) fn rect(&self) -> &Rect {
        match self {
            Self::Scope(f) => &f.rect,
            Self::Zone(z) => &z.rect,
        }
    }

    /// Owning layer of the scope, regardless of variant.
    pub(crate) fn layer_key(&self) -> &LayerKey {
        match self {
            Self::Scope(f) => &f.layer_key,
            Self::Zone(z) => &z.layer_key,
        }
    }

    /// Immediate enclosing zone, if any.
    pub(crate) fn parent_zone(&self) -> Option<&SpatialKey> {
        match self {
            Self::Scope(f) => f.parent_zone.as_ref(),
            Self::Zone(z) => z.parent_zone.as_ref(),
        }
    }

    /// Per-direction overrides, regardless of variant.
    pub(crate) fn overrides(&self) -> &HashMap<Direction, Option<Moniker>> {
        match self {
            Self::Scope(f) => &f.overrides,
            Self::Zone(z) => &z.overrides,
        }
    }

    /// `true` if the scope is a [`FocusZone`] container.
    pub(crate) fn is_zone(&self) -> bool {
        matches!(self, Self::Zone(_))
    }

    /// `true` if the scope is a [`FocusScope`] leaf.
    pub(crate) fn is_scope(&self) -> bool {
        matches!(self, Self::Scope(_))
    }

    /// Borrow the inner [`FocusZone`] if this scope is a zone, else `None`.
    pub(crate) fn as_zone(&self) -> Option<&FocusZone> {
        match self {
            Self::Zone(z) => Some(z),
            Self::Scope(_) => None,
        }
    }

    /// Borrow the inner [`FocusScope`] leaf if this entry is a leaf, else
    /// `None`.
    pub(crate) fn as_scope(&self) -> Option<&FocusScope> {
        match self {
            Self::Scope(f) => Some(f),
            Self::Zone(_) => None,
        }
    }

    /// Mutably borrow the rect, regardless of variant. Used by the
    /// registry's `update_rect` to refresh geometry without reallocating
    /// the whole scope.
    pub(crate) fn rect_mut(&mut self) -> &mut Rect {
        match self {
            Self::Scope(f) => &mut f.rect,
            Self::Zone(z) => &mut z.rect,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit-level coverage for [`RegisteredScope`] accessors. Mirrors the
    //! integration tests in `tests/focus_registry.rs` so contract drift
    //! is caught at the inner-crate compile step.

    use super::*;
    use crate::types::{LayerKey, Pixels};

    fn rect_zero() -> Rect {
        Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(0.0),
            height: Pixels::new(0.0),
        }
    }

    #[test]
    fn scope_accessors() {
        let scope = RegisteredScope::Scope(FocusScope {
            key: SpatialKey::from_string("k"),
            moniker: Moniker::from_string("ui:k"),
            rect: rect_zero(),
            layer_key: LayerKey::from_string("L"),
            parent_zone: None,
            overrides: HashMap::new(),
        });

        assert!(scope.is_scope());
        assert!(!scope.is_zone());
        assert_eq!(scope.as_zone(), None);
        assert_eq!(scope.key(), &SpatialKey::from_string("k"));
    }

    #[test]
    fn zone_accessors() {
        let scope = RegisteredScope::Zone(FocusZone {
            key: SpatialKey::from_string("k"),
            moniker: Moniker::from_string("ui:k"),
            rect: rect_zero(),
            layer_key: LayerKey::from_string("L"),
            parent_zone: None,
            last_focused: None,
            overrides: HashMap::new(),
        });

        assert!(scope.is_zone());
        assert!(!scope.is_scope());
        assert!(scope.as_zone().is_some());
    }
}
