//! Integration tests for the Android-style beam-search navigator
//! under the **unified cascade** policy.
//!
//! Headless pattern matching `tests/focus_state.rs` and
//! `tests/focus_registry.rs` — pure Rust, no Tauri runtime, no jsdom.
//! Every navigation runs through [`BeamNavStrategy`] (the default
//! [`NavStrategy`] impl) and asserts on the returned
//! [`FullyQualifiedMoniker`].
//!
//! Migrated from the pre-path-monikers identifier model: every place
//! the suite previously addressed scopes by `SpatialKey` plus a flat
//! `Moniker` now uses a [`FullyQualifiedMoniker`] alone. The path
//! through the focus hierarchy IS the spatial key. Tests construct
//! FQMs via [`fq_in_layer`] / [`fq_in_zone`] helpers so the path
//! shape stays consistent with how the React side composes them
//! through `FullyQualifiedMonikerContext`.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    BeamNavStrategy, Direction, FocusLayer, FocusScope, FocusZone, FullyQualifiedMoniker,
    LayerName, NavStrategy, Pixels, Rect, SegmentMoniker, SpatialRegistry, WindowLabel,
};

// ---------------------------------------------------------------------------
// Builders — small helpers that keep test setup readable.
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

/// FQM for a primitive registered inside a parent zone (`parent_fq`).
fn fq_in_zone(parent_fq: &FullyQualifiedMoniker, segment: &str) -> FullyQualifiedMoniker {
    FullyQualifiedMoniker::compose(parent_fq, &SegmentMoniker::from_string(segment))
}

/// Build a `FocusScope` leaf with the given identity, rect, layer, and
/// optional parent zone. Overrides are intentionally empty.
fn leaf(
    fq: FullyQualifiedMoniker,
    segment: &str,
    layer_fq: &str,
    parent_zone: Option<FullyQualifiedMoniker>,
    r: Rect,
) -> FocusScope {
    FocusScope {
        fq,
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: FullyQualifiedMoniker::from_string(layer_fq),
        parent_zone,
        overrides: HashMap::new(),
    }
}

/// Build a `FocusZone` with the given identity, rect, layer, and
/// optional parent zone.
fn zone(
    fq: FullyQualifiedMoniker,
    segment: &str,
    layer_fq: &str,
    parent_zone: Option<FullyQualifiedMoniker>,
    r: Rect,
) -> FocusZone {
    FocusZone {
        fq,
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: FullyQualifiedMoniker::from_string(layer_fq),
        parent_zone,
        last_focused: None,
        overrides: HashMap::new(),
    }
}

/// Build a `FocusLayer` with the given identity tied to a window.
fn layer(fq_str: &str, segment: &str, window: &str, parent: Option<&str>) -> FocusLayer {
    FocusLayer {
        fq: FullyQualifiedMoniker::from_string(fq_str),
        segment: SegmentMoniker::from_string(segment),
        name: LayerName::from_string("window"),
        parent: parent.map(FullyQualifiedMoniker::from_string),
        window_label: WindowLabel::from_string(window),
        last_focused: None,
    }
}

/// Run the default `BeamNavStrategy` and return the navigated-to FQM.
/// Resolves the focused entry's segment from the registry — under the
/// no-silent-dropout contract every nav call needs the focused segment
/// alongside the focused FQM. For unknown `from`, falls back to a
/// synthetic segment matching the leaf segment of the FQM so the test
/// can still exercise the torn-state path.
fn nav(
    reg: &SpatialRegistry,
    from: &FullyQualifiedMoniker,
    dir: Direction,
) -> FullyQualifiedMoniker {
    let focused_segment = reg
        .find_by_fq(from)
        .map(|e| e.segment().clone())
        .unwrap_or_else(|| {
            let s = from.as_str().rsplit('/').next().unwrap_or("");
            SegmentMoniker::from_string(s)
        });
    BeamNavStrategy::new().next(reg, from, &focused_segment, dir)
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
    reg.push_layer(layer("/win", "window", "main", None));
    reg.push_layer(layer("/win/inspector", "inspector", "main", Some("/win")));

    let card_fq = fq_in_layer("/win", "card");
    reg.register_scope(leaf(
        card_fq.clone(),
        "card",
        "/win",
        None,
        rect(200.0, 100.0, 80.0, 40.0),
    ));
    let pill_fq = fq_in_layer("/win/inspector", "pill");
    reg.register_scope(leaf(
        pill_fq.clone(),
        "pill",
        "/win/inspector",
        None,
        rect(0.0, 100.0, 50.0, 40.0),
    ));

    assert_eq!(nav(&reg, &pill_fq, Direction::Right), pill_fq);
}

