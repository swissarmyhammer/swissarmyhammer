//! Source-of-truth integration tests for **arrow navigation between
//! the column body, the column-name field zone, and the topmost card
//! scope** under the unified cascade.
//!
//! Built against the realistic-app fixture in `tests/fixtures/mod.rs`.
//! After card `01KQAWVDS931PADB0559F2TVCS`, the column-name surface is
//! registered as a `<FocusZone>` (kind `Zone`) with moniker
//! `field:column:<NAME>.name` parented at the column zone — collapsing
//! the previous synthetic outer `<FocusScope moniker="column:<id>.name">`
//! that was a leaf duplicating the inner Field zone.
//!
//! These tests pin the post-refactor trajectory under the **any-kind
//! iter-0 sibling rule** (zones and scopes are siblings under a parent
//! zone — see `swissarmyhammer-focus/README.md` and
//! `tests/in_zone_any_kind_first.rs` for the contract):
//!
//!   - `Up` from the topmost card scope lands on the column-name field
//!     zone above it — the two share `column:TODO` as their parent
//!     zone, so iter 0 considers them peers regardless of kind.
//!   - `Down` from the column zone drills out to `ui:board` — cardinal
//!     nav from a zone walks sibling zones, not descendants, so the
//!     column-name field zone and the card scopes (both children of
//!     `column:TODO`) do not enter the search at the column zone's
//!     level.
//!
//! [`01KQAWVDS931PADB0559F2TVCS`]: # "column-header redundancy collapse"

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
// Up — leaf-origin cardinal nav from the topmost card.
// ---------------------------------------------------------------------------

/// Pressing `Up` from the topmost card in `column:TODO` lands on the
/// column-name field zone above it.
///
/// The card is a leaf scope; the sibling above it is the column-name
/// field zone (`field:column:TODO.name`) — both share `column:TODO`
/// as their `parent_zone`. Under the any-kind iter-0 rule the kernel
/// considers them peers and picks the column-name zone (the geometric
/// best Up candidate).
#[test]
fn up_from_topmost_card_lands_on_column_name_zone() {
    let app = RealisticApp::new();
    let from = app.card_fq(1, 0);
    let landing = nav(&app, &from, Direction::Up);

    assert_eq!(
        landing,
        app.column_name_fq(0),
        "Up from task:T1A must land on the column-name field zone \
         (`field:column:TODO.name`) above it — they are siblings under the \
         same parent zone (`column:TODO`), and the unified-cascade iter-0 \
         rule considers any-kind in-zone candidates."
    );
}

/// The result is consistent across all three columns (TODO, DOING,
/// DONE) — there is no special-case in the leftmost or rightmost
/// column.
#[test]
fn up_from_topmost_card_is_consistent_across_columns() {
    let app = RealisticApp::new();
    for col in 0..3 {
        let from = app.card_fq(1, col);
        let landing = nav(&app, &from, Direction::Up);
        assert_eq!(
            landing,
            app.column_name_fq(col),
            "Up from task:T1{col} must land on its column-name field zone \
             (column index {col})",
            col = col,
        );
    }
}

// ---------------------------------------------------------------------------
// Down — zone-origin cardinal nav from the column zone.
// ---------------------------------------------------------------------------

/// Pressing `Down` from the column zone has nothing strictly in the
/// Down half-plane — the column zone shares its bottom edge with
/// `ui:board` and with the other column zones (`column:TODO.bottom ==
/// ui:board.bottom == 900`), so under the strict half-plane test no
/// candidate qualifies. The geometric pick echoes the focused FQM
/// (stay-put).
///
/// Pre-fix the structural cascade escalated to `ui:board` and drilled
/// out. The new behaviour is correct: `column:TODO` is at the visual
/// bottom of the layer in this fixture, so pressing Down has no real
/// target. The user reaches the column's descendants via `Enter`
/// (drill in), not via `Down`.
#[test]
fn down_from_column_zone_stays_put_at_visual_edge() {
    let app = RealisticApp::new();
    let from = app.column_fq(0);
    let landing = nav(&app, &from, Direction::Down);

    assert_eq!(
        landing, from,
        "Down from column:TODO has no scope strictly in the Down half-plane \
         (the column shares its bottom edge with ui:board and the other \
         columns), so the geometric pick stays put per the no-silent- \
         dropout contract."
    );
}
