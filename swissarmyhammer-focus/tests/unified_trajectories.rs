//! Source-of-truth integration tests for the **unified spatial-nav
//! cascade** in the kernel.
//!
//! This file pins the policy specified by the unified-nav supersession
//! card [`01KQ7S6WHK9RCCG2R4FN474EFD`]: when [`BeamNavStrategy::next`]
//! runs from a focused entry and finds no in-beam peer at the entry's
//! level, it walks **one** level up the parent chain. At the parent
//! level it searches the parent's same-level peers; if a peer satisfies
//! the beam test, that peer wins. Otherwise the cascade falls back to
//! the parent zone itself (a "drill out" by one level). Only when the
//! focused entry sits at the layer root with no parent does the
//! navigator echo the focused FQM.
//!
//! # Layer-boundary guard
//!
//! Escalation is bounded by the layer's FQM. The inspector layer is
//! captured-focus: navigation inside a `panel:*` zone never escalates
//! up into the window layer that hosts `ui:board`. Trajectory D
//! exercises this contract end-to-end.
//!
//! [`01KQ7S6WHK9RCCG2R4FN474EFD`]: # "unified-policy supersession card"
//! [`BeamNavStrategy::next`]: swissarmyhammer_focus::BeamNavStrategy::next

mod fixtures;

use swissarmyhammer_focus::{BeamNavStrategy, Direction, FullyQualifiedMoniker, NavStrategy};

use fixtures::RealisticApp;

/// Convenience: run [`BeamNavStrategy::next`] against the fixture's
/// registry from the named [`FullyQualifiedMoniker`] in the named
/// [`Direction`].
fn nav(app: &RealisticApp, from: &FullyQualifiedMoniker, dir: Direction) -> FullyQualifiedMoniker {
    let focused_segment = app
        .registry()
        .find_by_fq(from)
        .map(|e| e.segment().clone())
        .unwrap_or_else(|| panic!("nav called with unregistered FQM {from:?}"));
    BeamNavStrategy::new().next(app.registry(), from, &focused_segment, dir)
}

// ---------------------------------------------------------------------------
// Trajectory A — vertical traversal up the entity stack.
// ---------------------------------------------------------------------------

