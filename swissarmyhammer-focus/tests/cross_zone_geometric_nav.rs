//! Source-of-truth integration tests for the **geometric cardinal-pick**
//! algorithm — keyboard-as-mouse navigation in the spatial-nav kernel.
//!
//! These four tests pin the cross-zone bug class that motivated the
//! redesign (see kanban tasks `01KQQTXDHP3XBHZ8G40AC4FG4D` and the
//! design `01KQQSXM2PEYR1WAQ7QXW3B8ME`). Each test reproduces one of
//! the four reported scenarios where structural cascade returned
//! `target=None`, `scope_chain=["engine"]`, or otherwise failed to
//! land on the visually-adjacent target. Under the geometric pick the
//! visually-nearest registered scope in direction D wins, regardless of
//! structural depth.
//!
//! # Contract restated
//!
//! Cardinal nav from `focused` in direction D returns the registered
//! scope (leaf or zone, in the same `layer_fq`) whose rect minimises
//! the Android beam score (`13 * major² + minor²`) across ALL registered
//! scopes in the layer that pass the in-beam test for D and lie strictly
//! in the half-plane of D. No structural filtering — `parent_zone` and
//! `is_zone` are tie-breakers and observability only.
//!
//! When the half-plane is empty (focused at the visual edge of the
//! layer), the kernel returns the focused FQM (stay-put, per the
//! no-silent-dropout invariant).

mod fixtures;

use swissarmyhammer_focus::{BeamNavStrategy, Direction, FullyQualifiedMoniker, NavStrategy};

use fixtures::RealisticApp;

/// Convenience: run [`BeamNavStrategy::next`] against the fixture's
/// registry from the named [`FullyQualifiedMoniker`] in the named
/// [`Direction`]. Centralised so each test reads top-to-bottom without
/// repeating the boilerplate.
fn nav(app: &RealisticApp, from: &FullyQualifiedMoniker, dir: Direction) -> FullyQualifiedMoniker {
    let focused_segment = app
        .registry()
        .find_by_fq(from)
        .map(|e| e.segment().clone())
        .unwrap_or_else(|| panic!("nav called with unregistered FQM {from:?}"));
    BeamNavStrategy::new().next(app.registry(), from, &focused_segment, dir)
}

// ---------------------------------------------------------------------------
// Scenario 1: Left from leftmost perspective tab → inside ui:left-nav.
// ---------------------------------------------------------------------------

/// Pressing `Left` from `perspective_tab:p1` (leftmost tab in the
/// perspective bar) lands inside `ui:left-nav` — either on the
/// view-button leaves stacked vertically inside or on the LeftNav zone
/// itself. The geometric pick walks across the structural boundary
/// (perspective bar → window root → left-nav zone) because the
/// view-button leaves are visually the nearest scopes in the Left
/// half-plane.
///
/// Pre-fix: structural cascade returned `target=None` /
/// `scope_chain=["engine"]` (the bug captured by card
/// `01KQPW1FTYFWTDMW6ESM5ABGJQ`). Under the geometric algorithm,
/// `view:grid` is at center_y=60 (matching p1's center_y) so it wins
/// with the lowest beam score.
#[test]
fn left_from_leftmost_perspective_tab_lands_in_left_nav() {
    let app = RealisticApp::new();
    let from = app.perspective_tab_p1_fq();
    let result = nav(&app, &from, Direction::Left);

    let acceptable = [
        app.left_nav_fq(),
        app.view_button_grid_fq(),
        app.view_button_list_fq(),
    ];
    assert!(
        acceptable.contains(&result),
        "Left from perspective_tab:p1 must land inside ui:left-nav (the zone \
         itself or one of its view:{{id}} leaves). The geometric pick should \
         find the LeftNav surface — the visually-nearest scope in the Left \
         half-plane. Got {result:?}; expected one of {acceptable:?}.",
    );
    assert_ne!(
        result, from,
        "Left from perspective_tab:p1 must not echo the focused FQM — \
         there is a real LeftNav surface to the left, the geometric pick \
         should find it."
    );
}

// ---------------------------------------------------------------------------
// Scenario 2: Up from a board column zone → inside ui:perspective-bar.
// ---------------------------------------------------------------------------

