//! Integration tests for the Android-style beam-search navigator
//! under the **unified cascade** policy.
//!
//! Headless pattern matching `tests/focus_state.rs` and
//! `tests/focus_registry.rs` — pure Rust, no Tauri runtime, no jsdom.
//! Every navigation runs through [`BeamNavStrategy`] (the default
//! [`NavStrategy`] impl) and asserts on the returned [`Moniker`].
//!
//! These tests originally pinned the per-direction tactical rules
//! (rule 1 within-zone, rule 2 cross-zone leaf fallback, rule 3 no-op,
//! plus the zone-only `navigate_zone` path). The unified-policy
//! supersession card [`01KQ7S6WHK9RCCG2R4FN474EFD`] collapsed all three
//! into one cascade — these tests have been updated to assert on the
//! observable outcome of that cascade rather than on the now-removed
//! mechanism. See [`tests/unified_trajectories.rs`] for the source-of-
//! truth user trajectories the policy must satisfy; this file
//! complements that by stressing edge cases and the layer-boundary
//! contracts that the trajectories don't enumerate explicitly.
//!
//! - **Layer isolation** — nav never crosses a `LayerKey`. Two windows,
//!   two inspectors, a dialog: each is its own layer; nav stays put.
//! - **Iter 0 in-zone beam** — candidates restricted to scopes sharing
//!   `parent_zone` with the focused entry; both leaves and zones are
//!   eligible at this level.
//! - **Iter 1 cross-zone escalation** — when no in-zone match exists,
//!   the cascade escalates to the focused entry's parent zone and
//!   searches at the parent's level. Cross-column horizontal nav from
//!   a card lands on the next-column zone moniker (the React adapter
//!   handles drill-back-in if a specific leaf is desired).
//! - **Drill-out fallback** — when no peer matches at iter 0 or iter
//!   1, the cascade returns the parent zone itself rather than `None`.
//!   `None` is reserved for the focused entry sitting at the very root
//!   of its layer.
//! - **Beam scoring** — Android scoring `13 * major² + minor²` selects
//!   the closest aligned candidate among in-beam peers (the cross-axis
//!   projection is a hard filter, not a soft preference).
//! - **Edge commands** — `First`, `Last`, `RowStart`, `RowEnd` scope
//!   their candidate sets to the focused entry's siblings only — no
//!   escalation cascade for the level-bounded commands.
//!
//! [`01KQ7S6WHK9RCCG2R4FN474EFD`]: # "unified-policy supersession card"
//! [`tests/unified_trajectories.rs`]: # "source-of-truth trajectories"

use std::collections::HashMap;

