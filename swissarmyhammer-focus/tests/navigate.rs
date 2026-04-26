//! Integration tests for the Android-style beam-search navigator.
//!
//! Headless pattern matching `tests/focus_state.rs` and
//! `tests/focus_registry.rs` — pure Rust, no Tauri runtime, no jsdom.
//! Every navigation runs through [`BeamNavStrategy`] (the default
//! [`NavStrategy`] impl) and asserts on the returned [`Moniker`].
//!
//! These tests cover the algorithm card (`01KNQXXF5W...`):
//!
//! - **Layer isolation** — nav never crosses a `LayerKey`. Two windows,
//!   two inspectors, a dialog: each is its own layer; nav stays put.
//! - **Rule 1: within-zone beam** — candidates restricted to siblings
//!   sharing `parent_zone` with the focused leaf.
//! - **Rule 2: cross-zone leaf fallback** — when no in-zone candidate
//!   matches the direction, the navigator falls back to all `Focusable`
//!   entries in the same layer.
//! - **Rule 3: no-op** — both rules empty → return `None`.
//! - **Zone-level nav** — when focus is on a `FocusZone`, only sibling
//!   zones are candidates; leaves are invisible.
//! - **Beam scoring** — Android scoring `13 * major² + minor²` prefers
//!   aligned candidates over closer-but-diagonal ones (13:1 ratio).
//! - **Edge commands** — `First`, `Last`, `RowStart`, `RowEnd` scope
//!   their candidate sets by level (leaf → in-zone siblings, zone →
//!   sibling zones).

use std::collections::HashMap;

use swissarmyhammer_focus::{
    BeamNavStrategy, Direction, FocusLayer, FocusZone, Focusable, LayerKey, LayerName, Moniker,
    NavStrategy, Pixels, Rect, SpatialKey, SpatialRegistry, WindowLabel,
};

// ---------------------------------------------------------------------------
// Builders — small helpers that keep test setup readable.
// ---------------------------------------------------------------------------

/// Build a `Rect` from raw `f64` coordinates. Tests construct rects with
/// stable integer-ish coordinates so beam scoring is deterministic.
fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        x: Pixels::new(x),
        y: Pixels::new(y),
        width: Pixels::new(w),
        height: Pixels::new(h),
    }
}

/// Build a `Focusable` with the given identity, rect, layer, and
/// optional parent zone. Overrides are intentionally empty for the
/// algorithm tests — override resolution lives in another card.
fn focusable(
    key: &str,
    moniker: &str,
    layer: &str,
    parent_zone: Option<&str>,
    r: Rect,
) -> Focusable {
    Focusable {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect: r,
        layer_key: LayerKey::from_string(layer),
        parent_zone: parent_zone.map(SpatialKey::from_string),
        overrides: HashMap::new(),
    }
}

/// Build a `FocusZone` with the given identity, rect, layer, and
/// optional parent zone. `last_focused` starts empty and overrides are
/// intentionally empty for the algorithm tests.
fn zone(key: &str, moniker: &str, layer: &str, parent_zone: Option<&str>, r: Rect) -> FocusZone {
    FocusZone {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect: r,
        layer_key: LayerKey::from_string(layer),
        parent_zone: parent_zone.map(SpatialKey::from_string),
        last_focused: None,
        overrides: HashMap::new(),
    }
}

/// Build a `FocusLayer` with the given identity tied to a window.
fn layer(key: &str, window: &str, parent: Option<&str>) -> FocusLayer {
    FocusLayer {
        key: LayerKey::from_string(key),
        name: LayerName::from_string("window"),
        parent: parent.map(LayerKey::from_string),
        window_label: WindowLabel::from_string(window),
        last_focused: None,
    }
}

/// Run the default `BeamNavStrategy` and return the navigated-to
/// `Moniker`. Centralized so test cases read top-to-bottom without
/// repeating the boilerplate.
fn nav(reg: &SpatialRegistry, from: &str, dir: Direction) -> Option<Moniker> {
    BeamNavStrategy::new().next(reg, &SpatialKey::from_string(from), dir)
}