/// Two windows, each with its own root layer and identical leaf rect
/// coordinates. Navigating from a leaf in window A must not return a
/// leaf in window B — the layer boundary is the absolute filter.
#[test]
fn nav_never_crosses_layer_boundary_between_windows() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/win-a", "win-a", "win-a", None));
    reg.push_layer(layer("/win-b", "win-b", "win-b", None));

    let a1_fq = fq_in_layer("/win-a", "a1");
    reg.register_scope(leaf(
        a1_fq.clone(),
        "a1",
        "/win-a",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
    ));
    reg.register_scope(leaf(
        fq_in_layer("/win-b", "b2"),
        "b2",
        "/win-b",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));

    assert_eq!(nav(&reg, &a1_fq, Direction::Right), a1_fq);
}

// ---------------------------------------------------------------------------
// Iter 0 — in-zone peer search.
// ---------------------------------------------------------------------------

/// Card with two leaves (title above, status below) inside the same
/// zone. `nav.down` from the title returns the status — same
/// `parent_zone`, beam-aligned, closest below.
#[test]
fn rule_1_within_zone_down_picks_sibling_leaf() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let card_fq = fq_in_layer("/L", "card");
    reg.register_zone(zone(
        card_fq.clone(),
        "card",
        "/L",
        None,
        rect(0.0, 0.0, 200.0, 100.0),
    ));
    let title_fq = fq_in_zone(&card_fq, "title");
    let status_fq = fq_in_zone(&card_fq, "status");
    reg.register_scope(leaf(
        title_fq.clone(),
        "title",
        "/L",
        Some(card_fq.clone()),
        rect(10.0, 10.0, 180.0, 30.0),
    ));
    reg.register_scope(leaf(
        status_fq.clone(),
        "status",
        "/L",
        Some(card_fq),
        rect(10.0, 60.0, 180.0, 30.0),
    ));

    assert_eq!(nav(&reg, &title_fq, Direction::Down), status_fq);
}

/// Inverse of `rule_1_within_zone_down_picks_sibling_leaf`. From the
/// status leaf at the bottom of the card, `nav.up` walks back to the
/// title.
#[test]
fn rule_1_within_zone_up_picks_sibling_leaf() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let card_fq = fq_in_layer("/L", "card");
    reg.register_zone(zone(
        card_fq.clone(),
        "card",
        "/L",
        None,
        rect(0.0, 0.0, 200.0, 100.0),
    ));
    let title_fq = fq_in_zone(&card_fq, "title");
    let status_fq = fq_in_zone(&card_fq, "status");
    reg.register_scope(leaf(
        title_fq.clone(),
        "title",
        "/L",
        Some(card_fq.clone()),
        rect(10.0, 10.0, 180.0, 30.0),
    ));
    reg.register_scope(leaf(
        status_fq.clone(),
        "status",
        "/L",
        Some(card_fq),
        rect(10.0, 60.0, 180.0, 30.0),
    ));

    assert_eq!(nav(&reg, &status_fq, Direction::Up), title_fq);
}

/// Aligned candidate beats closer-but-diagonal candidate. The in-beam
/// test is a hard filter for cardinal directions.
#[test]
fn rule_1_aligned_candidate_beats_closer_diagonal() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let card_fq = fq_in_layer("/L", "card");
    reg.register_zone(zone(
        card_fq.clone(),
        "card",
        "/L",
        None,
        rect(0.0, 0.0, 400.0, 200.0),
    ));
    let src_fq = fq_in_zone(&card_fq, "src");
    let aligned_fq = fq_in_zone(&card_fq, "aligned");
    reg.register_scope(leaf(
        src_fq.clone(),
        "src",
        "/L",
        Some(card_fq.clone()),
        rect(0.0, 0.0, 20.0, 20.0),
    ));
    reg.register_scope(leaf(
        aligned_fq.clone(),
        "aligned",
        "/L",
        Some(card_fq.clone()),
        rect(0.0, 100.0, 20.0, 20.0),
    ));
    reg.register_scope(leaf(
        fq_in_zone(&card_fq, "diagonal"),
        "diagonal",
        "/L",
        Some(card_fq),
        rect(50.0, 30.0, 20.0, 20.0),
    ));

    assert_eq!(nav(&reg, &src_fq, Direction::Down), aligned_fq);
}

// ---------------------------------------------------------------------------
// Cross-zone navigation under the unified cascade.
// ---------------------------------------------------------------------------