/// Pressing `Up` from `column:TODO` lands inside `ui:perspective-bar`
/// — either on the perspective-tab leaves visible above or on the
/// perspective-bar zone itself. Structurally the column is a child of
/// `ui:board`, the perspective bar is a peer of the board at the layer
/// root, and there is no shared `parent_zone`. The geometric pick
/// crosses that structural gap because the perspective bar's tab leaves
/// are the visually-nearest scopes in the Up half-plane from the
/// column.
///
/// Pre-fix: structural cascade with iter-1 escalation could miss this
/// landing or stop at `ui:perspective-bar` without drilling in. Under
/// the geometric algorithm the perspective tab leaves are direct
/// candidates.
#[test]
fn up_from_column_lands_in_perspective_bar() {
    let app = RealisticApp::new();
    let from = app.column_fq(0);
    let result = nav(&app, &from, Direction::Up);

    let acceptable = [
        app.perspective_bar_fq(),
        app.perspective_tab_p1_fq(),
        app.perspective_tab_p2_fq(),
        app.perspective_tab_p3_fq(),
    ];
    assert!(
        acceptable.contains(&result),
        "Up from column:TODO must land inside ui:perspective-bar (the zone \
         itself or one of its perspective_tab:p{{n}} leaves). Got {result:?}; \
         expected one of {acceptable:?}.",
    );
    let forbidden = [
        app.column_fq(0), // stay-put / echo
        app.board_fq(),   // an unrelated chrome zone above the column
        app.left_nav_fq(),
    ];
    assert!(
        !forbidden.contains(&result),
        "Up from column:TODO must not stay put, drill out to ui:board, or \
         land on ui:left-nav — got {result:?}",
    );
}

// ---------------------------------------------------------------------------
// Scenario 3: Down from a perspective tab → inside the perspective body.
// ---------------------------------------------------------------------------

/// Pressing `Down` from `perspective_tab:p1` lands inside the
/// perspective body — the column-name field zone, a column zone, or a
/// card. The geometric pick reaches across `ui:perspective-bar`'s
/// boundary into `ui:board`'s descendants because they're visually
/// below the tab in the Down half-plane.
///
/// Pre-fix: structural cascade had no path from a perspective-tab leaf
/// to a column or card without hitting `ui:perspective-bar` first as an
/// iter-1 candidate (and the perspective bar has no Down peer).
#[test]
fn down_from_perspective_tab_lands_in_perspective_body() {
    let app = RealisticApp::new();
    let from = app.perspective_tab_p1_fq();
    let result = nav(&app, &from, Direction::Down);

    // Acceptable landings: any column-name field zone, any column zone,
    // any card leaf, or the board zone — anything strictly inside the
    // perspective body that's visually below p1.
    let mut acceptable: Vec<FullyQualifiedMoniker> = Vec::new();
    acceptable.push(app.board_fq());
    for i in 0..3 {
        acceptable.push(app.column_fq(i));
        acceptable.push(app.column_name_fq(i));
        for row in 1..=3 {
            acceptable.push(app.card_fq(row, i));
        }
    }
    assert!(
        acceptable.contains(&result),
        "Down from perspective_tab:p1 must land inside the perspective \
         body (a column zone, a column-name field zone, a card leaf, or \
         ui:board). Got {result:?}.",
    );
    let forbidden = [
        app.perspective_tab_p1_fq(), // stay-put
        app.perspective_bar_fq(),    // parent zone (no drill-out semantics for cardinal)
        app.navbar_fq(),
        app.left_nav_fq(),
    ];
    assert!(
        !forbidden.contains(&result),
        "Down from perspective_tab:p1 must not stay put, drill out to its \
         parent zone, or land on the navbar or left-nav. Got {result:?}.",
    );
}

// ---------------------------------------------------------------------------
// Scenario 4: Up from perspective bar → inside ui:navbar.
// ---------------------------------------------------------------------------

/// Pressing `Up` from `ui:perspective-bar` lands inside `ui:navbar` —
/// either on a navbar leaf entry or on the navbar zone itself. Both
/// the navbar and the perspective bar are layer-root zones (no shared
/// `parent_zone`), but the navbar's leaves are visually directly above
/// the perspective bar in the Up half-plane.
///
/// This case also covers the symmetric "Up from any board column lands
/// on perspective bar" — see `up_from_column_lands_in_perspective_bar`
/// above. Together they pin the multi-layer chain navbar ←
/// perspective-bar ← board.
#[test]
fn up_from_perspective_bar_lands_in_navbar() {
    let app = RealisticApp::new();
    let from = app.perspective_bar_fq();
    let result = nav(&app, &from, Direction::Up);

    let acceptable = [
        app.navbar_fq(),
        app.navbar_board_selector_fq(),
        app.navbar_inspect_fq(),
        app.navbar_percent_field_fq(),
        app.navbar_search_fq(),
    ];
    assert!(
        acceptable.contains(&result),
        "Up from ui:perspective-bar must land inside ui:navbar (the zone \
         itself or one of its leaf/field entries). Got {result:?}; expected \
         one of {acceptable:?}.",
    );
    let forbidden = [
        app.perspective_bar_fq(), // stay-put / echo
        app.board_fq(),
        app.left_nav_fq(),
    ];
    assert!(
        !forbidden.contains(&result),
        "Up from ui:perspective-bar must not stay put, drill out to ui:board, \
         or land on ui:left-nav. Got {result:?}.",
    );
}