// ---------------------------------------------------------------------------
// Layer isolation — absolute, never crossed.
// ---------------------------------------------------------------------------

/// Two layers stacked on the same window: a `window` root and an
/// `inspector` child. A leaf in the inspector navigating `right` does
/// not see leaves on the window layer, even when their rect would be
/// the rect-wise nearest match.
#[test]
fn nav_never_crosses_layer_boundary_within_one_window() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L_window", "main", None));
    reg.push_layer(layer("L_inspector", "main", Some("L_window")));

    // Card on the window layer — would be rect-wise to the right of the
    // inspector pill, but lives in a different layer.
    reg.register_focusable(focusable(
        "card",
        "ui:card",
        "L_window",
        None,
        rect(200.0, 100.0, 80.0, 40.0),
    ));
    // Pill in the inspector — focused.
    reg.register_focusable(focusable(
        "pill",
        "ui:pill",
        "L_inspector",
        None,
        rect(0.0, 100.0, 50.0, 40.0),
    ));

    // No other inspector leaves to the right → must return None even
    // though `card` is rect-wise the nearest right match.
    assert_eq!(nav(&reg, "pill", Direction::Right), None);
}

/// Two windows, each with its own root layer and identical leaf rect
/// coordinates. Navigating from a leaf in window A must not return a
/// leaf in window B — the layer boundary is the absolute filter.
#[test]
fn nav_never_crosses_layer_boundary_between_windows() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L_a", "win-a", None));
    reg.push_layer(layer("L_b", "win-b", None));

    // Two leaves at the same rect but in different windows. From `a1`'s
    // perspective, `b2` does not exist; the nav has nothing to land on.
    reg.register_focusable(focusable(
        "a1",
        "ui:a1",
        "L_a",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
    ));
    reg.register_focusable(focusable(
        "b2",
        "ui:b2",
        "L_b",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));

    assert_eq!(nav(&reg, "a1", Direction::Right), None);
}

// ---------------------------------------------------------------------------
// Rule 1: within-zone beam search.
// ---------------------------------------------------------------------------

/// Card with two leaves (title above, status below) inside the same
/// zone. `nav.down` from the title returns the status — same
/// `parent_zone`, beam-aligned, closest below.
#[test]
fn rule_1_within_zone_down_picks_sibling_leaf() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "card",
        "ui:card",
        "L",
        None,
        rect(0.0, 0.0, 200.0, 100.0),
    ));
    reg.register_focusable(focusable(
        "title",
        "ui:title",
        "L",
        Some("card"),
        rect(10.0, 10.0, 180.0, 30.0),
    ));
    reg.register_focusable(focusable(
        "status",
        "ui:status",
        "L",
        Some("card"),
        rect(10.0, 60.0, 180.0, 30.0),
    ));

    assert_eq!(
        nav(&reg, "title", Direction::Down),
        Some(Moniker::from_string("ui:status"))
    );
}

/// Inverse of `rule_1_within_zone_down_picks_sibling_leaf`. From the
/// status leaf at the bottom of the card, `nav.up` walks back to the
/// title — guards against a sign flip in the `Direction::Up` arm of
/// `score_candidate`.
#[test]
fn rule_1_within_zone_up_picks_sibling_leaf() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "card",
        "ui:card",
        "L",
        None,
        rect(0.0, 0.0, 200.0, 100.0),
    ));
    reg.register_focusable(focusable(
        "title",
        "ui:title",
        "L",
        Some("card"),
        rect(10.0, 10.0, 180.0, 30.0),
    ));
    reg.register_focusable(focusable(
        "status",
        "ui:status",
        "L",
        Some("card"),
        rect(10.0, 60.0, 180.0, 30.0),
    ));

    assert_eq!(
        nav(&reg, "status", Direction::Up),
        Some(Moniker::from_string("ui:title"))
    );
}

