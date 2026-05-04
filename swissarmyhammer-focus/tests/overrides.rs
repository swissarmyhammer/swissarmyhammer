//! Integration tests for navigation overrides.
//!
//! Overrides are a per-direction directive map that runs as **rule 0**
//! before the beam-search cascade in [`BeamNavStrategy`]. They live on
//! [`FocusScope::overrides`] and [`FocusScope::overrides`] as
//! `HashMap<Direction, Option<FullyQualifiedMoniker>>` and have three
//! states:
//!
//! - **No entry for the direction** — the override does not apply; the
//!   beam-search cascade runs as usual.
//! - **`Some(target_fq)`** — redirect to `target_fq`, but only
//!   when the target is registered in the focused entry's layer. A
//!   target in a different layer is **ignored** and the beam-search
//!   cascade runs (cross-layer teleportation is never allowed).
//! - **`None`** — explicit "wall": navigation in that direction is
//!   blocked regardless of what beam search would have found.
//!
//! These tests exercise the four cases the override resolver must
//! distinguish:
//!
//! 1. Same-layer target → returns the override FQM.
//! 2. `None` (wall) → echoes focused FQM, beam search is **not**
//!    consulted.
//! 3. Cross-layer target → falls through to beam search (a sibling that
//!    beam search would otherwise pick is returned).
//! 4. No override entry for the direction → beam search runs as usual.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    BeamNavStrategy, Direction, FocusLayer, FocusScope, FullyQualifiedMoniker,
    LayerName, NavStrategy, Pixels, Rect, SegmentMoniker, SpatialRegistry, WindowLabel,
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

/// FQM for a primitive registered directly under a layer's root.
fn fq_in_layer(layer_path: &str, segment: &str) -> FullyQualifiedMoniker {
    FullyQualifiedMoniker::from_string(format!("{layer_path}/{segment}"))
}

/// Build a `FocusScope` leaf with empty overrides. `segment` is the
/// last component of the FQM by convention here.
fn leaf(
    segment: &str,
    layer: &str,
    parent_zone: Option<FullyQualifiedMoniker>,
    r: Rect,
) -> FocusScope {
    FocusScope {
        fq: fq_in_layer(layer, segment),
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone,
        overrides: HashMap::new(),
        last_focused: None,
    }
}

/// Build a `FocusScope` leaf carrying the supplied overrides map.
fn leaf_with_overrides(
    segment: &str,
    layer: &str,
    parent_zone: Option<FullyQualifiedMoniker>,
    r: Rect,
    overrides: HashMap<Direction, Option<FullyQualifiedMoniker>>,
) -> FocusScope {
    FocusScope {
        fq: fq_in_layer(layer, segment),
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone,
        overrides,
        last_focused: None,
    }
}

/// Build a `FocusScope` carrying the supplied overrides map. `last_focused`
/// starts empty.
fn zone_with_overrides(
    segment: &str,
    layer: &str,
    parent_zone: Option<FullyQualifiedMoniker>,
    r: Rect,
    overrides: HashMap<Direction, Option<FullyQualifiedMoniker>>,
) -> FocusScope {
    FocusScope {
        fq: fq_in_layer(layer, segment),
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone,
        last_focused: None,
        overrides,
    }
}

/// Build a `FocusLayer` rooted at `window` with optional parent.
fn layer(layer_fq: &str, segment: &str, window: &str, parent: Option<&str>) -> FocusLayer {
    FocusLayer {
        fq: FullyQualifiedMoniker::from_string(layer_fq),
        segment: SegmentMoniker::from_string(segment),
        name: LayerName::from_string("window"),
        parent: parent.map(FullyQualifiedMoniker::from_string),
        window_label: WindowLabel::from_string(window),
        last_focused: None,
    }
}

