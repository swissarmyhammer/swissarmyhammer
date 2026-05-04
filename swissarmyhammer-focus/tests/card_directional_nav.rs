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

/// Pressing `down` from the bottom card has nothing strictly in the
/// Down half-plane in the layer — the column zone, the board zone,
/// and other columns all extend above T3A's bottom edge or share it,
/// so the strict half-plane test filters them out. The geometric
/// pick stays put per the no-silent-dropout contract.
///
/// Pre-fix the structural cascade drilled out to `column:TODO`. The
/// new behaviour is correct: T3A is the visually bottom-most thing
/// in this region, so pressing Down has no real target.
#[test]
fn down_from_t3a_stays_put_at_visual_edge() {
    let app = RealisticApp::new();
    let from = app.card_fq(3, 0);
    assert_eq!(
        nav(&app, &from, Direction::Down),
        from,
        "down from task:T3A (bottom card) has nothing strictly in the Down \
         half-plane — the geometric pick stays put per the no-silent-dropout \
         contract."
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

/// Pressing `up` from the top card in column TODO lands on the
/// column-name field zone above it — both share `column:TODO` as
/// their `parent_zone`, so they are siblings under the new any-kind
/// iter-0 rule.
///
/// Pre-fix behaviour (now removed): iter 0 used a same-kind filter
/// that skipped the column-name `<FocusZone>` for a leaf-origin (card
/// scope) search; the cascade escalated and returned the parent zone
/// (`column:TODO`). The new contract: zones and scopes are siblings
/// under a parent zone, and iter 0 considers any-kind candidates. See
/// `swissarmyhammer-focus/README.md` for the prose contract and
/// `tests/in_zone_any_kind_first.rs` for the synthetic regression
/// suite.
#[test]
fn up_from_t1a_lands_on_column_name_zone() {
    let app = RealisticApp::new();
    let from = app.card_fq(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Up),
        app.column_name_fq(0),
        "Up from task:T1A (top card) must land on the column-name field zone \
         above it. They are siblings under the same parent zone (`column:TODO`); \
         the unified-cascade iter-0 rule considers any-kind in-zone candidates, \
         so the column-name zone is a valid peer for the card leaf."
    );
}

// ---------------------------------------------------------------------------
// Right — horizontal advance into the next column.
// ---------------------------------------------------------------------------

/// Pressing `right` from the top card in column TODO lands on the
/// top card in column DOING — the visually-adjacent card directly to
/// the right. T1A and T1B share the same y range, so T1B is in-beam
/// for `Right`; geometrically T1B's leading edge (left=488) and
/// matching y range produce a much smaller minor-axis distance than
/// the column-name field zone (which sits ABOVE T1A's row, so it is
/// out of beam) or the column zone (which has its center far below).
///
/// Under the geometric-pick contract this is the natural keyboard-
/// as-mouse answer: pressing Right from a card lands on the next
/// card in the next column rather than on the column header. Pre-fix
/// the structural cascade drilled into `column:DOING`'s natural
/// child via a cross-zone drill-in step that no longer exists.
#[test]
fn right_from_t1a_lands_on_t1b() {
    let app = RealisticApp::new();
    let from = app.card_fq(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Right),
        app.card_fq(1, 1),
        "right from task:T1A must land on task:T1B — the visually-adjacent \
         card to the right under the geometric pick. Both share the same y \
         range, so T1B is in-beam and beats the column-name zone (out of \
         beam) and the column zone (further by minor-axis distance)."
    );
}

/// Pressing `right` from any card in the rightmost column has
/// nothing strictly in the layer's Right half-plane — the column
/// zones, the board zone, and the navbar/perspective bar all share
/// the same right edge or extend leftward. Under the geometric-pick
/// contract this is the "stay-put at the visual edge" path.
///
/// Pre-fix the structural cascade drilled out to `column:DONE`. The
/// new behaviour is correct: T1C is the visually rightmost focusable
/// thing in its row.
#[test]
fn right_from_t1c_stays_put_at_visual_edge() {
    let app = RealisticApp::new();
    let from = app.card_fq(1, 2);
    assert_eq!(
        nav(&app, &from, Direction::Right),
        from,
        "right from task:T1C (rightmost column) has nothing strictly in the \
         Right half-plane — the geometric pick stays put per the \
         no-silent-dropout contract."
    );
}

// ---------------------------------------------------------------------------
// Left — horizontal retreat into the previous column.
// ---------------------------------------------------------------------------

/// Pressing `left` from the top card in column DOING lands on the
/// top card in column TODO — the visually-adjacent card directly to
/// the left. Symmetric to `right_from_t1a_lands_on_t1b`.
///
/// Under the geometric pick this is the natural keyboard-as-mouse
/// answer. Pre-fix the structural cascade drilled into `column:TODO`'s
/// natural child via a cross-zone drill-in step that no longer
/// exists.
#[test]
fn left_from_t1b_lands_on_t1a() {
    let app = RealisticApp::new();
    let from = app.card_fq(1, 1);
    assert_eq!(
        nav(&app, &from, Direction::Left),
        app.card_fq(1, 0),
        "left from task:T1B must land on task:T1A — the visually-adjacent \
         card to the left under the geometric pick. Symmetric to \
         `right_from_t1a_lands_on_t1b`."
    );
}

/// Pressing `left` from the top card in the leftmost column lands
/// inside `ui:left-nav` — the LeftNav sidebar visible to the left of
/// the board. Under the geometric pick the LeftNav zone (and its
/// view-button leaves) are the only scopes strictly in the Left
/// half-plane that are also in-beam vertically.
///
/// This is the cross-zone bug class the redesign fixes — pressing
/// Left from a leftmost card lands on the visually-adjacent surface
/// across structural boundaries. Pre-fix the structural cascade
/// drilled out to `column:TODO` (the parent zone) because the
/// LeftNav was unreachable through the iter-0/iter-1 ladder.
#[test]
fn left_from_t1a_lands_in_left_nav() {
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
        "left from task:T1A (leftmost column) must land inside ui:left-nav \
         under the geometric pick. Got {result:?}; expected one of \
         {acceptable:?}.",
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