/// Aligned candidate beats closer-but-diagonal candidate. The in-beam
/// tier wins regardless of raw score: a candidate whose rect overlaps
/// the source's cross-axis projection always beats a non-overlapping
/// candidate, even when the diagonal one would score lower under the
/// raw `13 * major² + minor²` formula. See the assertion comment for
/// the worked-out numbers.
#[test]
fn rule_1_aligned_candidate_beats_closer_diagonal() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "card",
        "ui:card",
        "L",
        None,
        rect(0.0, 0.0, 400.0, 200.0),
    ));
    // Focused: small box at top-left.
    reg.register_focusable(focusable(
        "src",
        "ui:src",
        "L",
        Some("card"),
        rect(0.0, 0.0, 20.0, 20.0),
    ));
    // Aligned: directly below `src`, far away.
    reg.register_focusable(focusable(
        "aligned",
        "ui:aligned",
        "L",
        Some("card"),
        rect(0.0, 100.0, 20.0, 20.0),
    ));
    // Diagonal: closer in raw distance but offset to the right.
    reg.register_focusable(focusable(
        "diagonal",
        "ui:diagonal",
        "L",
        Some("card"),
        rect(50.0, 30.0, 20.0, 20.0),
    ));

    // The in-beam tier wins regardless of raw score. `aligned`
    // overlaps the source rect's vertical projection (x: 0..20 vs
    // 0..20), so it is in-beam. `diagonal` (x: 50..70) does not
    // overlap, so it is out-of-beam.
    //
    // For reference, the raw Android scores would actually favor the
    // diagonal candidate:
    //   score(aligned)  = 13 * 80²  + 0²   = 83_200
    //   score(diagonal) = 13 * 10²  + 50²  = 1_300 + 2_500 = 3_800
    // But `pick_best_candidate` applies the beam test as a hard tier
    // strictly above raw score: an in-beam candidate beats every
    // out-of-beam candidate, so `aligned` wins despite the worse raw
    // score. This is Android FocusFinder's beam preference.
    assert_eq!(
        nav(&reg, "src", Direction::Down),
        Some(Moniker::from_string("ui:aligned"))
    );
}

// ---------------------------------------------------------------------------
// Rule 2: cross-zone leaf fallback.
// ---------------------------------------------------------------------------

/// Two cards laid out side-by-side, each with a single leaf. From the
/// left card's leaf, `nav.right` has no in-zone candidate → falls back
/// to the leaf in the right card (rule 2).
#[test]
fn rule_2_cross_zone_right_falls_back_to_leaf_in_neighbor_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "col0",
        "ui:col0",
        "L",
        None,
        rect(0.0, 0.0, 100.0, 200.0),
    ));
    reg.register_zone(zone(
        "col1",
        "ui:col1",
        "L",
        None,
        rect(100.0, 0.0, 100.0, 200.0),
    ));
    reg.register_focusable(focusable(
        "leaf0",
        "ui:leaf0",
        "L",
        Some("col0"),
        rect(10.0, 10.0, 80.0, 40.0),
    ));
    reg.register_focusable(focusable(
        "leaf1",
        "ui:leaf1",
        "L",
        Some("col1"),
        rect(110.0, 10.0, 80.0, 40.0),
    ));

    assert_eq!(
        nav(&reg, "leaf0", Direction::Right),
        Some(Moniker::from_string("ui:leaf1"))
    );
}

/// Mirror of `rule_2_cross_zone_right_falls_back_to_leaf_in_neighbor_zone`
/// for `nav.left`. From the right column's leaf, `nav.left` falls back
/// to the leaf in the left column via rule 2 — guards against a sign
/// flip in the `Direction::Left` arm of `score_candidate`.
#[test]
fn rule_2_cross_zone_left_falls_back_to_leaf_in_neighbor_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "col0",
        "ui:col0",
        "L",
        None,
        rect(0.0, 0.0, 100.0, 200.0),
    ));
    reg.register_zone(zone(
        "col1",
        "ui:col1",
        "L",
        None,
        rect(100.0, 0.0, 100.0, 200.0),
    ));
    reg.register_focusable(focusable(
        "leaf0",
        "ui:leaf0",
        "L",
        Some("col0"),
        rect(10.0, 10.0, 80.0, 40.0),
    ));
    reg.register_focusable(focusable(
        "leaf1",
        "ui:leaf1",
        "L",
        Some("col1"),
        rect(110.0, 10.0, 80.0, 40.0),
    ));

    assert_eq!(
        nav(&reg, "leaf1", Direction::Left),
        Some(Moniker::from_string("ui:leaf0"))
    );
}

