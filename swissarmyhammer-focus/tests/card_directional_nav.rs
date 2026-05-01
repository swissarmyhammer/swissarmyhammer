//! Source-of-truth integration tests for **directional navigation from a
//! focused card** in the spatial-nav kernel, under the **unified
//! cascade** policy.
//!
//! Built against the realistic-app fixture in `tests/fixtures/mod.rs`,
//! which constructs a [`SpatialRegistry`] whose zone/leaf graph mirrors
//! what `kanban-app/ui/src/components/board-view.tsx` and
//! `kanban-app/ui/src/components/column-view.tsx` mount at runtime. Each
//! test focuses one card and asserts the FQM the navigator returns
//! for one direction key.
//!
//! # Test cases
//!
//! User-trajectory cases covering all four directions:
//!
//! - **Down**: T1A → T2A, T2A → T3A, T3A → drill-out to `column:TODO`.
//! - **Up**: T2A → T1A, T1A → `column:TODO.name`.
//! - **Right**: T1A → `column:DOING` (zone), T1C → drill-out to
//!   `column:DONE`.
//! - **Left**: T1B → `column:TODO` (zone), T1A → drill-out to
//!   `column:TODO`.

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
// Down — vertical advance through cards stacked in a column.
// ---------------------------------------------------------------------------

/// Pressing `down` from the top card in column TODO advances to the
/// second card.
#[test]
fn down_from_t1a_lands_on_t2a() {
    let app = RealisticApp::new();
    let from = app.card_fq(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Down),
        app.card_fq(2, 0),
        "down from task:T1A must land on task:T2A (iter 0, in-zone peer)"
    );
}

/// Pressing `down` from the middle card in column TODO advances to the
/// bottom card.
#[test]
fn down_from_t2a_lands_on_t3a() {
    let app = RealisticApp::new();
    let from = app.card_fq(2, 0);
    assert_eq!(
        nav(&app, &from, Direction::Down),
        app.card_fq(3, 0),
        "down from task:T2A must land on task:T3A (iter 0, in-zone peer)"
    );
}

/// Pressing `down` from the bottom card in a column has no in-zone
/// peer below; the unified cascade escalates to `column:TODO` and
/// finds no zone peer below either. The cascade falls back to the
/// parent zone via drill-out and returns `column:TODO`.
#[test]
fn down_from_t3a_drills_out_to_column_zone() {
    let app = RealisticApp::new();
    let from = app.card_fq(3, 0);
    assert_eq!(
        nav(&app, &from, Direction::Down),
        app.column_fq(0),
        "down from task:T3A (bottom card) must drill out to column:TODO under the unified cascade"
    );
}

// ---------------------------------------------------------------------------
// Up — vertical retreat back to the column header.
// ---------------------------------------------------------------------------

/// Pressing `up` from the middle card in column TODO retreats to the
/// top card.
#[test]
fn up_from_t2a_lands_on_t1a() {
    let app = RealisticApp::new();
    let from = app.card_fq(2, 0);
    assert_eq!(
        nav(&app, &from, Direction::Up),
        app.card_fq(1, 0),
        "up from task:T2A must land on task:T1A (iter 0, in-zone peer)"
    );
}

/// Pressing `up` from the top card in column TODO drills out to the
/// column zone.
///
/// The column-name surface is a `<FocusZone>` (kind `Zone`) — its
/// same-kind filter at iter 0 skips it for a leaf-origin (card scope)
/// search. The cascade escalates and falls back to the parent zone
/// (`column:TODO`). To reach the column-name zone, the user drills out
/// to `column:TODO` first and then presses `Down` — see
/// `tests/column_header_arrow_nav.rs` for the symmetric cases.
#[test]
fn up_from_t1a_drills_out_to_column_zone() {
    let app = RealisticApp::new();
    let from = app.card_fq(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Up),
        app.column_fq(0),
        "up from task:T1A (top card) must drill out to column:TODO \
         (the column-name field zone above is filtered out by iter 0's \
         same-kind leaf filter; the cascade escalates and returns the \
         parent zone)"
    );
}

// ---------------------------------------------------------------------------
// Right — horizontal advance into the next column.
// ---------------------------------------------------------------------------

/// Pressing `right` from the top card in column TODO advances onto
/// column DOING's zone.
#[test]
fn right_from_t1a_lands_on_column_doing_zone() {
    let app = RealisticApp::new();
    let from = app.card_fq(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Right),
        app.column_fq(1),
        "right from task:T1A must land on column:DOING (peer at the parent's level under the \
         unified cascade)"
    );
}

/// Pressing `right` from any card in the rightmost column has no
/// in-zone peer; the unified cascade escalates to `column:DONE` and
/// finds no zone peer right of it. The cascade drills out and returns
/// `column:DONE`.
#[test]
fn right_from_t1c_drills_out_to_column_done_zone() {
    let app = RealisticApp::new();
    let from = app.card_fq(1, 2);
    assert_eq!(
        nav(&app, &from, Direction::Right),
        app.column_fq(2),
        "right from task:T1C (rightmost column) must drill out to column:DONE under the unified \
         cascade"
    );
}

// ---------------------------------------------------------------------------
// Left — horizontal retreat into the previous column.
// ---------------------------------------------------------------------------

/// Pressing `left` from the top card in column DOING retreats onto
/// column TODO's zone.
#[test]
fn left_from_t1b_lands_on_column_todo_zone() {
    let app = RealisticApp::new();
    let from = app.card_fq(1, 1);
    assert_eq!(
        nav(&app, &from, Direction::Left),
        app.column_fq(0),
        "left from task:T1B must land on column:TODO (peer at the parent's level under the \
         unified cascade)"
    );
}

/// Pressing `left` from any card in the leftmost column has no
/// in-zone peer; the unified cascade escalates to `column:TODO` and
/// finds no zone peer to its left. The cascade drills out and returns
/// `column:TODO`.
#[test]
fn left_from_t1a_drills_out_to_column_todo_zone() {
    let app = RealisticApp::new();
    let from = app.card_fq(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Left),
        app.column_fq(0),
        "left from task:T1A (leftmost column) must drill out to column:TODO under the unified \
         cascade — the kernel never returns None when a parent zone exists in the same layer"
    );
}

// ---------------------------------------------------------------------------
// Sanity — fixture has the shape we asserted on.
// ---------------------------------------------------------------------------

/// The fixture registers nine cards across three columns with the
/// expected `task:T<row><letter>` segments. Used as a tripwire against
/// future fixture edits that would invalidate the directional tests
/// silently.
#[test]
fn fixture_has_nine_cards_across_three_columns() {
    let app = RealisticApp::new();
    let mut card_segments: Vec<String> = app
        .registry()
        .leaves_iter()
        .filter(|f| f.segment.as_str().starts_with("task:"))
        .map(|f| f.segment.as_str().to_string())
        .collect();
    card_segments.sort();
    let expected: Vec<String> = (1..=3)
        .flat_map(|row| {
            ['A', 'B', 'C']
                .iter()
                .map(move |c| format!("task:T{row}{c}"))
        })
        .collect();
    let mut expected_sorted = expected.clone();
    expected_sorted.sort();
    assert_eq!(
        card_segments, expected_sorted,
        "fixture must register exactly nine cards across columns A, B, C and rows 1, 2, 3"
    );
}