/// Run the default `BeamNavStrategy` and return the navigated-to FQM.
///
/// Resolves the focused entry's segment from the registry — under the
/// no-silent-dropout contract every nav call needs the focused segment
/// alongside the focused FQM. For unknown FQMs, falls back to a
/// synthetic segment derived from the leaf component.
fn nav(
    reg: &SpatialRegistry,
    from: &FullyQualifiedMoniker,
    dir: Direction,
) -> FullyQualifiedMoniker {
    let focused_segment = reg
        .find_by_fq(from)
        .map(|e| e.segment.clone())
        .unwrap_or_else(|| {
            let s = from.as_str().rsplit('/').next().unwrap_or("");
            SegmentMoniker::from_string(s)
        });
    BeamNavStrategy::new().next(reg, from, &focused_segment, dir)
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
    reg.push_layer(layer("/L", "L", "main", None));

    // Beam-search candidate: a leaf directly to the right of `src`.
    reg.register_scope(leaf(
        "ui:beam_target",
        "/L",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));
    // Override candidate: a leaf far below — nothing beam search would
    // pick for `Direction::Right`.
    let override_target_fq = fq_in_layer("/L", "ui:override_target");
    reg.register_scope(leaf(
        "ui:override_target",
        "/L",
        None,
        rect(0.0, 500.0, 50.0, 50.0),
    ));

    // Source carries an override that redirects right → override_target.
    let mut overrides = HashMap::new();
    overrides.insert(Direction::Right, Some(override_target_fq.clone()));
    let src_fq = fq_in_layer("/L", "ui:src");
    reg.register_scope(leaf_with_overrides(
        "ui:src",
        "/L",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
        overrides,
    ));

    assert_eq!(
        nav(&reg, &src_fq, Direction::Right),
        override_target_fq,
        "override target in same layer must win over beam-search candidate"
    );
}

/// Same-layer override applies for zone scopes, not just leaves.
#[test]
fn override_redirects_to_same_layer_target_for_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));

    // Beam-search candidate: a sibling zone to the right of `src`.
    reg.register_scope(zone_with_overrides(
        "ui:beam_zone",
        "/L",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
        HashMap::new(),
    ));
    // Override target: a sibling zone elsewhere.
    let override_zone_fq = fq_in_layer("/L", "ui:override_zone");
    reg.register_scope(zone_with_overrides(
        "ui:override_zone",
        "/L",
        None,
        rect(0.0, 500.0, 50.0, 50.0),
        HashMap::new(),
    ));

    let mut overrides = HashMap::new();
    overrides.insert(Direction::Right, Some(override_zone_fq.clone()));
    let src_fq = fq_in_layer("/L", "ui:src");
    reg.register_scope(zone_with_overrides(
        "ui:src",
        "/L",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
        overrides,
    ));

    assert_eq!(nav(&reg, &src_fq, Direction::Right), override_zone_fq);
}

// ---------------------------------------------------------------------------
// Case 2: explicit `None` wall
// ---------------------------------------------------------------------------

/// `nav.right` from a leaf carrying a `None` override for that direction
/// echoes the focused FQM — even when a beam-search candidate exists.
#[test]
fn override_none_blocks_navigation() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));

    // A beam-search candidate exists to the right — but the override
    // wall must override it.
    reg.register_scope(leaf(
        "ui:beam_target",
        "/L",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));

    let mut overrides = HashMap::new();
    overrides.insert(Direction::Right, None);
    let src_fq = fq_in_layer("/L", "ui:src");
    reg.register_scope(leaf_with_overrides(
        "ui:src",
        "/L",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
        overrides,
    ));

    assert_eq!(
        nav(&reg, &src_fq, Direction::Right),
        src_fq,
        "explicit None override must block navigation — under the no-silent-dropout \
         contract this echoes the focused FQM (the React side detects equality \
         and stays put)"
    );
}

/// A `None` override on one direction must not affect navigation in
/// other directions — only the keyed direction is walled.
#[test]
fn override_none_only_blocks_named_direction() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));

    // Candidates to both right and down.
    reg.register_scope(leaf(
        "ui:right_target",
        "/L",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));
    let down_target_fq = fq_in_layer("/L", "ui:down_target");
    reg.register_scope(leaf(
        "ui:down_target",
        "/L",
        None,
        rect(0.0, 100.0, 50.0, 50.0),
    ));

    // Wall right; leave down untouched.
    let mut overrides = HashMap::new();
    overrides.insert(Direction::Right, None);
    let src_fq = fq_in_layer("/L", "ui:src");
    reg.register_scope(leaf_with_overrides(
        "ui:src",
        "/L",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
        overrides,
    ));

    assert_eq!(nav(&reg, &src_fq, Direction::Right), src_fq);
    assert_eq!(
        nav(&reg, &src_fq, Direction::Down),
        down_target_fq,
        "down has no override, beam search must run"
    );
}