/// In-zone candidate is preferred over a closer cross-zone candidate.
/// Rule 1 fires first; rule 2 only runs when rule 1 finds nothing.
#[test]
fn rule_1_preferred_over_rule_2_when_in_zone_match_exists() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "col0",
        "ui:col0",
        "L",
        None,
        rect(0.0, 0.0, 100.0, 200.0),
    ));
    reg.register_zone(zone(
        "col1",
        "ui:col1",
        "L",
        None,
        rect(100.0, 0.0, 100.0, 200.0),
    ));
    reg.register_focusable(focusable(
        "src",
        "ui:src",
        "L",
        Some("col0"),
        rect(10.0, 10.0, 30.0, 30.0),
    ));
    // In-zone but far below.
    reg.register_focusable(focusable(
        "inzone",
        "ui:inzone",
        "L",
        Some("col0"),
        rect(10.0, 150.0, 30.0, 30.0),
    ));
    // Cross-zone but closer (raw rect distance is smaller).
    reg.register_focusable(focusable(
        "crosszone",
        "ui:crosszone",
        "L",
        Some("col1"),
        rect(110.0, 50.0, 30.0, 30.0),
    ));

    assert_eq!(
        nav(&reg, "src", Direction::Down),
        Some(Moniker::from_string("ui:inzone")),
        "rule 1 (in-zone) must win even when rule 2 candidate is closer"
    );
}

// ---------------------------------------------------------------------------
// Rule 3: no-op.
// ---------------------------------------------------------------------------

/// The only leaf in a layer has nothing to navigate to → `None`. This
/// is the rule-3 termination of the three-rule cascade.
#[test]
fn rule_3_no_candidate_returns_none() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_focusable(focusable(
        "lonely",
        "ui:lonely",
        "L",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
    ));

    assert_eq!(nav(&reg, "lonely", Direction::Down), None);
    assert_eq!(nav(&reg, "lonely", Direction::Up), None);
    assert_eq!(nav(&reg, "lonely", Direction::Left), None);
    assert_eq!(nav(&reg, "lonely", Direction::Right), None);
}

// ---------------------------------------------------------------------------
// Zone-level navigation.
// ---------------------------------------------------------------------------

/// Focused on a column zone, `nav.right` walks to the next sibling
/// column zone — leaves inside any column are invisible at this level.
#[test]
fn zone_nav_right_picks_sibling_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "col0",
        "ui:col0",
        "L",
        None,
        rect(0.0, 0.0, 100.0, 200.0),
    ));
    reg.register_zone(zone(
        "col1",
        "ui:col1",
        "L",
        None,
        rect(100.0, 0.0, 100.0, 200.0),
    ));
    reg.register_zone(zone(
        "col2",
        "ui:col2",
        "L",
        None,
        rect(200.0, 0.0, 100.0, 200.0),
    ));

    assert_eq!(
        nav(&reg, "col0", Direction::Right),
        Some(Moniker::from_string("ui:col1"))
    );
}

/// Three columns laid out horizontally — `nav.up` from a column zone
/// has no sibling zone vertically, so it returns `None`. Leaves
/// inside the columns are invisible at zone level.
#[test]
fn zone_nav_up_with_only_horizontal_siblings_returns_none() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "col0",
        "ui:col0",
        "L",
        None,
        rect(0.0, 0.0, 100.0, 200.0),
    ));
    reg.register_zone(zone(
        "col1",
        "ui:col1",
        "L",
        None,
        rect(100.0, 0.0, 100.0, 200.0),
    ));
    // Leaf inside col0 — would be rect-wise above col0 if zones saw
    // leaves, but at zone level it is invisible.
    reg.register_focusable(focusable(
        "leaf",
        "ui:leaf",
        "L",
        Some("col0"),
        rect(10.0, -50.0, 30.0, 30.0),
    ));

    assert_eq!(nav(&reg, "col0", Direction::Up), None);
}

