//! Integration tests for navigation overrides.
//!
//! Overrides are a per-direction directive map that runs as **rule 0**
//! before the beam-search cascade in [`BeamNavStrategy`]. They live on
//! [`FocusScope::overrides`] and [`FocusZone::overrides`] as
//! `HashMap<Direction, Option<Moniker>>` and have three states:
//!
//! - **No entry for the direction** — the override does not apply; the
//!   beam-search cascade runs as usual.
//! - **`Some(target_moniker)`** — redirect to `target_moniker`, but only
//!   when the target is registered in the focused entry's layer. A
//!   target in a different layer is **ignored** and the beam-search
//!   cascade runs (cross-layer teleportation is never allowed).
//! - **`None`** — explicit "wall": navigation in that direction is
//!   blocked regardless of what beam search would have found.
//!
//! These tests exercise the four cases the override resolver must
//! distinguish:
//!
//! 1. Same-layer target → returns the override moniker.
//! 2. `None` (wall) → returns `None`, beam search is **not** consulted.
//! 3. Cross-layer target → falls through to beam search (a sibling that
//!    beam search would otherwise pick is returned).
//! 4. No override entry for the direction → beam search runs as usual.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    BeamNavStrategy, Direction, FocusLayer, FocusScope, FocusZone, LayerKey, LayerName, Moniker,
    NavStrategy, Pixels, Rect, SpatialKey, SpatialRegistry, WindowLabel,
};

// ---------------------------------------------------------------------------
// Builders
// ---------------------------------------------------------------------------

/// Build a `Rect` from raw `f64` coordinates.
fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        x: Pixels::new(x),
        y: Pixels::new(y),
        width: Pixels::new(w),
        height: Pixels::new(h),
    }
}

/// Build a `FocusScope` leaf with empty overrides.
fn leaf(key: &str, moniker: &str, layer: &str, parent_zone: Option<&str>, r: Rect) -> FocusScope {
    FocusScope {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect: r,
        layer_key: LayerKey::from_string(layer),
        parent_zone: parent_zone.map(SpatialKey::from_string),
        overrides: HashMap::new(),
    }
}

/// Build a `FocusScope` leaf carrying the supplied overrides map.
fn leaf_with_overrides(
    key: &str,
    moniker: &str,
    layer: &str,
    parent_zone: Option<&str>,
    r: Rect,
    overrides: HashMap<Direction, Option<Moniker>>,
) -> FocusScope {
    FocusScope {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect: r,
        layer_key: LayerKey::from_string(layer),
        parent_zone: parent_zone.map(SpatialKey::from_string),
        overrides,
    }
}

/// Build a `FocusZone` carrying the supplied overrides map. `last_focused`
/// starts empty.
fn zone_with_overrides(
    key: &str,
    moniker: &str,
    layer: &str,
    parent_zone: Option<&str>,
    r: Rect,
    overrides: HashMap<Direction, Option<Moniker>>,
) -> FocusZone {
    FocusZone {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect: r,
        layer_key: LayerKey::from_string(layer),
        parent_zone: parent_zone.map(SpatialKey::from_string),
        last_focused: None,
        overrides,
    }
}

/// Build a `FocusLayer` rooted at `window` with optional parent.
fn layer(key: &str, window: &str, parent: Option<&str>) -> FocusLayer {
    FocusLayer {
        key: LayerKey::from_string(key),
        name: LayerName::from_string("window"),
        parent: parent.map(LayerKey::from_string),
        window_label: WindowLabel::from_string(window),
        last_focused: None,
    }
}

/// Run the default `BeamNavStrategy` and return the navigated-to moniker.
fn nav(reg: &SpatialRegistry, from: &str, dir: Direction) -> Option<Moniker> {
    BeamNavStrategy::new().next(reg, &SpatialKey::from_string(from), dir)
}

// ---------------------------------------------------------------------------
// Case 1: same-layer override target
// ---------------------------------------------------------------------------