/// Two columns laid out side-by-side, each with a single leaf inside.
/// From the left column's leaf, `nav.right` finds no in-zone peer; the
/// cascade escalates to the column zone, finds the right column zone
/// as a peer at the parent's level, and drills into its natural child
/// for `Right` (the leftmost child, here `leaf1`).
#[test]
fn cross_zone_right_drills_into_next_column_leftmost_leaf() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let col0_fq = fq_in_layer("/L", "col0");
    let col1_fq = fq_in_layer("/L", "col1");
    reg.register_zone(zone(
        col0_fq.clone(),
        "col0",
        "/L",
        None,
        rect(0.0, 0.0, 100.0, 200.0),
    ));
    reg.register_zone(zone(
        col1_fq.clone(),
        "col1",
        "/L",
        None,
        rect(100.0, 0.0, 100.0, 200.0),
    ));
    let leaf0_fq = fq_in_zone(&col0_fq, "leaf0");
    reg.register_scope(leaf(
        leaf0_fq.clone(),
        "leaf0",
        "/L",
        Some(col0_fq),
        rect(10.0, 10.0, 80.0, 40.0),
    ));
    let leaf1_fq = fq_in_zone(&col1_fq, "leaf1");
    reg.register_scope(leaf(
        leaf1_fq.clone(),
        "leaf1",
        "/L",
        Some(col1_fq),
        rect(110.0, 10.0, 80.0, 40.0),
    ));

    assert_eq!(nav(&reg, &leaf0_fq, Direction::Right), leaf1_fq);
}

/// Mirror of `cross_zone_right_drills_into_next_column_leftmost_leaf`
/// for `nav.left` — drilling into the previous column resolves to its
/// rightmost child, here `leaf0`.
#[test]
fn cross_zone_left_drills_into_previous_column_rightmost_leaf() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let col0_fq = fq_in_layer("/L", "col0");
    let col1_fq = fq_in_layer("/L", "col1");
    reg.register_zone(zone(
        col0_fq.clone(),
        "col0",
        "/L",
        None,
        rect(0.0, 0.0, 100.0, 200.0),
    ));
    reg.register_zone(zone(
        col1_fq.clone(),
        "col1",
        "/L",
        None,
        rect(100.0, 0.0, 100.0, 200.0),
    ));
    let leaf0_fq = fq_in_zone(&col0_fq, "leaf0");
    reg.register_scope(leaf(
        leaf0_fq.clone(),
        "leaf0",
        "/L",
        Some(col0_fq),
        rect(10.0, 10.0, 80.0, 40.0),
    ));
    let leaf1_fq = fq_in_zone(&col1_fq, "leaf1");
    reg.register_scope(leaf(
        leaf1_fq.clone(),
        "leaf1",
        "/L",
        Some(col1_fq),
        rect(110.0, 10.0, 80.0, 40.0),
    ));

    assert_eq!(nav(&reg, &leaf1_fq, Direction::Left), leaf0_fq);
}

/// Production-shape regression: a board with two columns, three card
/// leaves per column, and a column-name leaf in each column header.
///
/// Cross-zone navigation drills into the destination column's natural
/// child for the search direction — for `Right`, the leftmost child
/// of column B (with topmost as tie-break, the column-name leaf); for
/// `Left`, the rightmost child of column A (same tie-break shape).
#[test]
fn cross_zone_realistic_board_right_from_card_in_a_drills_into_column_b_name() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));

    let board_fq = fq_in_layer("/L", "board");
    reg.register_zone(zone(
        board_fq.clone(),
        "board",
        "/L",
        None,
        rect(0.0, 0.0, 600.0, 400.0),
    ));

    let col_a_fq = fq_in_zone(&board_fq, "column:A");
    let col_b_fq = fq_in_zone(&board_fq, "column:B");
    reg.register_zone(zone(
        col_a_fq.clone(),
        "column:A",
        "/L",
        Some(board_fq.clone()),
        rect(0.0, 0.0, 300.0, 400.0),
    ));
    reg.register_zone(zone(
        col_b_fq.clone(),
        "column:B",
        "/L",
        Some(board_fq),
        rect(300.0, 0.0, 300.0, 400.0),
    ));

    reg.register_scope(leaf(
        fq_in_zone(&col_a_fq, "column:A.name"),
        "column:A.name",
        "/L",
        Some(col_a_fq.clone()),
        rect(10.0, 10.0, 280.0, 30.0),
    ));
    let task1_a_fq = fq_in_zone(&col_a_fq, "task:1A");
    reg.register_scope(leaf(
        task1_a_fq.clone(),
        "task:1A",
        "/L",
        Some(col_a_fq.clone()),
        rect(10.0, 50.0, 280.0, 60.0),
    ));
    reg.register_scope(leaf(
        fq_in_zone(&col_a_fq, "task:2A"),
        "task:2A",
        "/L",
        Some(col_a_fq.clone()),
        rect(10.0, 120.0, 280.0, 60.0),
    ));
    reg.register_scope(leaf(
        fq_in_zone(&col_a_fq, "task:3A"),
        "task:3A",
        "/L",
        Some(col_a_fq.clone()),
        rect(10.0, 190.0, 280.0, 60.0),
    ));

    reg.register_scope(leaf(
        fq_in_zone(&col_b_fq, "column:B.name"),
        "column:B.name",
        "/L",
        Some(col_b_fq.clone()),
        rect(310.0, 10.0, 280.0, 30.0),
    ));
    let task1_b_fq = fq_in_zone(&col_b_fq, "task:1B");
    reg.register_scope(leaf(
        task1_b_fq.clone(),
        "task:1B",
        "/L",
        Some(col_b_fq.clone()),
        rect(310.0, 50.0, 280.0, 60.0),
    ));
    reg.register_scope(leaf(
        fq_in_zone(&col_b_fq, "task:2B"),
        "task:2B",
        "/L",
        Some(col_b_fq.clone()),
        rect(310.0, 120.0, 280.0, 60.0),
    ));
    reg.register_scope(leaf(
        fq_in_zone(&col_b_fq, "task:3B"),
        "task:3B",
        "/L",
        Some(col_b_fq.clone()),
        rect(310.0, 190.0, 280.0, 60.0),
    ));

    let col_a_name_fq = fq_in_zone(&col_a_fq, "column:A.name");
    let col_b_name_fq = fq_in_zone(&col_b_fq, "column:B.name");
    assert_eq!(nav(&reg, &task1_a_fq, Direction::Right), col_b_name_fq);
    assert_eq!(nav(&reg, &task1_b_fq, Direction::Left), col_a_name_fq);
}