/// `nav.right` from a zone never returns a leaf, even if a leaf inside
/// the next column happens to be the rect-wise nearest match.
#[test]
fn zone_nav_right_does_not_return_leaf_inside_neighbor_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "col0",
        "ui:col0",
        "L",
        None,
        rect(0.0, 0.0, 100.0, 200.0),
    ));
    reg.register_zone(zone(
        "col1",
        "ui:col1",
        "L",
        None,
        rect(200.0, 0.0, 100.0, 200.0),
    ));
    // Leaf in col1 sits closer to col0 than col1's left edge.
    reg.register_focusable(focusable(
        "leaf1",
        "ui:leaf1",
        "L",
        Some("col1"),
        rect(110.0, 10.0, 30.0, 30.0),
    ));

    let target = nav(&reg, "col0", Direction::Right).expect("zone nav must reach col1");
    assert_eq!(
        target,
        Moniker::from_string("ui:col1"),
        "zone nav must land on the sibling zone, never a leaf"
    );
}

// ---------------------------------------------------------------------------
// Inspector layer scenarios.
// ---------------------------------------------------------------------------

/// Two pills side-by-side in an inspector group — `nav.right` picks the
/// next pill (rule 1).
#[test]
fn inspector_pill_a_to_pill_b_in_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "group",
        "ui:group",
        "L",
        None,
        rect(0.0, 0.0, 200.0, 50.0),
    ));
    reg.register_focusable(focusable(
        "pill_a",
        "ui:pill_a",
        "L",
        Some("group"),
        rect(0.0, 0.0, 50.0, 30.0),
    ));
    reg.register_focusable(focusable(
        "pill_b",
        "ui:pill_b",
        "L",
        Some("group"),
        rect(100.0, 0.0, 50.0, 30.0),
    ));

    assert_eq!(
        nav(&reg, "pill_a", Direction::Right),
        Some(Moniker::from_string("ui:pill_b"))
    );
}

/// Two field-row leaves in different rows, no enclosing zone — `nav.down`
/// finds the next row's leaf via rule 2.
#[test]
fn inspector_label_1_to_label_2_via_cross_zone_fallback() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    // Each label sits in its own zone (one zone per row), so rule 1
    // finds nothing and rule 2 walks across.
    reg.register_zone(zone(
        "row1",
        "ui:row1",
        "L",
        None,
        rect(0.0, 0.0, 200.0, 50.0),
    ));
    reg.register_zone(zone(
        "row2",
        "ui:row2",
        "L",
        None,
        rect(0.0, 50.0, 200.0, 50.0),
    ));
    reg.register_focusable(focusable(
        "label_1",
        "ui:label_1",
        "L",
        Some("row1"),
        rect(0.0, 0.0, 50.0, 30.0),
    ));
    reg.register_focusable(focusable(
        "label_2",
        "ui:label_2",
        "L",
        Some("row2"),
        rect(0.0, 50.0, 50.0, 30.0),
    ));

    assert_eq!(
        nav(&reg, "label_1", Direction::Down),
        Some(Moniker::from_string("ui:label_2"))
    );
}

/// The last leaf in the layer with `nav.down` returns `None` — there's
/// no leaf below, and nav never escapes the layer.
#[test]
fn inspector_last_leaf_down_returns_none() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_focusable(focusable(
        "label_1",
        "ui:label_1",
        "L",
        None,
        rect(0.0, 0.0, 50.0, 30.0),
    ));
    reg.register_focusable(focusable(
        "label_2",
        "ui:label_2",
        "L",
        None,
        rect(0.0, 50.0, 50.0, 30.0),
    ));

    assert_eq!(nav(&reg, "label_2", Direction::Down), None);
}

// ---------------------------------------------------------------------------
// Realistic board scenario.
// ---------------------------------------------------------------------------

