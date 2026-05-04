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

/// Trajectory A: from `task:T1A`, repeated `Up` under the geometric
/// pick walks across zone boundaries directly to the visually-nearest
/// in-beam scope:
/// `T1A → field:column:TODO.name → perspective_tab:p2 →
/// navbar.board-selector → echo (stay-put at top edge)`.
///
/// Pre-fix the structural cascade drilled out to the column zone
/// after the column-name field zone, then escalated through
/// peer-zone landings with a cross-zone drill-in step. Under the
/// geometric pick the user reaches the visually-nearest scope
/// directly — column-name → perspective-tab (skipping the column
/// zone wrapper), and perspective-tab → navbar leaf (skipping the
/// navbar zone wrapper).
#[test]
fn geometric_trajectory_a_up_walks_card_to_column_name_to_perspective_tab_to_navbar_leaf() {
    let app = RealisticApp::new();

    // Step 1: T1A → field:column:TODO.name (the column-name field
    // zone above task:T1A is the visually-nearest in-beam Up scope).
    let from = app.card_fq(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Up),
        app.column_name_fq(0),
        "Up from task:T1A must land on field:column:TODO.name — the \
         visually-nearest in-beam Up scope under the geometric pick."
    );

    // Step 2: field:column:TODO.name → perspective_tab:p2 — the
    // perspective tab whose center_x (232) is closest to the
    // column-name's center_x (260) wins on minor-axis distance. The
    // geometric pick reaches across structural boundaries directly
    // without a column-zone drill-out step.
    let from = app.column_name_fq(0);
    assert_eq!(
        nav(&app, &from, Direction::Up),
        app.perspective_tab_p2_fq(),
        "Up from field:column:TODO.name must land on perspective_tab:p2 \
         under the geometric pick (closest center_x match); pre-fix the \
         structural cascade drilled out to column:TODO first."
    );

    // Step 3: perspective_tab:p1 → ui:navbar.board-selector. Pick
    // p1 explicitly here so the test exercises the
    // perspective-tab → navbar-leaf trajectory at the leftmost tab.
    let from = app.perspective_tab_p1_fq();
    assert_eq!(
        nav(&app, &from, Direction::Up),
        app.navbar_board_selector_fq(),
        "Up from perspective_tab:p1 must land on ui:navbar.board-selector \
         (the closest in-beam Up scope by center_x match)."
    );

    // Step 4: ui:navbar.board-selector → echo. The board-selector
    // leaf sits at the very top of the layer (y=8); nothing is
    // strictly above it in the Up half-plane. The geometric pick
    // stays put per the no-silent-dropout contract.
    let from = app.navbar_board_selector_fq();
    assert_eq!(
        nav(&app, &from, Direction::Up),
        from,
        "Up from ui:navbar.board-selector stays put — nothing strictly \
         above in the layer's Up half-plane."
    );

    // Step 5: ui:navbar → echo. No peer, no parent zone.
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

/// Trajectory B (geometric): `nav("task:T1A", Right) == task:T1B` —
/// the visually-adjacent card directly to the right at the same y.
/// Under the geometric pick the cross-column nav lands on the
/// matching-row card, not on the column-name header above. Pre-fix
/// the structural cascade drilled into `column:DOING`'s natural
/// child via a cross-zone drill-in step that no longer exists.
#[test]
fn geometric_trajectory_b_right_from_card_lands_on_card_in_next_column() {
    let app = RealisticApp::new();

    // Right from T1A → T1B (matching y range, in-beam Right).
    let from = app.card_fq(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Right),
        app.card_fq(1, 1),
        "Right from task:T1A must land on task:T1B — the visually-adjacent \
         card in the next column."
    );

    // Mirror: Left from T1B → T1A (symmetric).
    let from = app.card_fq(1, 1);
    assert_eq!(
        nav(&app, &from, Direction::Left),
        app.card_fq(1, 0),
        "Left from task:T1B must land on task:T1A symmetrically."
    );
}

// ---------------------------------------------------------------------------
// Trajectory C — left from leftmost card.
// ---------------------------------------------------------------------------

/// Trajectory C (geometric): `nav("task:T1A", Left)` lands inside
/// `ui:left-nav` — the LeftNav sidebar visible to the left of the
/// board. This is the cross-zone bug class fix; pre-fix the
/// structural cascade returned `column:TODO` (the parent zone).
#[test]
fn geometric_trajectory_c_left_from_leftmost_card_lands_in_left_nav() {
    let app = RealisticApp::new();

    let from = app.card_fq(1, 0);
    let result = nav(&app, &from, Direction::Left);
    let acceptable = [
        app.left_nav_fq(),
        app.view_button_grid_fq(),
        app.view_button_list_fq(),
    ];
    assert!(
        acceptable.contains(&result),
        "Left from task:T1A in the leftmost column must land inside \
         ui:left-nav under the geometric pick. Got {result:?}; expected \
         one of {acceptable:?}.",
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

    // Step 3: assignees → echo (stay-put). The inspector layer has
    // nothing strictly below the assignees field — the panel zone
    // wraps assignees and fails the strict half-plane test. Under
    // the geometric pick this is the visual-edge stay-put path.
    let from = app.inspector_field_assignees_fq();
    let result_fq = nav(&app, &from, Direction::Down);
    assert_eq!(
        result_fq, from,
        "Down from field:task:T1A.assignees stays put — the inspector \
         layer has nothing strictly below in the Down half-plane (the \
         panel zone wraps assignees and fails the strict test)."
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
