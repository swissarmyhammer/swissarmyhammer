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
//! navigator return `None`.
//!
//! That two-step cascade with a drill-out tail collapses what the
//! per-direction tactical cards used to keep separate:
//!
//! - **Leaf rule 1** (in-zone leaves) — covered by the iteration-0
//!   peer search.
//! - **Leaf rule 2** (cross-zone leaf fallback) — replaced by escalation;
//!   cross-column nav now returns the next-column zone moniker rather
//!   than a leaf inside the next column. The React adapter handles
//!   any drill-back-in if the user wants to land on a specific leaf.
//! - **Zone-only `navigate_zone`** — also replaced; zones use the same
//!   two-step cascade.
//!
//! # Layer-boundary guard
//!
//! Escalation is bounded by `LayerKey`. The inspector layer is
//! captured-focus: navigation inside a `panel:*` zone never escalates
//! up into the window layer that hosts `ui:board`. Trajectory D
//! exercises this contract end-to-end.
//!
//! # Geometry
//!
//! All four trajectories run against the realistic-app fixture in
//! [`fixtures::RealisticApp`], whose rectangles match the production
//! kanban-app at desktop scale (1400×900 viewport, 440 px columns,
//! ~80 px cards). Synthetic geometry would let scoring drift away from
//! the user-visible answers; the realistic fixture pins the kernel
//! against the same rect math the running app produces.
//!
//! [`01KQ7S6WHK9RCCG2R4FN474EFD`]: # "unified-policy supersession card"
//! [`BeamNavStrategy::next`]: swissarmyhammer_focus::BeamNavStrategy::next
//! [`fixtures::RealisticApp`]: crate::fixtures::RealisticApp

mod fixtures;

use swissarmyhammer_focus::{BeamNavStrategy, Direction, Moniker, NavStrategy, SpatialKey};

use fixtures::{RealisticApp, INSPECTOR_LAYER_KEY, WINDOW_LAYER_KEY};

/// Convenience: run [`BeamNavStrategy::next`] against the fixture's
/// registry from the named [`SpatialKey`] in the named [`Direction`].
/// Centralised so each test reads top-to-bottom without repeating the
/// boilerplate.
fn nav(app: &RealisticApp, from: &SpatialKey, dir: Direction) -> Option<Moniker> {
    BeamNavStrategy::new().next(app.registry(), from, dir)
}

/// Resolve a [`Moniker`] back to its [`SpatialKey`] via the fixture's
/// registry. Trajectory tests that walk multiple navigation steps need
/// this to feed each step's result back in as the next step's focused
/// key.
///
/// Panics if no registered scope carries the requested moniker — every
/// caller in this file works against monikers the fixture is known to
/// register, so the panic surfaces a fixture drift rather than a real
/// failure mode.
fn key_for(app: &RealisticApp, moniker: &str) -> SpatialKey {
    app.registry()
        .leaves_iter()
        .map(|f| (&f.key, &f.moniker))
        .chain(app.registry().zones_iter().map(|z| (&z.key, &z.moniker)))
        .find(|(_, m)| m.as_str() == moniker)
        .map(|(k, _)| k.clone())
        .unwrap_or_else(|| panic!("fixture has no registered scope with moniker {moniker:?}"))
}

// ---------------------------------------------------------------------------
// Trajectory A — vertical traversal up the entity stack.
//
// Pressing `Up` repeatedly from a card walks the user up through the
// enclosing column header, then the column zone, then the perspective
// bar above the board, then the navbar, then nowhere. Each press moves
// at most one zone-level out, with peer matches taken when available.
// ---------------------------------------------------------------------------