/// 3 columns × 2 cards, each card a Zone with title + status leaves.
/// Verifies the navigator handles a realistic layout: in-card nav,
/// cross-card nav, cross-column nav, all within one layer.
#[test]
fn realistic_board_nav_walks_through_cards() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));

    // Three columns laid out horizontally.
    for (i, col) in ["col0", "col1", "col2"].iter().enumerate() {
        reg.register_zone(zone(
            col,
            &format!("ui:{col}"),
            "L",
            None,
            rect(i as f64 * 100.0, 0.0, 100.0, 400.0),
        ));
        // Two cards per column.
        for (j, card) in ["card_a", "card_b"].iter().enumerate() {
            let card_key = format!("{col}_{card}");
            reg.register_zone(zone(
                &card_key,
                &format!("ui:{card_key}"),
                "L",
                Some(col),
                rect(i as f64 * 100.0 + 5.0, j as f64 * 80.0 + 10.0, 90.0, 70.0),
            ));
            // Title leaf (top half).
            reg.register_focusable(focusable(
                &format!("{card_key}_title"),
                &format!("ui:{card_key}_title"),
                "L",
                Some(&card_key),
                rect(i as f64 * 100.0 + 10.0, j as f64 * 80.0 + 15.0, 80.0, 25.0),
            ));
            // Status leaf (bottom half).
            reg.register_focusable(focusable(
                &format!("{card_key}_status"),
                &format!("ui:{card_key}_status"),
                "L",
                Some(&card_key),
                rect(i as f64 * 100.0 + 10.0, j as f64 * 80.0 + 45.0, 80.0, 25.0),
            ));
        }
    }

    // Within a card: title → status (rule 1, same zone).
    assert_eq!(
        nav(&reg, "col0_card_a_title", Direction::Down),
        Some(Moniker::from_string("ui:col0_card_a_status"))
    );

    // Status of card A → title of card B (rule 2 — different zones,
    // same layer).
    assert_eq!(
        nav(&reg, "col0_card_a_status", Direction::Down),
        Some(Moniker::from_string("ui:col0_card_b_title"))
    );

    // Title of col0 card_a → title of col1 card_a (rule 2 — across
    // columns).
    assert_eq!(
        nav(&reg, "col0_card_a_title", Direction::Right),
        Some(Moniker::from_string("ui:col1_card_a_title"))
    );
}

// ---------------------------------------------------------------------------
// Edge commands — First, Last, RowStart, RowEnd.
// ---------------------------------------------------------------------------

/// `Direction::First` from a leaf scopes to the leaf's `parent_zone`
/// siblings — picks the topmost-leftmost in-zone sibling.
#[test]
fn edge_first_for_leaf_scopes_to_parent_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "card",
        "ui:card",
        "L",
        None,
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    // Three leaves laid out vertically.
    reg.register_focusable(focusable(
        "title",
        "ui:title",
        "L",
        Some("card"),
        rect(10.0, 10.0, 180.0, 30.0),
    ));
    reg.register_focusable(focusable(
        "body",
        "ui:body",
        "L",
        Some("card"),
        rect(10.0, 60.0, 180.0, 30.0),
    ));
    reg.register_focusable(focusable(
        "status",
        "ui:status",
        "L",
        Some("card"),
        rect(10.0, 110.0, 180.0, 30.0),
    ));

    // `First` from anywhere in the card lands on the topmost-leftmost
    // leaf (`title`).
    assert_eq!(
        nav(&reg, "status", Direction::First),
        Some(Moniker::from_string("ui:title"))
    );
}

/// `Direction::Last` from a leaf scopes to the leaf's `parent_zone`
/// siblings — picks the bottommost-rightmost in-zone sibling.
#[test]
fn edge_last_for_leaf_scopes_to_parent_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "card",
        "ui:card",
        "L",
        None,
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    reg.register_focusable(focusable(
        "title",
        "ui:title",
        "L",
        Some("card"),
        rect(10.0, 10.0, 180.0, 30.0),
    ));
    reg.register_focusable(focusable(
        "status",
        "ui:status",
        "L",
        Some("card"),
        rect(10.0, 110.0, 180.0, 30.0),
    ));

    assert_eq!(
        nav(&reg, "title", Direction::Last),
        Some(Moniker::from_string("ui:status"))
    );
}

