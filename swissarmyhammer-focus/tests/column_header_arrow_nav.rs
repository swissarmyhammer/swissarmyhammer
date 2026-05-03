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

/// Pressing `Down` from the column zone drills out to `ui:board`.
///
/// Under the unified cascade, cardinal nav from a zone searches sibling
/// zones at the same level (iter 0) and then escalates to the parent
/// (iter 1). The column-name field zone and the card scopes are
/// **descendants** of `column:TODO`, not siblings — so they do not enter
/// the cardinal-nav search. With no Down peer at the column-zone level
/// inside `ui:board`, the cascade falls back to the parent (`ui:board`).
///
/// This is the symmetric counterpart to the navbar's percent-complete
/// precedent: `Down` from a parent zone does not drill into children;
/// children are reached via `Enter` (the React adapter's drill-in).
/// The user-facing experience: arrow keys move *between* peer zones,
/// `Enter` moves *into* a zone's descendants.
///
/// If a future tweak adds a "drill into column zone via Down" rule, this
/// assertion must change — and a follow-up must justify the cascade
/// change rather than smuggling it into the redundancy-collapse fix.
#[test]
fn down_from_column_zone_drills_out_to_board() {
    let app = RealisticApp::new();
    let from = app.column_fq(0);
    let landing = nav(&app, &from, Direction::Down);

    assert_eq!(
        landing,
        app.board_fq(),
        "Down from column:TODO must drill out to ui:board. Cardinal nav \
         from a zone searches sibling zones, not descendants — the \
         column-name field zone and the card scopes are children of \
         column:TODO, so they do not satisfy the same-level peer search. \
         With no Down peer at ui:board's level the cascade falls back to \
         the parent zone (ui:board itself). The user reaches the \
         column-name zone via `Enter` (drill in), not via `Down`."
    );
}