/// Trajectory A: from `task:T1A`, repeated `Up` walks
/// `T1A → column:TODO.name → column:TODO → ui:perspective-bar →
/// ui:navbar → None`.
///
/// - **T1A → column:TODO.name** — the column-name leaf is `T1A`'s
///   in-zone peer above. Iter 0 (peer search at the leaf level) wins.
/// - **column:TODO.name → column:TODO** — no in-zone peer is above
///   the header (the cards are stacked below it); iter 1 (peer search
///   at the column-zone level) finds nothing in the `Up` direction
///   either (column:DOING and column:DONE are at the same `top` as
///   column:TODO). The cascade falls back to the parent zone itself.
/// - **column:TODO → ui:perspective-bar** — column:TODO has no zone
///   peer above (siblings same `top`); iter 1 escalates into ui:board's
///   level where ui:perspective-bar is the closest in-beam Up peer.
/// - **ui:perspective-bar → ui:navbar** — ui:navbar is the in-beam Up
///   peer at the layer root. Iter 0 wins.
/// - **ui:navbar → None** — no Up peer at the layer root and no parent
///   zone to drill out to. The cascade returns `None`.
#[test]
fn unified_trajectory_a_up_walks_card_to_header_to_column_to_perspective_bar_to_navbar() {
    let app = RealisticApp::new();

    // Step 1: T1A → column:TODO.name (peer at iter 0).
    let from = app.card_key(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Up),
        Some(Moniker::from_string("column:TODO.name")),
        "Up from task:T1A must land on column:TODO.name (in-zone peer above)"
    );

    // Step 2: column:TODO.name → column:TODO (drill out — no peer at
    // either level).
    let from = key_for(&app, "column:TODO.name");
    assert_eq!(
        nav(&app, &from, Direction::Up),
        Some(Moniker::from_string("column:TODO")),
        "Up from column:TODO.name must land on column:TODO (drill out, no peer at parent's level)"
    );

    // Step 3: column:TODO → ui:perspective-bar (peer match at iter 1
    // after escalation).
    let from = key_for(&app, "column:TODO");
    assert_eq!(
        nav(&app, &from, Direction::Up),
        Some(Moniker::from_string("ui:perspective-bar")),
        "Up from column:TODO must land on ui:perspective-bar (peer at ui:board's level)"
    );

    // Step 4: ui:perspective-bar → ui:navbar (peer at iter 0).
    let from = key_for(&app, "ui:perspective-bar");
    assert_eq!(
        nav(&app, &from, Direction::Up),
        Some(Moniker::from_string("ui:navbar")),
        "Up from ui:perspective-bar must land on ui:navbar (peer at layer root)"
    );

    // Step 5: ui:navbar → None (no peer, no parent zone).
    let from = key_for(&app, "ui:navbar");
    assert_eq!(
        nav(&app, &from, Direction::Up),
        None,
        "Up from ui:navbar must return None (at layer root with no Up peer)"
    );
}

// ---------------------------------------------------------------------------
// Trajectory B — cross-column horizontal navigation.
//
// Pressing `Right` from a card in column TODO escalates to column:TODO
// (no peer matches inside the column), then iter 1 finds column:DOING
// at the parent level. The kernel returns the next-column zone moniker;
// the React adapter handles drill-back-in to a specific card if it
// wants. Mirror: `Left` from a card in column DOING returns column:TODO.
// ---------------------------------------------------------------------------

/// Trajectory B: `nav("task:T1A", Right) == Some("column:DOING")`.
///
/// Iter 0 finds no in-zone peer to the right of T1A — the column's
/// children are stacked vertically. Iter 1 escalates to column:TODO and
/// scores its zone peers (column:DOING, column:DONE) for `Right`.
/// column:DOING is the in-beam neighbor.
///
/// Mirror: `Left` from T1B returns column:TODO via the same path
/// reflected across the major axis.
#[test]
fn unified_trajectory_b_right_from_card_in_column_a_returns_column_doing_zone() {
    let app = RealisticApp::new();

    // Right from T1A → column:DOING.
    let from = app.card_key(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Right),
        Some(Moniker::from_string("column:DOING")),
        "Right from task:T1A must land on column:DOING (peer at parent's level)"
    );

    // Mirror: Left from T1B → column:TODO.
    let from = app.card_key(1, 1);
    assert_eq!(
        nav(&app, &from, Direction::Left),
        Some(Moniker::from_string("column:TODO")),
        "Left from task:T1B must land on column:TODO (mirror of trajectory B)"
    );
}

// ---------------------------------------------------------------------------
// Trajectory C — left from leftmost card.
//
// `Left` from any card in the leftmost column has no in-zone peer
// (cards are stacked vertically) and the parent column has no zone
// peer to its left (column:TODO is the leftmost column). The cascade
// drills out by one — to column:TODO. The user's mental model: a key
// press should never get "stuck" returning `None` unless the focused
// entry sits at the very root of its layer.
// ---------------------------------------------------------------------------

/// Trajectory C: `nav("task:T1A", Left) == Some("column:TODO")`.
///
/// Iter 0 finds no in-zone peer to the left of T1A. Iter 1 escalates
/// to column:TODO and scores column:DOING / column:DONE for `Left` —
/// both are to the right, not to the left. The cascade falls back to
/// the parent zone itself: drill out to column:TODO.
///
/// This is the user's "no `None` except at the very root" contract:
/// pressing `Left` from a card in the leftmost column moves focus up
/// to the column zone rather than dead-ending. From column:TODO, a
/// further `Left` press would walk one more level out (toward the
/// board / layer root) before eventually returning `None` at the very
/// top.
#[test]
fn unified_trajectory_c_left_from_leftmost_card_drills_out_to_column_zone() {
    let app = RealisticApp::new();

    let from = app.card_key(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Left),
        Some(Moniker::from_string("column:TODO")),
        "Left from task:T1A in the leftmost column must drill out to column:TODO \
         (no peer at any level; the cascade returns the parent zone rather than None)"
    );
}

