//! Focusable / FocusZone / FocusScope — the three peer types that describe
//! a single registered point in the spatial-nav tree.
//!
//! These types **peer** with React components of the same names. UI is
//! authoritative for structure: `<Focusable>` declares a leaf, `<FocusZone>`
//! declares a navigable container, and the composite `<FocusScope>` wires
//! either primitive into the entity-aware command-dispatch layer. Rust owns
//! all spatial computation (beam search, fallback, layer ops) and stores
//! the registered metadata in [`SpatialRegistry`].
//!
//! ## Why three Rust types instead of one
//!
//! Leaves and zones have a meaningful structural difference: zones own a
//! `last_focused: Option<SpatialKey>` slot used by drill-out / fallback
//! resolution, leaves don't. Modeling that with a single struct + a `kind`
//! field would force every leaf-only access path to either `unwrap()` or
//! ignore a meaningless field. The two distinct structs make the
//! zone-only fields type-checked, and the [`FocusScope`] enum gives the
//! registry a single map keyed by [`SpatialKey`] without losing the
//! variant distinction.
//!
//! ## Wire format
//!
//! [`FocusScope`] serializes with `#[serde(tag = "kind", rename_all =
//! "snake_case")]`, so the wire shape is
//!
//! ```json
//! { "kind": "focusable", "key": "...", "moniker": "...", ... }
//! { "kind": "zone",      "key": "...", "moniker": "...", "last_focused": null, ... }
//! ```
//!
//! The frontend reads `kind` to pick its component shape; the snake-case
//! rename keeps the discriminator consistent with other tagged enums in
//! this crate (`UIStateChange`, `EntityChange`).
//!
//! [`SpatialRegistry`]: super::registry::SpatialRegistry

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::types::{Direction, LayerKey, Moniker, Rect, SpatialKey};

/// A leaf focusable point — atomic, no children, no zone-level features.
///
/// Examples: task title text, status pill, tag pill, button, menu item,
/// breadcrumb item. Each `Focusable` has a stable `key` (ULID minted per
/// mount on the React side), an entity `moniker` (e.g. `"task:01ABC"`,
/// `"ui:toolbar.new"`), a screen-coordinate `rect`, and a `layer_key`
/// pointing at the [`super::layer::FocusLayer`] it lives in.
///
/// `parent_zone` is the immediate enclosing [`FocusZone`], or `None` if
/// this leaf is registered directly under its layer root. `overrides`
/// per-direction lets the React side hard-wire a navigation target (or a
/// `None` "wall") without round-tripping through beam search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Focusable {
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
    /// [`Focusable::overrides`] — `Some(target)` redirects, `None` is a
    /// wall, missing falls through to beam search.
    pub overrides: HashMap<Direction, Option<Moniker>>,
}

/// Sum type over the two registered scope shapes.
///
/// The registry stores one `FocusScope` per [`SpatialKey`]; pattern
/// matching distinguishes leaf vs container at the call site. This is the
/// "scope" in spatial-nav speak — any registered focus point that is not
/// itself a layer.
///
/// The `kind` tag and `snake_case` rename keep the JSON wire shape
/// consistent with other tagged enums in this crate so the React side
/// can read the discriminator the same way it reads `UIStateChange`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FocusScope {
    /// A leaf focusable point. See [`Focusable`].
    Focusable(Focusable),
    /// A navigable container. See [`FocusZone`].
    Zone(FocusZone),
}

impl FocusScope {
    /// Stable identity of the scope, regardless of variant.
    pub fn key(&self) -> &SpatialKey {
        match self {
            Self::Focusable(f) => &f.key,
            Self::Zone(z) => &z.key,
        }
    }

    /// Entity-identity moniker of the scope, regardless of variant.
    pub fn moniker(&self) -> &Moniker {
        match self {
            Self::Focusable(f) => &f.moniker,
            Self::Zone(z) => &z.moniker,
        }
    }