/// `Direction::First` from a zone scopes to sibling zones — picks the
/// topmost-leftmost sibling zone.
#[test]
fn edge_first_for_zone_scopes_to_sibling_zones() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "col0",
        "ui:col0",
        "L",
        None,
        rect(0.0, 0.0, 100.0, 200.0),
    ));
    reg.register_zone(zone(
        "col1",
        "ui:col1",
        "L",
        None,
        rect(100.0, 0.0, 100.0, 200.0),
    ));
    reg.register_zone(zone(
        "col2",
        "ui:col2",
        "L",
        None,
        rect(200.0, 0.0, 100.0, 200.0),
    ));

    assert_eq!(
        nav(&reg, "col2", Direction::First),
        Some(Moniker::from_string("ui:col0"))
    );
}

/// `Direction::RowStart` from a leaf moves to the leftmost in-zone
/// sibling whose vertical extent overlaps the focused leaf — i.e. the
/// start of the focused row.
#[test]
fn edge_row_start_picks_leftmost_in_row_sibling() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "row",
        "ui:row",
        "L",
        None,
        rect(0.0, 0.0, 300.0, 50.0),
    ));
    // Three leaves on the same row.
    reg.register_focusable(focusable(
        "left",
        "ui:left",
        "L",
        Some("row"),
        rect(0.0, 10.0, 50.0, 30.0),
    ));
    reg.register_focusable(focusable(
        "middle",
        "ui:middle",
        "L",
        Some("row"),
        rect(100.0, 10.0, 50.0, 30.0),
    ));
    reg.register_focusable(focusable(
        "right",
        "ui:right",
        "L",
        Some("row"),
        rect(200.0, 10.0, 50.0, 30.0),
    ));

    assert_eq!(
        nav(&reg, "right", Direction::RowStart),
        Some(Moniker::from_string("ui:left"))
    );
}

/// `Direction::RowEnd` from a leaf moves to the rightmost in-zone
/// sibling whose vertical extent overlaps the focused leaf.
#[test]
fn edge_row_end_picks_rightmost_in_row_sibling() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "row",
        "ui:row",
        "L",
        None,
        rect(0.0, 0.0, 300.0, 50.0),
    ));
    reg.register_focusable(focusable(
        "left",
        "ui:left",
        "L",
        Some("row"),
        rect(0.0, 10.0, 50.0, 30.0),
    ));
    reg.register_focusable(focusable(
        "right",
        "ui:right",
        "L",
        Some("row"),
        rect(200.0, 10.0, 50.0, 30.0),
    ));

    assert_eq!(
        nav(&reg, "left", Direction::RowEnd),
        Some(Moniker::from_string("ui:right"))
    );
}

/// `Direction::First` from the topmost-leftmost leaf returns that
/// leaf's own moniker. The resolver in
/// [`swissarmyhammer_focus::SpatialState::focus`] short-circuits via
/// the "already focused → no event" check, so the user-visible result
/// is a no-op. Adding the focused leaf to the candidate set (rather
/// than excluding it) keeps `Home` semantics intuitive: pressing
/// `Home` while at the start of a list does nothing instead of
/// jumping to the second element.
#[test]
fn edge_first_at_boundary_returns_focused_self() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "card",
        "ui:card",
        "L",
        None,
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    reg.register_focusable(focusable(
        "title",
        "ui:title",
        "L",
        Some("card"),
        rect(10.0, 10.0, 180.0, 30.0),
    ));
    reg.register_focusable(focusable(
        "status",
        "ui:status",
        "L",
        Some("card"),
        rect(10.0, 60.0, 180.0, 30.0),
    ));

    // Already at the boundary: the strategy returns the focused
    // moniker, and `state.focus()` will no-op the redundant move.
    assert_eq!(
        nav(&reg, "title", Direction::First),
        Some(Moniker::from_string("ui:title"))
    );
}