use swissarmyhammer_focus::{
    BeamNavStrategy, Direction, FocusLayer, FocusScope, FocusZone, LayerKey, LayerName, Moniker,
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

/// Build a `FocusScope` leaf with the given identity, rect, layer, and
/// optional parent zone. Overrides are intentionally empty for the
/// algorithm tests — override resolution lives in another card.
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
    reg.register_scope(leaf(
        "card",
        "ui:card",
        "L_window",
        None,
        rect(200.0, 100.0, 80.0, 40.0),
    ));
    // Pill in the inspector — focused.
    reg.register_scope(leaf(
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
    reg.register_scope(leaf("a1", "ui:a1", "L_a", None, rect(0.0, 0.0, 50.0, 50.0)));
    reg.register_scope(leaf(
        "b2",
        "ui:b2",
        "L_b",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));

    assert_eq!(nav(&reg, "a1", Direction::Right), None);
}

// ---------------------------------------------------------------------------
// Iter 0 — in-zone peer search.
//
// The unified cascade's first iteration searches scopes sharing the
// focused entry's `parent_zone`. Both leaves and zones are eligible
// candidates; the in-beam Android score picks the winner.
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
    reg.register_scope(leaf(
        "title",
        "ui:title",
        "L",
        Some("card"),
        rect(10.0, 10.0, 180.0, 30.0),
    ));
    reg.register_scope(leaf(
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
    reg.register_scope(leaf(
        "title",
        "ui:title",
        "L",
        Some("card"),
        rect(10.0, 10.0, 180.0, 30.0),
    ));
    reg.register_scope(leaf(
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
/// test is a hard filter for cardinal directions: a candidate whose
/// rect does not overlap the source's cross-axis projection is dropped
/// before scoring, even when its raw `13 * major² + minor²` distance
/// would otherwise have made it the winner. See the assertion comment
/// for the worked-out numbers.
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
    reg.register_scope(leaf(
        "src",
        "ui:src",
        "L",
        Some("card"),
        rect(0.0, 0.0, 20.0, 20.0),
    ));
    // Aligned: directly below `src`, far away.
    reg.register_scope(leaf(
        "aligned",
        "ui:aligned",
        "L",
        Some("card"),
        rect(0.0, 100.0, 20.0, 20.0),
    ));
    // Diagonal: closer in raw distance but offset to the right.
    reg.register_scope(leaf(
        "diagonal",
        "ui:diagonal",
        "L",
        Some("card"),
        rect(50.0, 30.0, 20.0, 20.0),
    ));

    // The in-beam test is a hard filter. `aligned` overlaps the
    // source rect's vertical projection (x: 0..20 vs 0..20), so it
    // is in-beam and survives. `diagonal` (x: 50..70) does not
    // overlap, so it is filtered out before scoring runs.
    //
    // For reference, the raw Android scores would actually favor the
    // diagonal candidate:
    //   score(aligned)  = 13 * 80²  + 0²   = 83_200
    //   score(diagonal) = 13 * 10²  + 50²  = 1_300 + 2_500 = 3_800
    // The kernel ignores those numbers when the candidate is
    // out-of-beam — see `pick_best_candidate` in
    // `swissarmyhammer-focus/src/navigate.rs` for the rationale on
    // the hard filter. The directional-nav card
    // `01KQ7STZN3G5N2WB3FF4PM4DKX` motivated the move from a soft
    // tier preference to a hard filter (out-of-beam fallbacks were
    // letting the navbar steal `right` presses from the rightmost
    // card).
    assert_eq!(
        nav(&reg, "src", Direction::Down),
        Some(Moniker::from_string("ui:aligned"))
    );
}

// ---------------------------------------------------------------------------
// Cross-zone navigation under the unified cascade.
//
// The unified-policy supersession card (`01KQ7S6WHK9RCCG2R4FN474EFD`)
// replaced the old leaf-level "rule 2 cross-zone leaf fallback" with a
// two-level cascade: when no in-zone peer matches, the navigator
// escalates to the parent zone and searches at that level. Cross-zone
// horizontal nav now lands on the **next-column zone moniker** rather
// than a leaf inside the next column; the React adapter handles drill-
// back-in if the user wants to land on a specific leaf.
//
// The observable contract these tests pin: from a leaf with no
// horizontal in-zone peer, pressing Right / Left lands focus on the
// next-column zone (or returns the parent zone via drill-out when no
// peer exists at any level — see the leftmost-column test).
// ---------------------------------------------------------------------------

/// Two columns laid out side-by-side, each with a single leaf inside.
/// From the left column's leaf, `nav.right` finds no in-zone peer; the
/// cascade escalates to the column zone and finds the right column zone
/// as a peer at the parent's level. Returns `ui:col1` (the next-column
/// zone moniker).
///
/// Pre-supersession this test asserted on the leaf inside the next
/// column (`ui:leaf1`) — the old rule-2 cross-zone leaf fallback.
/// Under the unified cascade the kernel's answer is the zone moniker;
/// the React adapter is responsible for drilling back into a specific
/// leaf if the consumer wants that behavior.
#[test]
fn cross_zone_right_lands_on_next_column_zone() {
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
    reg.register_scope(leaf(
        "leaf0",
        "ui:leaf0",
        "L",
        Some("col0"),
        rect(10.0, 10.0, 80.0, 40.0),
    ));
    reg.register_scope(leaf(
        "leaf1",
        "ui:leaf1",
        "L",
        Some("col1"),
        rect(110.0, 10.0, 80.0, 40.0),
    ));

    assert_eq!(
        nav(&reg, "leaf0", Direction::Right),
        Some(Moniker::from_string("ui:col1")),
        "Right from a leaf with no in-zone peer must land on the next-column zone via the \
         unified cascade's iter-1 escalation"
    );
}

/// Mirror of `cross_zone_right_lands_on_next_column_zone` for
/// `nav.left`. Guards the unified cascade's symmetry across the
/// horizontal axis — and, secondarily, against a sign flip in the
/// `Direction::Left` arm of `score_candidate`.
#[test]
fn cross_zone_left_lands_on_previous_column_zone() {
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
    reg.register_scope(leaf(
        "leaf0",
        "ui:leaf0",
        "L",
        Some("col0"),
        rect(10.0, 10.0, 80.0, 40.0),
    ));
    reg.register_scope(leaf(
        "leaf1",
        "ui:leaf1",
        "L",
        Some("col1"),
        rect(110.0, 10.0, 80.0, 40.0),
    ));

    assert_eq!(
        nav(&reg, "leaf1", Direction::Left),
        Some(Moniker::from_string("ui:col0")),
        "Left from a leaf with no in-zone peer must land on the previous-column zone via the \
         unified cascade's iter-1 escalation"
    );
}

/// Production-shape regression: a board with two columns, three card
/// leaves per column, and a column-name leaf in each column header.
/// Pressing right on the top card of column A must land on the
/// next-column zone via the unified cascade's iter-1 escalation —
/// this is the kanban board layout the React side actually mounts,
/// so this test pins the kernel against the real production graph
/// rather than the synthetic one-leaf-per-zone shape
/// [`cross_zone_right_lands_on_next_column_zone`] uses. Without this
/// guard, regressions that interact with header leaves or stacked-
/// card siblings only surface in browser tests.
///
/// Pre-supersession this test asserted on a card-leaf moniker in
/// column B (`task:1B`) — the old rule-2 cross-zone leaf fallback.
/// Under the unified cascade the kernel returns the column zone
/// (`column:B`); the React adapter handles drill-back-in to a card.
#[test]
fn cross_zone_realistic_board_right_from_card_in_a_lands_on_column_b_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));

    // Outer board chrome zone — every column lives under it. Mirrors the
    // `ui:board` `<FocusZone>` in `board-view.tsx`.
    reg.register_zone(zone(
        "board",
        "ui:board",
        "L",
        None,
        rect(0.0, 0.0, 600.0, 400.0),
    ));

    // Two column zones inside the board, side-by-side.
    reg.register_zone(zone(
        "colA",
        "column:A",
        "L",
        Some("board"),
        rect(0.0, 0.0, 300.0, 400.0),
    ));
    reg.register_zone(zone(
        "colB",
        "column:B",
        "L",
        Some("board"),
        rect(300.0, 0.0, 300.0, 400.0),
    ));

    // Column A — header leaf at the top, three card leaves stacked
    // beneath it. The header lives inside the column zone (parent =
    // column zone) — same shape `column-view.tsx` produces via the
    // `<FocusScope moniker="column:<id>.name">` wrapping the header.
    reg.register_scope(leaf(
        "headerA",
        "column:A.name",
        "L",
        Some("colA"),
        rect(10.0, 10.0, 280.0, 30.0),
    ));
    reg.register_scope(leaf(
        "task1A",
        "task:1A",
        "L",
        Some("colA"),
        rect(10.0, 50.0, 280.0, 60.0),
    ));
    reg.register_scope(leaf(
        "task2A",
        "task:2A",
        "L",
        Some("colA"),
        rect(10.0, 120.0, 280.0, 60.0),
    ));
    reg.register_scope(leaf(
        "task3A",
        "task:3A",
        "L",
        Some("colA"),
        rect(10.0, 190.0, 280.0, 60.0),
    ));

    // Column B — same shape.
    reg.register_scope(leaf(
        "headerB",
        "column:B.name",
        "L",
        Some("colB"),
        rect(310.0, 10.0, 280.0, 30.0),
    ));
    reg.register_scope(leaf(
        "task1B",
        "task:1B",
        "L",
        Some("colB"),
        rect(310.0, 50.0, 280.0, 60.0),
    ));
    reg.register_scope(leaf(
        "task2B",
        "task:2B",
        "L",
        Some("colB"),
        rect(310.0, 120.0, 280.0, 60.0),
    ));
    reg.register_scope(leaf(
        "task3B",
        "task:3B",
        "L",
        Some("colB"),
        rect(310.0, 190.0, 280.0, 60.0),
    ));

    // From `task:1A`, iter 0 (peers inside `column:A`) finds no
    // horizontal candidate — every sibling is stacked above or below.
    // Iter 1 (peers at the parent's level — i.e. column zones under
    // `ui:board`) finds `column:B` as the in-beam right neighbor.
    assert_eq!(
        nav(&reg, "task1A", Direction::Right),
        Some(Moniker::from_string("column:B")),
        "the unified cascade must take the right press from task:1A across into column B's zone"
    );

    // Mirror left from `task:1B` lands on `column:A`.
    assert_eq!(
        nav(&reg, "task1B", Direction::Left),
        Some(Moniker::from_string("column:A")),
        "the unified cascade must take the left press from task:1B across into column A's zone"
    );
}

/// In-zone candidate is preferred over a closer cross-zone candidate.
/// Iter 0 (peer search at the focused entry's level) fires first; iter
/// 1 (escalation to the parent's level) only runs when iter 0 finds
/// nothing.
#[test]
fn iter_0_preferred_over_iter_1_when_in_zone_match_exists() {
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
    reg.register_scope(leaf(
        "src",
        "ui:src",
        "L",
        Some("col0"),
        rect(10.0, 10.0, 30.0, 30.0),
    ));
    // In-zone but far below.
    reg.register_scope(leaf(
        "inzone",
        "ui:inzone",
        "L",
        Some("col0"),
        rect(10.0, 150.0, 30.0, 30.0),
    ));
    // Cross-zone but closer (raw rect distance is smaller).
    reg.register_scope(leaf(
        "crosszone",
        "ui:crosszone",
        "L",
        Some("col1"),
        rect(110.0, 50.0, 30.0, 30.0),
    ));

    assert_eq!(
        nav(&reg, "src", Direction::Down),
        Some(Moniker::from_string("ui:inzone")),
        "iter 0 (in-zone peer) must win even when an iter-1 candidate is closer"
    );
}

// ---------------------------------------------------------------------------
// Layer-root termination — `None` is reserved for the very root.
//
// Under the unified cascade, a single key press always returns
// **something** unless the focused entry sits at the layer root with
// no parent zone to drill out to. The drill-out fallback prevents
// "stuck" no-ops at any other level.
// ---------------------------------------------------------------------------

/// The only leaf in a layer has nothing to navigate to → `None`. The
/// leaf sits at the layer root (`parent_zone == None`); there's no
/// parent zone to drill out to and no peer to find. This is the only
/// shape under the unified cascade where `None` is a valid answer.
#[test]
fn layer_root_lone_leaf_returns_none() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_scope(leaf(
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
    reg.register_scope(leaf(
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
    reg.register_scope(leaf(
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
    reg.register_scope(leaf(
        "pill_a",
        "ui:pill_a",
        "L",
        Some("group"),
        rect(0.0, 0.0, 50.0, 30.0),
    ));
    reg.register_scope(leaf(
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

/// Two field-row leaves stacked vertically, each in its own zone.
/// `nav.down` from the first leaf finds no in-zone peer (the row zone
/// holds only one leaf), so the unified cascade escalates and finds
/// `ui:row2` (the next row's zone) at the parent level. The kernel
/// returns the row zone moniker; the React adapter handles drill-
/// back-in to the row's leaf if desired.
///
/// Pre-supersession this test asserted on `ui:label_2` — the old
/// rule-2 cross-zone leaf fallback. Under the unified cascade the
/// kernel's answer at iter 1 is the next-row zone moniker.
#[test]
fn cross_zone_inspector_down_lands_on_next_row_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    // Each label sits in its own zone (one zone per row), so iter 0
    // finds nothing and iter 1 picks up the next row's zone.
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
    reg.register_scope(leaf(
        "label_1",
        "ui:label_1",
        "L",
        Some("row1"),
        rect(0.0, 0.0, 50.0, 30.0),
    ));
    reg.register_scope(leaf(
        "label_2",
        "ui:label_2",
        "L",
        Some("row2"),
        rect(0.0, 50.0, 50.0, 30.0),
    ));

    assert_eq!(
        nav(&reg, "label_1", Direction::Down),
        Some(Moniker::from_string("ui:row2")),
        "Down from the first row's lone leaf must land on the second row's zone via iter-1 \
         escalation under the unified cascade"
    );
}

/// The last leaf in the layer with `nav.down` returns `None` — there's
/// no leaf below, and nav never escapes the layer.
#[test]
fn inspector_last_leaf_down_returns_none() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_scope(leaf(
        "label_1",
        "ui:label_1",
        "L",
        None,
        rect(0.0, 0.0, 50.0, 30.0),
    ));
    reg.register_scope(leaf(
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
/// Verifies the unified cascade handles a realistic layout: in-card
/// nav (iter 0 peer match), cross-card nav (iter 1 peer match after
/// escalating to the column zone), cross-column nav (iter 1 peer
/// match after escalating to the column zone).
///
/// The pre-supersession version of this test asserted on leaf
/// monikers in neighboring cards (`ui:col0_card_b_title`,
/// `ui:col1_card_a_title`) — the old rule-2 cross-zone leaf fallback.
/// Under the unified cascade the kernel returns the next-card zone
/// or next-column zone moniker; the React adapter handles drill-back-
/// in if the consumer wants a specific leaf inside the destination
/// zone. The test pins the new observable outcomes.
#[test]
fn realistic_board_nav_walks_through_cards_under_unified_cascade() {
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
            reg.register_scope(leaf(
                &format!("{card_key}_title"),
                &format!("ui:{card_key}_title"),
                "L",
                Some(&card_key),
                rect(i as f64 * 100.0 + 10.0, j as f64 * 80.0 + 15.0, 80.0, 25.0),
            ));
            // Status leaf (bottom half).
            reg.register_scope(leaf(
                &format!("{card_key}_status"),
                &format!("ui:{card_key}_status"),
                "L",
                Some(&card_key),
                rect(i as f64 * 100.0 + 10.0, j as f64 * 80.0 + 45.0, 80.0, 25.0),
            ));
        }
    }

    // Within a card: title → status (iter 0, same zone).
    assert_eq!(
        nav(&reg, "col0_card_a_title", Direction::Down),
        Some(Moniker::from_string("ui:col0_card_a_status")),
        "Down inside a card should find the in-zone status leaf at iter 0"
    );

    // Status of card A → card B's zone (no peer below in card A;
    // escalate to col0 and find col0_card_b at the parent's level).
    assert_eq!(
        nav(&reg, "col0_card_a_status", Direction::Down),
        Some(Moniker::from_string("ui:col0_card_b")),
        "Down from the bottom leaf of card A must land on card B's zone via iter-1 escalation"
    );

    // Title of col0 card_a → Right: no peer right inside the card,
    // escalate to col0_card_a, no right peer at col0's child level
    // (col0_card_b is stacked below, not to the right). Drill out:
    // return col0_card_a itself. The user can press Right again from
    // the card zone to traverse to col1.
    assert_eq!(
        nav(&reg, "col0_card_a_title", Direction::Right),
        Some(Moniker::from_string("ui:col0_card_a")),
        "Right from a title leaf with no horizontal peer at iter 0 or iter 1 must drill out \
         to the enclosing card zone"
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
    reg.register_scope(leaf(
        "title",
        "ui:title",
        "L",
        Some("card"),
        rect(10.0, 10.0, 180.0, 30.0),
    ));
    reg.register_scope(leaf(
        "body",
        "ui:body",
        "L",
        Some("card"),
        rect(10.0, 60.0, 180.0, 30.0),
    ));
    reg.register_scope(leaf(
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
    reg.register_scope(leaf(
        "title",
        "ui:title",
        "L",
        Some("card"),
        rect(10.0, 10.0, 180.0, 30.0),
    ));
    reg.register_scope(leaf(
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
    reg.register_scope(leaf(
        "left",
        "ui:left",
        "L",
        Some("row"),
        rect(0.0, 10.0, 50.0, 30.0),
    ));
    reg.register_scope(leaf(
        "middle",
        "ui:middle",
        "L",
        Some("row"),
        rect(100.0, 10.0, 50.0, 30.0),
    ));
    reg.register_scope(leaf(
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
    reg.register_scope(leaf(
        "left",
        "ui:left",
        "L",
        Some("row"),
        rect(0.0, 10.0, 50.0, 30.0),
    ));
    reg.register_scope(leaf(
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
    reg.register_scope(leaf(
        "title",
        "ui:title",
        "L",
        Some("card"),
        rect(10.0, 10.0, 180.0, 30.0),
    ));
    reg.register_scope(leaf(
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
    reg.register_scope(leaf(
        "title",
        "ui:title",
        "L",
        Some("card"),
        rect(10.0, 10.0, 180.0, 30.0),
    ));
    reg.register_scope(leaf(
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
    reg.register_scope(leaf(
        "left",
        "ui:left",
        "L",
        Some("row"),
        rect(0.0, 10.0, 50.0, 30.0),
    ));
    reg.register_scope(leaf(
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
    reg.register_scope(leaf(
        "window_card",
        "ui:window_card",
        "L_window",
        None,
        rect(500.0, 100.0, 50.0, 50.0),
    ));
    // Inspector leaf — would be rect-wise above the dialog leaf.
    reg.register_scope(leaf(
        "inspector_pill",
        "ui:inspector_pill",
        "L_inspector",
        None,
        rect(100.0, 0.0, 50.0, 50.0),
    ));
    // Dialog leaves — focused on the first; only the second is visible.
    reg.register_scope(leaf(
        "dlg_btn_a",
        "ui:dlg_btn_a",
        "L_dialog",
        None,
        rect(100.0, 100.0, 50.0, 50.0),
    ));
    reg.register_scope(leaf(
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

// ---------------------------------------------------------------------------
// Rect freshness — beam search must read the current rect, not the rect
// recorded at registration time.
// ---------------------------------------------------------------------------

/// Beam search runs on the **latest** rect a scope has received via
/// `update_rect`, not on the mount-time rect captured at registration.
///
/// Regression guard for the scroll-staleness bug: when `<FocusZone>` and
/// `<FocusScope>` register their bounding-client rect on mount and only
/// refresh it via `ResizeObserver`, an ancestor scroll would shift every
/// descendant's viewport-y while the kernel kept the mount-time rect.
/// Beam-search would then run on stale geometry and pick the wrong
/// candidate (or no candidate). The React side now also re-publishes
/// rects on ancestor scroll; this test pins the kernel half of the
/// contract — that those updates actually steer beam search.
///
/// Setup: two leaves in a single zone. Card A is focused at y=100; card
/// B is initially at y=50 (above A). With those mount-time rects,
/// `nav.down(A)` would find no in-beam candidate below A and return
/// `None` (after iter-0 escalates and iter-1 finds no peers either —
/// the focused leaf has no parent zone to bubble to). After
/// `update_rect` moves B to y=200 (below A — mimicking the post-scroll
/// layout the user sees), `nav.down(A)` must return B.
#[test]
fn nav_down_uses_current_rect_not_stale_rect() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None));
    reg.register_zone(zone(
        "col",
        "ui:col",
        "L",
        None,
        rect(0.0, 0.0, 200.0, 400.0),
    ));
    // Focused: card A.
    reg.register_scope(leaf(
        "card_a",
        "ui:card_a",
        "L",
        Some("col"),
        rect(10.0, 100.0, 180.0, 40.0),
    ));
    // Card B starts ABOVE card A — `nav.down(A)` cannot reach it from
    // here.
    reg.register_scope(leaf(
        "card_b",
        "ui:card_b",
        "L",
        Some("col"),
        rect(10.0, 50.0, 180.0, 40.0),
    ));

    // Sanity: with the mount-time rects, `down` finds no in-zone
    // candidate below card A. The cascade escalates to card A's parent
    // zone (`col`), but `col` itself is at the top of the layer — there
    // are no zones below it on the same layer — so the cascade falls
    // through to a drill-out fallback that returns the parent zone's
    // own moniker. Test against that observable outcome rather than
    // hand-waving the cascade rules.
    let pre_update = nav(&reg, "card_a", Direction::Down);
    assert_ne!(
        pre_update,
        Some(Moniker::from_string("ui:card_b")),
        "with stale geometry, beam search must NOT return card B as the down-target"
    );

    // Simulate a scroll that moves card B from above to below card A.
    // The new viewport-y is 200; card A still sits at y=100.
    reg.update_rect(
        &SpatialKey::from_string("card_b"),
        rect(10.0, 200.0, 180.0, 40.0),
    );

    // Now `nav.down(card_a)` must pick card B — proving beam search
    // is running on the post-update rect, not the registration rect.
    assert_eq!(
        nav(&reg, "card_a", Direction::Down),
        Some(Moniker::from_string("ui:card_b"))
    );
}