/// In-zone candidate is preferred over a closer cross-zone candidate.
#[test]
fn iter_0_preferred_over_iter_1_when_in_zone_match_exists() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let col0_fq = fq_in_layer("/L", "col0");
    let col1_fq = fq_in_layer("/L", "col1");
    reg.register_zone(zone(
        col0_fq.clone(),
        "col0",
        "/L",
        None,
        rect(0.0, 0.0, 100.0, 200.0),
    ));
    reg.register_zone(zone(
        col1_fq.clone(),
        "col1",
        "/L",
        None,
        rect(100.0, 0.0, 100.0, 200.0),
    ));
    let src_fq = fq_in_zone(&col0_fq, "src");
    let inzone_fq = fq_in_zone(&col0_fq, "inzone");
    reg.register_scope(leaf(
        src_fq.clone(),
        "src",
        "/L",
        Some(col0_fq.clone()),
        rect(10.0, 10.0, 30.0, 30.0),
    ));
    reg.register_scope(leaf(
        inzone_fq.clone(),
        "inzone",
        "/L",
        Some(col0_fq),
        rect(10.0, 150.0, 30.0, 30.0),
    ));
    reg.register_scope(leaf(
        fq_in_zone(&col1_fq, "crosszone"),
        "crosszone",
        "/L",
        Some(col1_fq),
        rect(110.0, 50.0, 30.0, 30.0),
    ));

    assert_eq!(nav(&reg, &src_fq, Direction::Down), inzone_fq);
}

// ---------------------------------------------------------------------------
// Layer-root termination.
// ---------------------------------------------------------------------------

/// The only leaf in a layer has nothing to navigate to → returns its
/// own FQM (semantic "stay put").
#[test]
fn layer_root_lone_leaf_returns_focused_fq() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let lonely_fq = fq_in_layer("/L", "lonely");
    reg.register_scope(leaf(
        lonely_fq.clone(),
        "lonely",
        "/L",
        None,
        rect(0.0, 0.0, 50.0, 50.0),
    ));

    assert_eq!(nav(&reg, &lonely_fq, Direction::Down), lonely_fq);
    assert_eq!(nav(&reg, &lonely_fq, Direction::Up), lonely_fq);
    assert_eq!(nav(&reg, &lonely_fq, Direction::Left), lonely_fq);
    assert_eq!(nav(&reg, &lonely_fq, Direction::Right), lonely_fq);
}

// ---------------------------------------------------------------------------
// Zone-level navigation.
// ---------------------------------------------------------------------------

/// Focused on a column zone, `nav.right` walks to the next sibling zone.
#[test]
fn zone_nav_right_picks_sibling_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let col0_fq = fq_in_layer("/L", "col0");
    let col1_fq = fq_in_layer("/L", "col1");
    reg.register_zone(zone(
        col0_fq.clone(),
        "col0",
        "/L",
        None,
        rect(0.0, 0.0, 100.0, 200.0),
    ));
    reg.register_zone(zone(
        col1_fq.clone(),
        "col1",
        "/L",
        None,
        rect(100.0, 0.0, 100.0, 200.0),
    ));
    reg.register_zone(zone(
        fq_in_layer("/L", "col2"),
        "col2",
        "/L",
        None,
        rect(200.0, 0.0, 100.0, 200.0),
    ));

    assert_eq!(nav(&reg, &col0_fq, Direction::Right), col1_fq);
}