/// `nav.right` from a leaf carrying an override that names a sibling in
/// the same layer returns the named target — even when beam search
/// would have picked a different sibling.
#[test]
fn override_redirects_to_same_layer_target() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));

    // Beam-search candidate: a leaf directly to the right of `src`.
    reg.register_scope(leaf(
        "beam_target",
        "ui:beam_target",
        "L",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));
    // Override candidate: a leaf far below — nothing beam search would
    // pick for `Direction::Right`.
    reg.register_scope(leaf(
        "override_target",
        "ui:override_target",
        "L",
        None,
        rect(0.0, 500.0, 50.0, 50.0),
    ));

    // Source carries an override that redirects right → override_target.
    let mut overrides = HashMap::new();
    overrides.insert(
        Direction::Right,
        Some(Moniker::from_string("ui:override_target")),
    );
    reg.register_scope(leaf_with_overrides(
        "src",
        "ui:src",
        "L",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
        overrides,
    ));

    assert_eq!(
        nav(&reg, "src", Direction::Right),
        Some(Moniker::from_string("ui:override_target")),
        "override target in same layer must win over beam-search candidate"
    );
}

/// Same-layer override applies for zone scopes, not just leaves.
#[test]
fn override_redirects_to_same_layer_target_for_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));

    // Beam-search candidate: a sibling zone to the right of `src`.
    reg.register_zone(zone_with_overrides(
        "beam_zone",
        "ui:beam_zone",
        "L",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
        HashMap::new(),
    ));
    // Override target: a sibling zone elsewhere.
    reg.register_zone(zone_with_overrides(
        "override_zone",
        "ui:override_zone",
        "L",
        None,
        rect(0.0, 500.0, 50.0, 50.0),
        HashMap::new(),
    ));

    let mut overrides = HashMap::new();
    overrides.insert(
        Direction::Right,
        Some(Moniker::from_string("ui:override_zone")),
    );
    reg.register_zone(zone_with_overrides(
        "src",
        "ui:src",
        "L",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
        overrides,
    ));

    assert_eq!(
        nav(&reg, "src", Direction::Right),
        Some(Moniker::from_string("ui:override_zone"))
    );
}

// ---------------------------------------------------------------------------
// Case 2: explicit `None` wall
// ---------------------------------------------------------------------------

/// `nav.right` from a leaf carrying a `None` override for that direction
/// returns `None` — even when a beam-search candidate exists.
#[test]
fn override_none_blocks_navigation() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));

    // A beam-search candidate exists to the right — but the override
    // wall must override it.
    reg.register_scope(leaf(
        "beam_target",
        "ui:beam_target",
        "L",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));

    let mut overrides = HashMap::new();
    overrides.insert(Direction::Right, None);
    reg.register_scope(leaf_with_overrides(
        "src",
        "ui:src",
        "L",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
        overrides,
    ));

    assert_eq!(
        nav(&reg, "src", Direction::Right),
        None,
        "explicit None override must block navigation"
    );
}

/// A `None` override on one direction must not affect navigation in
/// other directions — only the keyed direction is walled.
#[test]
fn override_none_only_blocks_named_direction() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));

    // Candidates to both right and down.
    reg.register_scope(leaf(
        "right_target",
        "ui:right_target",
        "L",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));
    reg.register_scope(leaf(
        "down_target",
        "ui:down_target",
        "L",
        None,
        rect(0.0, 100.0, 50.0, 50.0),
    ));

    // Wall right; leave down untouched.
    let mut overrides = HashMap::new();
    overrides.insert(Direction::Right, None);
    reg.register_scope(leaf_with_overrides(
        "src",
        "ui:src",
        "L",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
        overrides,
    ));

    assert_eq!(nav(&reg, "src", Direction::Right), None);
    assert_eq!(
        nav(&reg, "src", Direction::Down),
        Some(Moniker::from_string("ui:down_target")),
        "down has no override, beam search must run"
    );
}

// ---------------------------------------------------------------------------
// Case 3: cross-layer fall-through
// ---------------------------------------------------------------------------