// ---------------------------------------------------------------------------
// Trajectory D — inspector field navigation with the layer-boundary guard.
//
// The inspector layer hosts `panel:task:T1A` at its root, with three
// field zones (`title`, `status`, `assignees`) stacked vertically
// inside. `Down` walks title→status→assignees as in-zone peer matches;
// from `assignees` (the bottommost field) the cascade escalates to
// `panel:task:T1A`, which has no siblings within the inspector layer
// and so drills out to the panel itself. The cascade NEVER crosses
// from the inspector layer into the window layer — the inspector is
// captured-focus.
// ---------------------------------------------------------------------------

/// Trajectory D: field-row navigation inside the inspector panel,
/// with the layer-boundary guard preventing escapes into the window
/// layer.
///
/// - `field:task:T1A.title → field:task:T1A.status` (in-zone peer below).
/// - `field:task:T1A.status → field:task:T1A.assignees` (in-zone peer
///   below).
/// - `field:task:T1A.assignees → panel:task:T1A` — no in-zone peer
///   below; iter 1 escalates to `panel:task:T1A`. The panel has no
///   siblings (the inspector layer is captured-focus, with one panel
///   zone at its root). The cascade drills out to the panel itself.
///
/// **Layer-boundary guard.** From the panel a further `Down` press
/// would attempt to escalate, but `panel:task:T1A` has `parent_zone =
/// None` — the panel is at the inspector layer root. Escalation never
/// crosses `LayerKey` boundaries (the kernel's absolute contract from
/// `tests/navigate.rs::nav_never_crosses_layer_boundary_within_one_window`),
/// so the navigator must NOT lift focus into the window layer's
/// `ui:board` zone or any of its descendants. This test asserts the
/// result of `Down` from `field:task:T1A.assignees` is the panel's
/// own moniker AND that the assertion on the next `Down` press from
/// the panel returns `None` — the only valid "stuck" answer per the
/// user's contract is the very root of a layer.
#[test]
fn unified_trajectory_d_down_between_inspector_field_zones_with_layer_boundary_guard() {
    let app = RealisticApp::new();

    // Step 1: title → status.
    let from = key_for(&app, "field:task:T1A.title");
    assert_eq!(
        nav(&app, &from, Direction::Down),
        Some(Moniker::from_string("field:task:T1A.status")),
        "Down from field:task:T1A.title must land on field:task:T1A.status (peer below)"
    );

    // Step 2: status → assignees.
    let from = key_for(&app, "field:task:T1A.status");
    assert_eq!(
        nav(&app, &from, Direction::Down),
        Some(Moniker::from_string("field:task:T1A.assignees")),
        "Down from field:task:T1A.status must land on field:task:T1A.assignees (peer below)"
    );

    // Step 3: assignees → panel:task:T1A (drill out — panel has no
    // siblings within the inspector layer, so the cascade returns the
    // parent zone itself).
    let from = key_for(&app, "field:task:T1A.assignees");
    let result = nav(&app, &from, Direction::Down);
    assert_eq!(
        result,
        Some(Moniker::from_string("panel:task:T1A")),
        "Down from field:task:T1A.assignees must drill out to panel:task:T1A \
         (no peer at any inspector-layer level; cascade returns the parent zone)"
    );

    // Layer-boundary guard. The result above must NOT be any moniker
    // from the window layer — the inspector is captured-focus. Iterate
    // every window-layer registration and confirm the navigator's
    // answer doesn't appear in that set.
    let result_moniker = result.expect("step 3 returned Some(panel:task:T1A) above");
    let window_layer_key = swissarmyhammer_focus::LayerKey::from_string(WINDOW_LAYER_KEY);
    let window_monikers: Vec<&Moniker> = app
        .registry()
        .leaves_in_layer(&window_layer_key)
        .map(|f| &f.moniker)
        .chain(
            app.registry()
                .zones_in_layer(&window_layer_key)
                .map(|z| &z.moniker),
        )
        .collect();
    for m in &window_monikers {
        assert_ne!(
            &&result_moniker, m,
            "layer-boundary guard violated: navigator returned {m:?} which lives in the window \
             layer, but the focused entry was in the inspector layer",
        );
    }

    // Step 4: panel:task:T1A → None (the panel sits at the inspector
    // layer root with no siblings; escalation has nowhere to go without
    // crossing the layer boundary, which the cascade refuses to do).
    let from = key_for(&app, "panel:task:T1A");
    let result = nav(&app, &from, Direction::Down);
    assert_eq!(
        result, None,
        "Down from panel:task:T1A (alone at the inspector layer root) must return None — \
         the cascade refuses to cross the layer boundary into the window layer"
    );

    // Confirm the inspector layer key resolves the panel's expected
    // owning layer — a tripwire that catches fixture drift renaming
    // the inspector layer or moving the panel onto the window layer.
    let inspector_layer_key = swissarmyhammer_focus::LayerKey::from_string(INSPECTOR_LAYER_KEY);
    let panel_lives_in_inspector = app
        .registry()
        .zones_in_layer(&inspector_layer_key)
        .any(|z| z.moniker.as_str() == "panel:task:T1A");
    assert!(
        panel_lives_in_inspector,
        "fixture invariant: panel:task:T1A must register on the inspector layer"
    );
}