/// Three columns laid out horizontally — `nav.up` from a column zone
/// has no sibling zone vertically, so it returns the focused FQM.
#[test]
fn zone_nav_up_with_only_horizontal_siblings_returns_self() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let col0_fq = fq_in_layer("/L", "col0");
    let col1_fq = fq_in_layer("/L", "col1");
    reg.register_zone(zone(
        col0_fq.clone(),
        "col0",
        "/L",
        None,
        rect(0.0, 0.0, 100.0, 200.0),
    ));
    reg.register_zone(zone(
        col1_fq.clone(),
        "col1",
        "/L",
        None,
        rect(100.0, 0.0, 100.0, 200.0),
    ));
    reg.register_scope(leaf(
        fq_in_zone(&col0_fq, "leaf"),
        "leaf",
        "/L",
        Some(col0_fq.clone()),
        rect(10.0, -50.0, 30.0, 30.0),
    ));

    assert_eq!(nav(&reg, &col0_fq, Direction::Up), col0_fq);
}

/// `nav.right` from a zone never returns a leaf, even if a leaf inside
/// the next column happens to be the rect-wise nearest match.
#[test]
fn zone_nav_right_does_not_return_leaf_inside_neighbor_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let col0_fq = fq_in_layer("/L", "col0");
    let col1_fq = fq_in_layer("/L", "col1");
    reg.register_zone(zone(
        col0_fq.clone(),
        "col0",
        "/L",
        None,
        rect(0.0, 0.0, 100.0, 200.0),
    ));
    reg.register_zone(zone(
        col1_fq.clone(),
        "col1",
        "/L",
        None,
        rect(200.0, 0.0, 100.0, 200.0),
    ));
    reg.register_scope(leaf(
        fq_in_zone(&col1_fq, "leaf1"),
        "leaf1",
        "/L",
        Some(col1_fq.clone()),
        rect(110.0, 10.0, 30.0, 30.0),
    ));

    let target = nav(&reg, &col0_fq, Direction::Right);
    assert_eq!(target, col1_fq);
}

// ---------------------------------------------------------------------------
// Inspector layer scenarios.
// ---------------------------------------------------------------------------

/// Two pills side-by-side in an inspector group — `nav.right` picks the
/// next pill (rule 1).
#[test]
fn inspector_pill_a_to_pill_b_in_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let group_fq = fq_in_layer("/L", "group");
    reg.register_zone(zone(
        group_fq.clone(),
        "group",
        "/L",
        None,
        rect(0.0, 0.0, 200.0, 50.0),
    ));
    let pill_a_fq = fq_in_zone(&group_fq, "pill_a");
    let pill_b_fq = fq_in_zone(&group_fq, "pill_b");
    reg.register_scope(leaf(
        pill_a_fq.clone(),
        "pill_a",
        "/L",
        Some(group_fq.clone()),
        rect(0.0, 0.0, 50.0, 30.0),
    ));
    reg.register_scope(leaf(
        pill_b_fq.clone(),
        "pill_b",
        "/L",
        Some(group_fq),
        rect(100.0, 0.0, 50.0, 30.0),
    ));

    assert_eq!(nav(&reg, &pill_a_fq, Direction::Right), pill_b_fq);
}

/// Two field-row leaves stacked vertically, each in its own zone.
/// Cross-zone `Down` drills into the destination row's natural child
/// for `Down` (the topmost child, here `label_2`).
#[test]
fn cross_zone_inspector_down_drills_into_next_row_topmost_leaf() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let row1_fq = fq_in_layer("/L", "row1");
    let row2_fq = fq_in_layer("/L", "row2");
    reg.register_zone(zone(
        row1_fq.clone(),
        "row1",
        "/L",
        None,
        rect(0.0, 0.0, 200.0, 50.0),
    ));
    reg.register_zone(zone(
        row2_fq.clone(),
        "row2",
        "/L",
        None,
        rect(0.0, 50.0, 200.0, 50.0),
    ));
    let label_1_fq = fq_in_zone(&row1_fq, "label_1");
    reg.register_scope(leaf(
        label_1_fq.clone(),
        "label_1",
        "/L",
        Some(row1_fq),
        rect(0.0, 0.0, 50.0, 30.0),
    ));
    let label_2_fq = fq_in_zone(&row2_fq, "label_2");
    reg.register_scope(leaf(
        label_2_fq.clone(),
        "label_2",
        "/L",
        Some(row2_fq),
        rect(0.0, 50.0, 50.0, 30.0),
    ));

    assert_eq!(nav(&reg, &label_1_fq, Direction::Down), label_2_fq);
}

/// The last leaf in the layer with `nav.down` returns its own FQM.
#[test]
fn inspector_last_leaf_down_returns_focused_fq() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    reg.register_scope(leaf(
        fq_in_layer("/L", "label_1"),
        "label_1",
        "/L",
        None,
        rect(0.0, 0.0, 50.0, 30.0),
    ));
    let label_2_fq = fq_in_layer("/L", "label_2");
    reg.register_scope(leaf(
        label_2_fq.clone(),
        "label_2",
        "/L",
        None,
        rect(0.0, 50.0, 50.0, 30.0),
    ));

    assert_eq!(nav(&reg, &label_2_fq, Direction::Down), label_2_fq);
}

// ---------------------------------------------------------------------------
// Realistic board scenario.
// ---------------------------------------------------------------------------