// ---------------------------------------------------------------------------
// Case 3: cross-layer fall-through
// ---------------------------------------------------------------------------

/// An override pointing at an FQM registered in a *different* layer
/// is ignored — beam search runs as usual.
///
/// The cross-layer target exists in the registry, but it's not in the
/// focused entry's layer. The resolver must reject the override and
/// fall through to the beam-search cascade.
#[test]
fn override_cross_layer_target_falls_through_to_beam_search() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L_window", "L_window", "main", None));
    reg.push_layer(layer(
        "/L_window/L_inspector",
        "L_inspector",
        "main",
        Some("/L_window"),
    ));

    // Cross-layer target — exists, but in a different layer.
    let cross_layer_fq = fq_in_layer("/L_window/L_inspector", "ui:cross_layer");
    reg.register_scope(leaf(
        "ui:cross_layer",
        "/L_window/L_inspector",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
    ));
    // Beam-search candidate in the same layer as `src`.
    let beam_target_fq = fq_in_layer("/L_window", "ui:beam_target");
    reg.register_scope(leaf(
        "ui:beam_target",
        "/L_window",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));

    // Override redirects right → an FQM that lives in the inspector layer.
    let mut overrides = HashMap::new();
    overrides.insert(Direction::Right, Some(cross_layer_fq));
    let src_fq = fq_in_layer("/L_window", "ui:src");
    reg.register_scope(leaf_with_overrides(
        "ui:src",
        "/L_window",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
        overrides,
    ));

    assert_eq!(
        nav(&reg, &src_fq, Direction::Right),
        beam_target_fq,
        "cross-layer override target must be ignored; beam search must run"
    );
}

/// Override target FQM that does not exist anywhere in the
/// registry also falls through to beam search — same "didn't apply"
/// outcome as the cross-layer case.
#[test]
fn override_unknown_target_falls_through_to_beam_search() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));

    let beam_target_fq = fq_in_layer("/L", "ui:beam_target");
    reg.register_scope(leaf(
        "ui:beam_target",
        "/L",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));

    let mut overrides = HashMap::new();
    overrides.insert(
        Direction::Right,
        Some(FullyQualifiedMoniker::from_string("/L/ui:ghost")),
    );
    let src_fq = fq_in_layer("/L", "ui:src");
    reg.register_scope(leaf_with_overrides(
        "ui:src",
        "/L",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
        overrides,
    ));

    assert_eq!(
        nav(&reg, &src_fq, Direction::Right),
        beam_target_fq,
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
    reg.push_layer(layer("/L", "L", "main", None));

    let right_target_fq = fq_in_layer("/L", "ui:right_target");
    reg.register_scope(leaf(
        "ui:right_target",
        "/L",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));
    let src_fq = fq_in_layer("/L", "ui:src");
    reg.register_scope(leaf("ui:src", "/L", None, rect(0.0, 0.0, 50.0, 50.0)));

    assert_eq!(nav(&reg, &src_fq, Direction::Right), right_target_fq);
}

/// A leaf with an override for `Right` but not `Left` runs beam search
/// for `Left` — only the keyed direction is consulted by the resolver.
#[test]
fn override_for_one_direction_does_not_affect_others() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));

    // Two candidates, one to the left and one to the right.
    let left_target_fq = fq_in_layer("/L", "ui:left_target");
    reg.register_scope(leaf(
        "ui:left_target",
        "/L",
        None,
        rect(-100.0, 0.0, 50.0, 50.0),
    ));
    reg.register_scope(leaf(
        "ui:right_target",
        "/L",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));
    // Override target for Right only.
    let right_override_fq = fq_in_layer("/L", "ui:right_override");
    reg.register_scope(leaf(
        "ui:right_override",
        "/L",
        None,
        rect(0.0, 200.0, 50.0, 50.0),
    ));

    let mut overrides = HashMap::new();
    overrides.insert(Direction::Right, Some(right_override_fq.clone()));
    let src_fq = fq_in_layer("/L", "ui:src");
    reg.register_scope(leaf_with_overrides(
        "ui:src",
        "/L",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
        overrides,
    ));

    // Right honors the override.
    assert_eq!(nav(&reg, &src_fq, Direction::Right), right_override_fq);
    // Left is unconsidered by the override; beam search runs.
    assert_eq!(nav(&reg, &src_fq, Direction::Left), left_target_fq);
}