/// An override pointing at a moniker registered in a *different* layer
/// is ignored — beam search runs as usual.
///
/// The cross-layer target exists in the registry, but it's not in the
/// focused entry's layer. The resolver must reject the override and
/// fall through to the beam-search cascade.
#[test]
fn override_cross_layer_target_falls_through_to_beam_search() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L_window", "main", None));
    reg.push_layer(layer("L_inspector", "main", Some("L_window")));

    // Cross-layer target — exists, but in a different layer.
    reg.register_scope(leaf(
        "cross_layer",
        "ui:cross_layer",
        "L_inspector",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
    ));
    // Beam-search candidate in the same layer as `src`.
    reg.register_scope(leaf(
        "beam_target",
        "ui:beam_target",
        "L_window",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));

    // Override redirects right → a moniker that lives in L_inspector.
    let mut overrides = HashMap::new();
    overrides.insert(
        Direction::Right,
        Some(Moniker::from_string("ui:cross_layer")),
    );
    reg.register_scope(leaf_with_overrides(
        "src",
        "ui:src",
        "L_window",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
        overrides,
    ));

    assert_eq!(
        nav(&reg, "src", Direction::Right),
        Some(Moniker::from_string("ui:beam_target")),
        "cross-layer override target must be ignored; beam search must run"
    );
}

/// Override target moniker that does not exist anywhere in the
/// registry also falls through to beam search — same "didn't apply"
/// outcome as the cross-layer case.
#[test]
fn override_unknown_target_falls_through_to_beam_search() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));

    reg.register_scope(leaf(
        "beam_target",
        "ui:beam_target",
        "L",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));

    let mut overrides = HashMap::new();
    overrides.insert(Direction::Right, Some(Moniker::from_string("ui:ghost")));
    reg.register_scope(leaf_with_overrides(
        "src",
        "ui:src",
        "L",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
        overrides,
    ));

    assert_eq!(
        nav(&reg, "src", Direction::Right),
        Some(Moniker::from_string("ui:beam_target")),
        "unknown override target must be ignored; beam search must run"
    );
}

// ---------------------------------------------------------------------------
// Case 4: no override → beam search delegation
// ---------------------------------------------------------------------------

/// A leaf with no override entry for the requested direction delegates
/// to the beam-search cascade — exactly the legacy behavior. This pins
/// that adding rule-0 hasn't regressed the pre-override path.
#[test]
fn no_override_delegates_to_beam_search() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));

    reg.register_scope(leaf(
        "right_target",
        "ui:right_target",
        "L",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));
    reg.register_scope(leaf("src", "ui:src", "L", None, rect(0.0, 0.0, 50.0, 50.0)));

    assert_eq!(
        nav(&reg, "src", Direction::Right),
        Some(Moniker::from_string("ui:right_target"))
    );
}

/// A leaf with an override for `Right` but not `Left` runs beam search
/// for `Left` — only the keyed direction is consulted by the resolver.
#[test]
fn override_for_one_direction_does_not_affect_others() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));

    // Two candidates, one to the left and one to the right.
    reg.register_scope(leaf(
        "left_target",
        "ui:left_target",
        "L",
        None,
        rect(-100.0, 0.0, 50.0, 50.0),
    ));
    reg.register_scope(leaf(
        "right_target",
        "ui:right_target",
        "L",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));
    // Override target for Right only.
    reg.register_scope(leaf(
        "right_override",
        "ui:right_override",
        "L",
        None,
        rect(0.0, 200.0, 50.0, 50.0),
    ));

    let mut overrides = HashMap::new();
    overrides.insert(
        Direction::Right,
        Some(Moniker::from_string("ui:right_override")),
    );
    reg.register_scope(leaf_with_overrides(
        "src",
        "ui:src",
        "L",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
        overrides,
    ));

    // Right honors the override.
    assert_eq!(
        nav(&reg, "src", Direction::Right),
        Some(Moniker::from_string("ui:right_override"))
    );
    // Left is unconsidered by the override; beam search runs.
    assert_eq!(
        nav(&reg, "src", Direction::Left),
        Some(Moniker::from_string("ui:left_target"))
    );
}