/// 3 columns × 2 cards, each card a Zone with title + status leaves.
#[test]
fn realistic_board_nav_walks_through_cards_under_unified_cascade() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));

    let col_fqs: Vec<FullyQualifiedMoniker> = ["col0", "col1", "col2"]
        .iter()
        .enumerate()
        .map(|(i, col)| {
            let col_fq = fq_in_layer("/L", col);
            reg.register_zone(zone(
                col_fq.clone(),
                col,
                "/L",
                None,
                rect(i as f64 * 100.0, 0.0, 100.0, 400.0),
            ));
            col_fq
        })
        .collect();

    // Two cards per column.
    let mut card_fqs: Vec<Vec<FullyQualifiedMoniker>> = Vec::new();
    for (i, col_fq) in col_fqs.iter().enumerate() {
        let mut col_cards = Vec::new();
        for (j, card) in ["card_a", "card_b"].iter().enumerate() {
            let card_fq = fq_in_zone(col_fq, card);
            reg.register_zone(zone(
                card_fq.clone(),
                card,
                "/L",
                Some(col_fq.clone()),
                rect(i as f64 * 100.0 + 5.0, j as f64 * 80.0 + 10.0, 90.0, 70.0),
            ));
            // Title leaf (top half).
            reg.register_scope(leaf(
                fq_in_zone(&card_fq, "title"),
                "title",
                "/L",
                Some(card_fq.clone()),
                rect(i as f64 * 100.0 + 10.0, j as f64 * 80.0 + 15.0, 80.0, 25.0),
            ));
            // Status leaf (bottom half).
            reg.register_scope(leaf(
                fq_in_zone(&card_fq, "status"),
                "status",
                "/L",
                Some(card_fq.clone()),
                rect(i as f64 * 100.0 + 10.0, j as f64 * 80.0 + 45.0, 80.0, 25.0),
            ));
            col_cards.push(card_fq);
        }
        card_fqs.push(col_cards);
    }

    let col0_card_a = &card_fqs[0][0];
    let col0_card_b = &card_fqs[0][1];
    let col0_card_a_title = fq_in_zone(col0_card_a, "title");
    let col0_card_a_status = fq_in_zone(col0_card_a, "status");

    // Within a card: title → status (iter 0, same zone).
    assert_eq!(
        nav(&reg, &col0_card_a_title, Direction::Down),
        col0_card_a_status
    );

    // Status of card A → drills into card B's natural child for
    // `Down` (the topmost leaf, here card B's `title`). Iter 1 still
    // finds card B's zone as the down peer of card A; the cascade
    // then descends into card B's natural child so the focus
    // indicator paints.
    let col0_card_b_title = fq_in_zone(col0_card_b, "title");
    assert_eq!(
        nav(&reg, &col0_card_a_status, Direction::Down),
        col0_card_b_title
    );

    // Title of col0 card_a → Right: drill out to the enclosing card zone.
    assert_eq!(
        nav(&reg, &col0_card_a_title, Direction::Right),
        *col0_card_a
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
    reg.push_layer(layer("/L", "L", "main", None));
    let card_fq = fq_in_layer("/L", "card");
    reg.register_zone(zone(
        card_fq.clone(),
        "card",
        "/L",
        None,
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    let title_fq = fq_in_zone(&card_fq, "title");
    let status_fq = fq_in_zone(&card_fq, "status");
    reg.register_scope(leaf(
        title_fq.clone(),
        "title",
        "/L",
        Some(card_fq.clone()),
        rect(10.0, 10.0, 180.0, 30.0),
    ));
    reg.register_scope(leaf(
        fq_in_zone(&card_fq, "body"),
        "body",
        "/L",
        Some(card_fq.clone()),
        rect(10.0, 60.0, 180.0, 30.0),
    ));
    reg.register_scope(leaf(
        status_fq.clone(),
        "status",
        "/L",
        Some(card_fq),
        rect(10.0, 110.0, 180.0, 30.0),
    ));

    assert_eq!(nav(&reg, &status_fq, Direction::First), title_fq);
}

/// `Direction::Last` from a leaf scopes to the leaf's `parent_zone`
/// siblings — picks the bottommost-rightmost in-zone sibling.
#[test]
fn edge_last_for_leaf_scopes_to_parent_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let card_fq = fq_in_layer("/L", "card");
    reg.register_zone(zone(
        card_fq.clone(),
        "card",
        "/L",
        None,
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    let title_fq = fq_in_zone(&card_fq, "title");
    let status_fq = fq_in_zone(&card_fq, "status");
    reg.register_scope(leaf(
        title_fq.clone(),
        "title",
        "/L",
        Some(card_fq.clone()),
        rect(10.0, 10.0, 180.0, 30.0),
    ));
    reg.register_scope(leaf(
        status_fq.clone(),
        "status",
        "/L",
        Some(card_fq),
        rect(10.0, 110.0, 180.0, 30.0),
    ));

    assert_eq!(nav(&reg, &title_fq, Direction::Last), status_fq);
}

/// `Direction::First` from a zone scopes to sibling zones — picks the
/// topmost-leftmost sibling zone.
#[test]
fn edge_first_for_zone_scopes_to_sibling_zones() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let col0_fq = fq_in_layer("/L", "col0");
    reg.register_zone(zone(
        col0_fq.clone(),
        "col0",
        "/L",
        None,
        rect(0.0, 0.0, 100.0, 200.0),
    ));
    reg.register_zone(zone(
        fq_in_layer("/L", "col1"),
        "col1",
        "/L",
        None,
        rect(100.0, 0.0, 100.0, 200.0),
    ));
    let col2_fq = fq_in_layer("/L", "col2");
    reg.register_zone(zone(
        col2_fq.clone(),
        "col2",
        "/L",
        None,
        rect(200.0, 0.0, 100.0, 200.0),
    ));

    assert_eq!(nav(&reg, &col2_fq, Direction::First), col0_fq);
}

/// `Direction::RowStart` from a leaf moves to the leftmost in-zone
/// sibling whose vertical extent overlaps the focused leaf.
#[test]
fn edge_row_start_picks_leftmost_in_row_sibling() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let row_fq = fq_in_layer("/L", "row");
    reg.register_zone(zone(
        row_fq.clone(),
        "row",
        "/L",
        None,
        rect(0.0, 0.0, 300.0, 50.0),
    ));
    let left_fq = fq_in_zone(&row_fq, "left");
    let right_fq = fq_in_zone(&row_fq, "right");
    reg.register_scope(leaf(
        left_fq.clone(),
        "left",
        "/L",
        Some(row_fq.clone()),
        rect(0.0, 10.0, 50.0, 30.0),
    ));
    reg.register_scope(leaf(
        fq_in_zone(&row_fq, "middle"),
        "middle",
        "/L",
        Some(row_fq.clone()),
        rect(100.0, 10.0, 50.0, 30.0),
    ));
    reg.register_scope(leaf(
        right_fq.clone(),
        "right",
        "/L",
        Some(row_fq),
        rect(200.0, 10.0, 50.0, 30.0),
    ));

    assert_eq!(nav(&reg, &right_fq, Direction::RowStart), left_fq);
}

/// `Direction::RowEnd` from a leaf moves to the rightmost in-zone
/// sibling whose vertical extent overlaps the focused leaf.
#[test]
fn edge_row_end_picks_rightmost_in_row_sibling() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let row_fq = fq_in_layer("/L", "row");
    reg.register_zone(zone(
        row_fq.clone(),
        "row",
        "/L",
        None,
        rect(0.0, 0.0, 300.0, 50.0),
    ));
    let left_fq = fq_in_zone(&row_fq, "left");
    let right_fq = fq_in_zone(&row_fq, "right");
    reg.register_scope(leaf(
        left_fq.clone(),
        "left",
        "/L",
        Some(row_fq.clone()),
        rect(0.0, 10.0, 50.0, 30.0),
    ));
    reg.register_scope(leaf(
        right_fq.clone(),
        "right",
        "/L",
        Some(row_fq),
        rect(200.0, 10.0, 50.0, 30.0),
    ));

    assert_eq!(nav(&reg, &left_fq, Direction::RowEnd), right_fq);
}

