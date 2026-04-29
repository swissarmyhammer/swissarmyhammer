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
/// `T1A → column:TODO.name → column:TODO → ui:perspective-bar →
/// ui:navbar → ui:navbar (echoed at layer root)`.
#[test]
fn unified_trajectory_a_up_walks_card_to_header_to_column_to_perspective_bar_to_navbar() {
    let app = RealisticApp::new();

    // Step 1: T1A → column:TODO.name (peer at iter 0).
    let from = app.card_fq(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Up),
        app.column_name_fq(0),
        "Up from task:T1A must land on column:TODO.name (in-zone peer above)"
    );

    // Step 2: column:TODO.name → column:TODO (drill out — no peer at
    // either level).
    let from = app.column_name_fq(0);
    assert_eq!(
        nav(&app, &from, Direction::Up),
        app.column_fq(0),
        "Up from column:TODO.name must land on column:TODO (drill out, no peer at parent's level)"
    );

    // Step 3: column:TODO → ui:perspective-bar (peer match at iter 1
    // after escalation).
    let from = app.column_fq(0);
    assert_eq!(
        nav(&app, &from, Direction::Up),
        app.perspective_bar_fq(),
        "Up from column:TODO must land on ui:perspective-bar (peer at ui:board's level)"
    );

    // Step 4: ui:perspective-bar → ui:navbar (peer at iter 0).
    let from = app.perspective_bar_fq();
    assert_eq!(
        nav(&app, &from, Direction::Up),
        app.navbar_fq(),
        "Up from ui:perspective-bar must land on ui:navbar (peer at layer root)"
    );

    // Step 5: ui:navbar → ui:navbar (no peer, no parent zone).
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

/// Trajectory B: `nav("task:T1A", Right) == column:DOING`.
#[test]
fn unified_trajectory_b_right_from_card_in_column_a_returns_column_doing_zone() {
    let app = RealisticApp::new();

    // Right from T1A → column:DOING.
    let from = app.card_fq(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Right),
        app.column_fq(1),
        "Right from task:T1A must land on column:DOING (peer at parent's level)"
    );

    // Mirror: Left from T1B → column:TODO.
    let from = app.card_fq(1, 1);
    assert_eq!(
        nav(&app, &from, Direction::Left),
        app.column_fq(0),
        "Left from task:T1B must land on column:TODO (mirror of trajectory B)"
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