/// Trajectory A: from `task:T1A`, repeated `Up` walks
/// `T1A → field:column:TODO.name → column:TODO → perspective_tab:p1
/// → ui:navbar.board-selector → ui:navbar (echoed at layer root)`.
///
/// Under the any-kind iter-0 sibling rule (`zones and scopes are
/// siblings under a parent zone`), the column-name field zone above
/// `task:T1A` is a valid Up candidate even though it is a zone and
/// the focused entry is a leaf — both share `column:TODO` as their
/// `parent_zone`. The walk visits the column-name zone first, then
/// drills out to the column zone, then escalates through the chrome
/// peers at the layer root with cross-zone drill-in landing on each
/// destination's natural child for `Up` (the bottommost leaf, with
/// leftmost as tie-break).
#[test]
fn unified_trajectory_a_up_walks_card_to_column_name_to_column_to_perspective_tab_to_navbar_leaf()
{
    let app = RealisticApp::new();

    // Step 1: T1A → field:column:TODO.name (any-kind in-zone Up peer
    // at iter 0 — the column-name field zone above task:T1A is a
    // sibling under `column:TODO`).
    let from = app.card_fq(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Up),
        app.column_name_fq(0),
        "Up from task:T1A must land on field:column:TODO.name (any-kind \
         in-zone sibling — both share column:TODO as their parent_zone, \
         and iter 0 picks the geometrically best candidate of any kind)"
    );

    // Step 2: field:column:TODO.name → column:TODO (drill out — there
    // is no Up peer of the column-name zone in column:TODO at iter 0,
    // and iter 1 finds no peer-zone of column:TODO above it at
    // ui:board's level; the cascade falls back to the parent zone).
    // Drill-out does NOT trigger drill-in.
    let from = app.column_name_fq(0);
    assert_eq!(
        nav(&app, &from, Direction::Up),
        app.column_fq(0),
        "Up from field:column:TODO.name must drill out to column:TODO \
         (no Up peer at iter 0 inside column:TODO; iter 1 finds no \
         peer-zone of column:TODO above; cascade returns the parent zone). \
         Drill-out does not trigger drill-in."
    );

    // Step 3: column:TODO → perspective_tab:p1 (peer match at iter 1
    // after escalation, then cross-zone drill-in into
    // ui:perspective-bar's natural Up child — the bottommost leaf,
    // with leftmost as tie-break — lands on perspective_tab:p1).
    let from = app.column_fq(0);
    assert_eq!(
        nav(&app, &from, Direction::Up),
        app.perspective_tab_p1_fq(),
        "Up from column:TODO must drill into ui:perspective-bar's natural \
         Up child (bottommost leaf, tie-broken leftmost): perspective_tab:p1"
    );

    // Step 4: perspective_tab:p1 → ui:navbar.board-selector (iter 1
    // escalates to ui:perspective-bar, finds ui:navbar as the Up
    // peer at the layer root, drills into ui:navbar's natural Up
    // child — the bottommost leaf, tie-broken leftmost — which is
    // ui:navbar.board-selector).
    let from = app.perspective_tab_p1_fq();
    assert_eq!(
        nav(&app, &from, Direction::Up),
        app.navbar_board_selector_fq(),
        "Up from perspective_tab:p1 must drill into ui:navbar's natural \
         Up child (bottommost leaf, tie-broken leftmost): ui:navbar.board-selector"
    );

    // Step 5: ui:navbar.board-selector → ui:navbar (drill out — no
    // Up peer inside ui:navbar at iter 0; iter 1 from ui:navbar's
    // grandparent finds no Up peer at the layer root; cascade falls
    // back to ui:navbar). Drill-out does not trigger drill-in.
    let from = app.navbar_board_selector_fq();
    assert_eq!(
        nav(&app, &from, Direction::Up),
        app.navbar_fq(),
        "Up from ui:navbar.board-selector must drill out to ui:navbar \
         (no Up peer at iter 0; iter 1 finds no peer-zone of ui:navbar \
         above at the layer root; cascade returns the parent zone)"
    );

    // Step 6: ui:navbar → ui:navbar (no peer, no parent zone).
    // Under the no-silent-dropout contract the cascade echoes the
    // focused FQM rather than returning None.
    let from = app.navbar_fq();
    assert_eq!(
        nav(&app, &from, Direction::Up),
        app.navbar_fq(),
        "Up from ui:navbar must echo the focused FQM (at layer root with no Up peer)"
    );
}

// ---------------------------------------------------------------------------
// Trajectory B — cross-column horizontal navigation.
// ---------------------------------------------------------------------------

/// Trajectory B: `nav("task:T1A", Right) == field:column:DOING.name`
/// — iter 1 finds `column:DOING` as the Right peer of `column:TODO`,
/// then cross-zone drill-in descends into `column:DOING`'s natural
/// `Right` child (leftmost child with topmost tie-break, here the
/// column-name field zone).
#[test]
fn unified_trajectory_b_right_from_card_in_column_a_drills_into_column_doing_name() {
    let app = RealisticApp::new();

    // Right from T1A → field:column:DOING.name (cross-zone drill-in
    // resolves to the leftmost child of column:DOING, tie-broken
    // topmost).
    let from = app.card_fq(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Right),
        app.column_name_fq(1),
        "Right from task:T1A must drill into column:DOING's natural Right child \
         (leftmost child, tie-broken topmost): field:column:DOING.name"
    );

    // Mirror: Left from T1B → field:column:TODO.name (cross-zone
    // drill-in resolves to the rightmost child of column:TODO,
    // tie-broken topmost).
    let from = app.card_fq(1, 1);
    assert_eq!(
        nav(&app, &from, Direction::Left),
        app.column_name_fq(0),
        "Left from task:T1B must drill into column:TODO's natural Left child \
         (rightmost child, tie-broken topmost): field:column:TODO.name"
    );
}