/// `Direction::First` from the topmost-leftmost leaf returns that
/// leaf's own FQM.
#[test]
fn edge_first_at_boundary_returns_focused_self() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let card_fq = fq_in_layer("/L", "card");
    reg.register_zone(zone(
        card_fq.clone(),
        "card",
        "/L",
        None,
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    let title_fq = fq_in_zone(&card_fq, "title");
    reg.register_scope(leaf(
        title_fq.clone(),
        "title",
        "/L",
        Some(card_fq.clone()),
        rect(10.0, 10.0, 180.0, 30.0),
    ));
    reg.register_scope(leaf(
        fq_in_zone(&card_fq, "status"),
        "status",
        "/L",
        Some(card_fq),
        rect(10.0, 60.0, 180.0, 30.0),
    ));

    assert_eq!(nav(&reg, &title_fq, Direction::First), title_fq);
}

/// `Direction::Last` from the bottommost-rightmost leaf returns that
/// leaf's own FQM.
#[test]
fn edge_last_at_boundary_returns_focused_self() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let card_fq = fq_in_layer("/L", "card");
    reg.register_zone(zone(
        card_fq.clone(),
        "card",
        "/L",
        None,
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    reg.register_scope(leaf(
        fq_in_zone(&card_fq, "title"),
        "title",
        "/L",
        Some(card_fq.clone()),
        rect(10.0, 10.0, 180.0, 30.0),
    ));
    let status_fq = fq_in_zone(&card_fq, "status");
    reg.register_scope(leaf(
        status_fq.clone(),
        "status",
        "/L",
        Some(card_fq),
        rect(10.0, 60.0, 180.0, 30.0),
    ));

    assert_eq!(nav(&reg, &status_fq, Direction::Last), status_fq);
}

/// `Direction::RowStart` from the leftmost-on-row leaf returns that
/// leaf's own FQM.
#[test]
fn edge_row_start_at_boundary_returns_focused_self() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let row_fq = fq_in_layer("/L", "row");
    reg.register_zone(zone(
        row_fq.clone(),
        "row",
        "/L",
        None,
        rect(0.0, 0.0, 300.0, 50.0),
    ));
    let left_fq = fq_in_zone(&row_fq, "left");
    reg.register_scope(leaf(
        left_fq.clone(),
        "left",
        "/L",
        Some(row_fq.clone()),
        rect(0.0, 10.0, 50.0, 30.0),
    ));
    reg.register_scope(leaf(
        fq_in_zone(&row_fq, "right"),
        "right",
        "/L",
        Some(row_fq),
        rect(200.0, 10.0, 50.0, 30.0),
    ));

    assert_eq!(nav(&reg, &left_fq, Direction::RowStart), left_fq);
}

// ---------------------------------------------------------------------------
// Layer-boundary stress: dialog → inspector → window forest.
// ---------------------------------------------------------------------------

/// Dialog stacked on inspector stacked on window. A leaf in the dialog
/// sees only dialog leaves.
#[test]
fn layer_stress_dialog_focused_sees_only_dialog_entries() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/win", "win", "main", None));
    reg.push_layer(layer("/win/inspector", "inspector", "main", Some("/win")));
    reg.push_layer(layer(
        "/win/inspector/dialog",
        "dialog",
        "main",
        Some("/win/inspector"),
    ));

    reg.register_scope(leaf(
        fq_in_layer("/win", "window_card"),
        "window_card",
        "/win",
        None,
        rect(500.0, 100.0, 50.0, 50.0),
    ));
    reg.register_scope(leaf(
        fq_in_layer("/win/inspector", "inspector_pill"),
        "inspector_pill",
        "/win/inspector",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));
    let dlg_a_fq = fq_in_layer("/win/inspector/dialog", "dlg_btn_a");
    let dlg_b_fq = fq_in_layer("/win/inspector/dialog", "dlg_btn_b");
    reg.register_scope(leaf(
        dlg_a_fq.clone(),
        "dlg_btn_a",
        "/win/inspector/dialog",
        None,
        rect(100.0, 100.0, 50.0, 50.0),
    ));
    reg.register_scope(leaf(
        dlg_b_fq.clone(),
        "dlg_btn_b",
        "/win/inspector/dialog",
        None,
        rect(200.0, 100.0, 50.0, 50.0),
    ));

    assert_eq!(nav(&reg, &dlg_a_fq, Direction::Right), dlg_b_fq);
    assert_eq!(nav(&reg, &dlg_a_fq, Direction::Up), dlg_a_fq);
}