    /// Bounding rect of the scope, regardless of variant.
    pub fn rect(&self) -> &Rect {
        match self {
            Self::Focusable(f) => &f.rect,
            Self::Zone(z) => &z.rect,
        }
    }

    /// Owning layer of the scope, regardless of variant.
    pub fn layer_key(&self) -> &LayerKey {
        match self {
            Self::Focusable(f) => &f.layer_key,
            Self::Zone(z) => &z.layer_key,
        }
    }

    /// Immediate enclosing zone, if any.
    pub fn parent_zone(&self) -> Option<&SpatialKey> {
        match self {
            Self::Focusable(f) => f.parent_zone.as_ref(),
            Self::Zone(z) => z.parent_zone.as_ref(),
        }
    }

    /// Per-direction overrides, regardless of variant.
    pub fn overrides(&self) -> &HashMap<Direction, Option<Moniker>> {
        match self {
            Self::Focusable(f) => &f.overrides,
            Self::Zone(z) => &z.overrides,
        }
    }

    /// `true` if the scope is a [`FocusZone`] container.
    pub fn is_zone(&self) -> bool {
        matches!(self, Self::Zone(_))
    }

    /// `true` if the scope is a [`Focusable`] leaf.
    pub fn is_focusable(&self) -> bool {
        matches!(self, Self::Focusable(_))
    }

    /// Borrow the inner [`FocusZone`] if this scope is a zone, else `None`.
    pub fn as_zone(&self) -> Option<&FocusZone> {
        match self {
            Self::Zone(z) => Some(z),
            Self::Focusable(_) => None,
        }
    }

    /// Mutably borrow the inner [`FocusZone`] if this scope is a zone, else
    /// `None`. Used by the registry to update zone-only fields like
    /// `last_focused` without re-inserting the whole entry.
    pub fn as_zone_mut(&mut self) -> Option<&mut FocusZone> {
        match self {
            Self::Zone(z) => Some(z),
            Self::Focusable(_) => None,
        }
    }

    /// Mutably borrow the rect, regardless of variant. Used by the
    /// registry's `update_rect` to refresh geometry without reallocating
    /// the whole scope.
    pub(crate) fn rect_mut(&mut self) -> &mut Rect {
        match self {
            Self::Focusable(f) => &mut f.rect,
            Self::Zone(z) => &mut z.rect,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit-level coverage for `FocusScope` accessors. Mirrors the
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
    fn focusable_accessors() {
        let scope = FocusScope::Focusable(Focusable {
            key: SpatialKey::from_string("k"),
            moniker: Moniker::from_string("ui:k"),
            rect: rect_zero(),
            layer_key: LayerKey::from_string("L"),
            parent_zone: None,
            overrides: HashMap::new(),
        });

        assert!(scope.is_focusable());
        assert!(!scope.is_zone());
        assert_eq!(scope.as_zone(), None);
        assert_eq!(scope.key(), &SpatialKey::from_string("k"));
    }

    #[test]
    fn zone_accessors() {
        let scope = FocusScope::Zone(FocusZone {
            key: SpatialKey::from_string("k"),
            moniker: Moniker::from_string("ui:k"),
            rect: rect_zero(),
            layer_key: LayerKey::from_string("L"),
            parent_zone: None,
            last_focused: None,
            overrides: HashMap::new(),
        });

        assert!(scope.is_zone());
        assert!(!scope.is_focusable());
        assert!(scope.as_zone().is_some());
    }

    /// Verify the `kind` discriminator is the snake-cased variant name.
    /// The React side mirrors this as `"focusable" | "zone"` so a typo
    /// here would break the runtime decoder.
    #[test]
    fn kind_tag_is_snake_case() {
        let leaf = FocusScope::Focusable(Focusable {
            key: SpatialKey::from_string("k"),
            moniker: Moniker::from_string("ui:k"),
            rect: rect_zero(),
            layer_key: LayerKey::from_string("L"),
            parent_zone: None,
            overrides: HashMap::new(),
        });
        let json = serde_json::to_value(&leaf).unwrap();
        assert_eq!(json["kind"], "focusable");
    }
}