// ---------------------------------------------------------------------------
// Trajectory C — left from leftmost card.
// ---------------------------------------------------------------------------

/// Trajectory C: `nav("task:T1A", Left) == column:TODO`.
#[test]
fn unified_trajectory_c_left_from_leftmost_card_drills_out_to_column_zone() {
    let app = RealisticApp::new();

    let from = app.card_fq(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Left),
        app.column_fq(0),
        "Left from task:T1A in the leftmost column must drill out to column:TODO \
         (no peer at any level; the cascade returns the parent zone rather than echoing)"
    );
}

// ---------------------------------------------------------------------------
// Trajectory D — inspector field navigation with the layer-boundary guard.
// ---------------------------------------------------------------------------

/// Trajectory D: field-row navigation inside the inspector panel,
/// with the layer-boundary guard preventing escapes into the window
/// layer.
#[test]
fn unified_trajectory_d_down_between_inspector_field_zones_with_layer_boundary_guard() {
    let app = RealisticApp::new();

    // Step 1: title → status.
    let from = app.inspector_field_title_fq();
    assert_eq!(
        nav(&app, &from, Direction::Down),
        app.inspector_field_status_fq(),
        "Down from field:task:T1A.title must land on field:task:T1A.status (peer below)"
    );

    // Step 2: status → assignees.
    let from = app.inspector_field_status_fq();
    assert_eq!(
        nav(&app, &from, Direction::Down),
        app.inspector_field_assignees_fq(),
        "Down from field:task:T1A.status must land on field:task:T1A.assignees (peer below)"
    );

    // Step 3: assignees → panel:task:T1A (drill out — panel has no
    // siblings within the inspector layer, so the cascade returns the
    // parent zone itself).
    let from = app.inspector_field_assignees_fq();
    let result_fq = nav(&app, &from, Direction::Down);
    assert_eq!(
        result_fq,
        app.inspector_panel_fq(),
        "Down from field:task:T1A.assignees must drill out to panel:task:T1A \
         (no peer at any inspector-layer level; cascade returns the parent zone)"
    );

    // Layer-boundary guard. The result above must NOT be any FQM
    // from the window layer — the inspector is captured-focus. Iterate
    // every window-layer registration and confirm the navigator's
    // answer doesn't appear in that set.
    let window_layer_fq = fixtures::window_layer_fq();
    let window_fqs: Vec<&FullyQualifiedMoniker> = app
        .registry()
        .leaves_in_layer(&window_layer_fq)
        .map(|f| &f.fq)
        .chain(
            app.registry()
                .zones_in_layer(&window_layer_fq)
                .map(|z| &z.fq),
        )
        .collect();
    for fq in &window_fqs {
        assert_ne!(
            &&result_fq, fq,
            "layer-boundary guard violated: navigator returned {fq:?} which lives in the window \
             layer, but the focused entry was in the inspector layer",
        );
    }

    // Step 4: panel:task:T1A → echo (the panel sits at the
    // inspector layer root with no siblings; escalation has nowhere to
    // go without crossing the layer boundary, which the cascade
    // refuses to do). Under the no-silent-dropout contract this echoes
    // the focused FQM rather than returning None.
    let from = app.inspector_panel_fq();
    let result = nav(&app, &from, Direction::Down);
    assert_eq!(
        result,
        app.inspector_panel_fq(),
        "Down from panel:task:T1A (alone at the inspector layer root) must echo the focused \
         FQM — the cascade refuses to cross the layer boundary into the window layer"
    );

    // Confirm the inspector layer FQM resolves the panel's expected
    // owning layer — a tripwire that catches fixture drift renaming
    // the inspector layer or moving the panel onto the window layer.
    let inspector_layer_fq = fixtures::inspector_layer_fq();
    let panel_lives_in_inspector = app
        .registry()
        .zones_in_layer(&inspector_layer_fq)
        .any(|z| z.segment.as_str() == "panel:task:T1A");
    assert!(
        panel_lives_in_inspector,
        "fixture invariant: panel:task:T1A must register on the inspector layer"
    );
}