// ---------------------------------------------------------------------------
// Unknown-FQM contracts.
// ---------------------------------------------------------------------------

/// An unknown starting FQM returns the input FQM (the `nav` helper
/// synthesises a segment from the FQM's last component for unregistered
/// FQMs) — no panic, no synthesized candidate.
#[test]
fn unknown_starting_fq_echoes_input() {
    let reg = SpatialRegistry::new();
    let ghost = FullyQualifiedMoniker::from_string("/ghost");
    assert_eq!(nav(&reg, &ghost, Direction::Right), ghost);
}

// ---------------------------------------------------------------------------
// Rect freshness.
// ---------------------------------------------------------------------------

/// Beam search runs on the **latest** rect a scope has received via
/// `update_rect`, not on the mount-time rect captured at registration.
#[test]
fn nav_down_uses_current_rect_not_stale_rect() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));
    let col_fq = fq_in_layer("/L", "col");
    reg.register_zone(zone(
        col_fq.clone(),
        "col",
        "/L",
        None,
        rect(0.0, 0.0, 200.0, 400.0),
    ));
    let card_a_fq = fq_in_zone(&col_fq, "card_a");
    let card_b_fq = fq_in_zone(&col_fq, "card_b");
    reg.register_scope(leaf(
        card_a_fq.clone(),
        "card_a",
        "/L",
        Some(col_fq.clone()),
        rect(10.0, 100.0, 180.0, 40.0),
    ));
    reg.register_scope(leaf(
        card_b_fq.clone(),
        "card_b",
        "/L",
        Some(col_fq),
        rect(10.0, 50.0, 180.0, 40.0),
    ));

    let pre_update = nav(&reg, &card_a_fq, Direction::Down);
    assert_ne!(pre_update, card_b_fq);

    reg.update_rect(&card_b_fq, rect(10.0, 200.0, 180.0, 40.0));

    assert_eq!(nav(&reg, &card_a_fq, Direction::Down), card_b_fq);
}