/// `Direction::Last` from the bottommost-rightmost leaf returns that
/// leaf's own moniker — mirrors `edge_first_at_boundary_returns_focused_self`.
#[test]
fn edge_last_at_boundary_returns_focused_self() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "card",
        "ui:card",
        "L",
        None,
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    reg.register_focusable(focusable(
        "title",
        "ui:title",
        "L",
        Some("card"),
        rect(10.0, 10.0, 180.0, 30.0),
    ));
    reg.register_focusable(focusable(
        "status",
        "ui:status",
        "L",
        Some("card"),
        rect(10.0, 60.0, 180.0, 30.0),
    ));

    assert_eq!(
        nav(&reg, "status", Direction::Last),
        Some(Moniker::from_string("ui:status"))
    );
}

/// `Direction::RowStart` from the leftmost-on-row leaf returns that
/// leaf's own moniker — guards the same boundary semantics as
/// `edge_first_at_boundary_returns_focused_self` for the row commands.
#[test]
fn edge_row_start_at_boundary_returns_focused_self() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "row",
        "ui:row",
        "L",
        None,
        rect(0.0, 0.0, 300.0, 50.0),
    ));
    reg.register_focusable(focusable(
        "left",
        "ui:left",
        "L",
        Some("row"),
        rect(0.0, 10.0, 50.0, 30.0),
    ));
    reg.register_focusable(focusable(
        "right",
        "ui:right",
        "L",
        Some("row"),
        rect(200.0, 10.0, 50.0, 30.0),
    ));

    assert_eq!(
        nav(&reg, "left", Direction::RowStart),
        Some(Moniker::from_string("ui:left"))
    );
}

// ---------------------------------------------------------------------------
// Layer-boundary stress: dialog → inspector → window forest.
// ---------------------------------------------------------------------------

/// Dialog stacked on inspector stacked on window. A leaf in the dialog
/// sees only dialog leaves — never inspector or window leaves, even
/// though they'd be rect-wise close.
#[test]
fn layer_stress_dialog_focused_sees_only_dialog_entries() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L_window", "main", None));
    reg.push_layer(layer("L_inspector", "main", Some("L_window")));
    reg.push_layer(layer("L_dialog", "main", Some("L_inspector")));

    // Window leaf — would be rect-wise to the right of the dialog leaf.
    reg.register_focusable(focusable(
        "window_card",
        "ui:window_card",
        "L_window",
        None,
        rect(500.0, 100.0, 50.0, 50.0),
    ));
    // Inspector leaf — would be rect-wise above the dialog leaf.
    reg.register_focusable(focusable(
        "inspector_pill",
        "ui:inspector_pill",
        "L_inspector",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));
    // Dialog leaves — focused on the first; only the second is visible.
    reg.register_focusable(focusable(
        "dlg_btn_a",
        "ui:dlg_btn_a",
        "L_dialog",
        None,
        rect(100.0, 100.0, 50.0, 50.0),
    ));
    reg.register_focusable(focusable(
        "dlg_btn_b",
        "ui:dlg_btn_b",
        "L_dialog",
        None,
        rect(200.0, 100.0, 50.0, 50.0),
    ));

    // From the dialog, `right` goes to dlg_btn_b (in-layer), not
    // window_card (different layer).
    assert_eq!(
        nav(&reg, "dlg_btn_a", Direction::Right),
        Some(Moniker::from_string("ui:dlg_btn_b"))
    );
    // From the dialog, `up` finds nothing — the only `up` rect-wise
    // candidate is `inspector_pill`, but it's a different layer.
    assert_eq!(nav(&reg, "dlg_btn_a", Direction::Up), None);
}

// ---------------------------------------------------------------------------
// Unknown-key contracts.
// ---------------------------------------------------------------------------

/// An unknown starting key returns `None` — no panic, no synthesized
/// candidate. Mirrors the contract of `SpatialState::navigate` for
/// stale keys arriving over IPC.
#[test]
fn unknown_starting_key_returns_none() {
    let reg = SpatialRegistry::new();
    assert_eq!(nav(&reg, "ghost", Direction::Right), None);
}
