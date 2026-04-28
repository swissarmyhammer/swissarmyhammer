//! Source-of-truth integration tests for **directional navigation from a
//! focused card** in the spatial-nav kernel, under the **unified
//! cascade** policy.
//!
//! Built against the realistic-app fixture in `tests/fixtures/mod.rs`,
//! which constructs a [`SpatialRegistry`] whose zone/leaf graph mirrors
//! what `kanban-app/ui/src/components/board-view.tsx` and
//! `kanban-app/ui/src/components/column-view.tsx` mount at runtime. Each
//! test focuses one card and asserts the moniker the navigator returns
//! for one direction key — the user-visible answer when they press
//! `j` / `k` / `h` / `l` (vim), `ArrowDown` / `ArrowUp` / `ArrowLeft` /
//! `ArrowRight` (cua), or `Ctrl-N` / `Ctrl-P` / `Ctrl-B` / `Ctrl-F`
//! (emacs). The keymap → direction-string mapping happens in React; the
//! kernel just receives the direction.
//!
//! # Why these tests live in Rust
//!
//! The directional-nav supersession card
//! [`01KQ7STZN3G5N2WB3FF4PM4DKX`] explained the decision: the prior
//! cross-column card landed nine green browser tests that used a JS
//! shadow registry mirroring the kernel — and missed the user-reported
//! bug because the JS port and the Rust kernel disagreed in a way the
//! test could not catch. Building the realistic state in Rust and
//! calling [`BeamNavStrategy::next`] directly removes the mimicry
//! layer; the kernel + the production registry shape are the things
//! under test, and they live in Rust, so test them there.
//!
//! # The unified cascade these tests pin
//!
//! The unified-policy supersession card
//! [`01KQ7S6WHK9RCCG2R4FN474EFD`] replaced the per-direction tactical
//! rules with a single two-level cascade:
//!
//! 1. **Iter 0** searches scopes sharing the focused entry's
//!    `parent_zone` (in-zone peers) for an in-beam match. Down inside
//!    a column, Up to the column header — these are iter-0 wins.
//! 2. **Iter 1** escalates to the focused entry's `parent_zone` and
//!    searches at the parent's level. Right from a card lands on the
//!    next-column zone moniker; the React adapter handles drill-back-
//!    in if a specific leaf inside the destination zone is desired.
//! 3. **Drill-out** returns the parent zone itself when neither iter
//!    finds a peer. Left from a card in the leftmost column drills
//!    out to the column zone — the kernel never gets stuck on a
//!    `None` answer when the focused entry has a parent zone to fall
//!    back to.
//!
//! Cross-column tests below assert on the *zone* moniker (e.g.
//! `column:DOING`, `column:TODO`) rather than on a card-leaf moniker
//! in the destination column — the latter was the old rule-2 cross-
//! zone leaf fallback that the unified cascade superseded.
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
//!
//! Future card-nav bugs append a `#[test]` here. They do **not** open
//! per-direction cards — the test surface is unified.
//!
//! [`01KQ7STZN3G5N2WB3FF4PM4DKX`]: # "directional-nav supersession card"
//! [`01KQ7S6WHK9RCCG2R4FN474EFD`]: # "unified-policy supersession card"

mod fixtures;

use swissarmyhammer_focus::{BeamNavStrategy, Direction, Moniker, NavStrategy, SpatialKey};

use fixtures::RealisticApp;

/// Convenience: run [`BeamNavStrategy::next`] against the fixture's
/// registry from the named [`SpatialKey`] in the named [`Direction`].
/// Centralised so each test reads top-to-bottom without repeating the
/// boilerplate.
fn nav(app: &RealisticApp, from: &SpatialKey, dir: Direction) -> Option<Moniker> {
    BeamNavStrategy::new().next(app.registry(), from, dir)
}

// ---------------------------------------------------------------------------
// Down — vertical advance through cards stacked in a column.
// ---------------------------------------------------------------------------

/// Pressing `down` from the top card in column TODO advances to the
/// second card. Iter 0 (in-zone peer search) fires: T2A is a sibling
/// leaf inside the same `column:TODO` zone, vertically below T1A,
/// in-beam on the horizontal axis.
#[test]
fn down_from_t1a_lands_on_t2a() {
    let app = RealisticApp::new();
    let from = app.card_key(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Down),
        Some(Moniker::from_string("task:T2A")),
        "down from task:T1A must land on task:T2A (iter 0, in-zone peer)"
    );
}

/// Pressing `down` from the middle card in column TODO advances to the
/// bottom card. Same iter-0 in-zone peer search, one slot further down.
#[test]
fn down_from_t2a_lands_on_t3a() {
    let app = RealisticApp::new();
    let from = app.card_key(2, 0);
    assert_eq!(
        nav(&app, &from, Direction::Down),
        Some(Moniker::from_string("task:T3A")),
        "down from task:T2A must land on task:T3A (iter 0, in-zone peer)"
    );
}

/// Pressing `down` from the bottom card in a column has no in-zone
/// peer below; the unified cascade escalates to `column:TODO` and
/// finds no zone peer below either (every column shares the same
/// vertical extent). The cascade falls back to the parent zone via
/// drill-out and returns `column:TODO`.
///
/// Pre-supersession this test asserted on `None` — the old rule-3
/// no-op termination of the per-direction cascade. Under the unified
/// cascade `None` is reserved for the focused entry sitting at the
/// very root of its layer; a key press never gets stuck on a leaf
/// that has a parent zone to drill out to.
#[test]
fn down_from_t3a_drills_out_to_column_zone() {
    let app = RealisticApp::new();
    let from = app.card_key(3, 0);
    assert_eq!(
        nav(&app, &from, Direction::Down),
        Some(Moniker::from_string("column:TODO")),
        "down from task:T3A (bottom card) must drill out to column:TODO under the unified cascade"
    );
}

// ---------------------------------------------------------------------------
// Up — vertical retreat back to the column header.
// ---------------------------------------------------------------------------

/// Pressing `up` from the middle card in column TODO retreats to the
/// top card. Mirror of `down_from_t1a_lands_on_t2a` — iter 0 in-zone
/// peer, same axis, opposite direction.
#[test]
fn up_from_t2a_lands_on_t1a() {
    let app = RealisticApp::new();
    let from = app.card_key(2, 0);
    assert_eq!(
        nav(&app, &from, Direction::Up),
        Some(Moniker::from_string("task:T1A")),
        "up from task:T2A must land on task:T1A (iter 0, in-zone peer)"
    );
}

/// Pressing `up` from the top card in column TODO retreats to the
/// column-name leaf in the column header. The header leaf
/// (`column:TODO.name`) lives inside the same `column:TODO` zone with
/// a rect directly above the top card; iter 0 picks it up as the
/// in-beam candidate.
///
/// This case is what the now-superseded card
/// `01KQ7RR4MJJPVTTB7GS6RKRH9E` was chasing: after the cross-column
/// fix, vertical nav from a top-row card needs to land *somewhere*
/// visible, and the column header is the natural target — focus does
/// not silently drop to nothing.
#[test]
fn up_from_t1a_lands_on_column_header() {
    let app = RealisticApp::new();
    let from = app.card_key(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Up),
        Some(Moniker::from_string("column:TODO.name")),
        "up from task:T1A (top card) must land on the column-name header leaf"
    );
}

// ---------------------------------------------------------------------------
// Right — horizontal advance into the next column.
// ---------------------------------------------------------------------------

/// Pressing `right` from the top card in column TODO advances onto
/// column DOING's zone. Iter 0 (peer search inside `column:TODO`)
/// finds nothing — every sibling inside the column is stacked above
/// or below T1A. Iter 1 escalates to `column:TODO` and finds
/// `column:DOING` as the in-beam right peer at the parent's level.
///
/// Pre-supersession this test asserted on `task:T1B` — the leaf
/// inside the next column, the answer the old rule-2 cross-zone leaf
/// fallback produced. Under the unified cascade the kernel's answer
/// is the next-column zone moniker; the React adapter handles drill-
/// back-in if the consumer wants a specific card.
#[test]
fn right_from_t1a_lands_on_column_doing_zone() {
    let app = RealisticApp::new();
    let from = app.card_key(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Right),
        Some(Moniker::from_string("column:DOING")),
        "right from task:T1A must land on column:DOING (peer at the parent's level under the \
         unified cascade)"
    );
}

/// Pressing `right` from any card in the rightmost column has no
/// in-zone peer; the unified cascade escalates to `column:DONE` and
/// finds no zone peer right of it (DONE is the rightmost column under
/// `ui:board`). The cascade drills out and returns `column:DONE`.
///
/// Pre-supersession this test asserted on `None` — the old rule-3
/// no-op termination. Under the unified cascade `None` is reserved
/// for the focused entry sitting at the very root of its layer.
#[test]
fn right_from_t1c_drills_out_to_column_done_zone() {
    let app = RealisticApp::new();
    let from = app.card_key(1, 2);
    assert_eq!(
        nav(&app, &from, Direction::Right),
        Some(Moniker::from_string("column:DONE")),
        "right from task:T1C (rightmost column) must drill out to column:DONE under the unified \
         cascade"
    );
}

// ---------------------------------------------------------------------------
// Left — horizontal retreat into the previous column.
// ---------------------------------------------------------------------------

/// Pressing `left` from the top card in column DOING retreats onto
/// column TODO's zone. Mirror of [`right_from_t1a_lands_on_column_doing_zone`]
/// — iter 0 finds no in-zone peer, iter 1 escalates to `column:DOING`
/// and finds `column:TODO` as the in-beam left peer.
///
/// Pre-supersession this test asserted on `task:T1A` (a leaf in the
/// previous column) via the old rule-2 cross-zone leaf fallback.
#[test]
fn left_from_t1b_lands_on_column_todo_zone() {
    let app = RealisticApp::new();
    let from = app.card_key(1, 1);
    assert_eq!(
        nav(&app, &from, Direction::Left),
        Some(Moniker::from_string("column:TODO")),
        "left from task:T1B must land on column:TODO (peer at the parent's level under the \
         unified cascade)"
    );
}

/// Pressing `left` from any card in the leftmost column has no
/// in-zone peer; the unified cascade escalates to `column:TODO` and
/// finds no zone peer to its left (TODO is the leftmost column under
/// `ui:board`). The cascade drills out and returns `column:TODO`.
///
/// This is the user-corrected trajectory C from the unified-policy
/// supersession card: pressing left from a card in the leftmost column
/// must NOT return `None` — `None` is reserved for the focused entry
/// sitting at the very root of its layer. The drill-out fallback
/// keeps the user from dead-ending on a key press.
#[test]
fn left_from_t1a_drills_out_to_column_todo_zone() {
    let app = RealisticApp::new();
    let from = app.card_key(1, 0);
    assert_eq!(
        nav(&app, &from, Direction::Left),
        Some(Moniker::from_string("column:TODO")),
        "left from task:T1A (leftmost column) must drill out to column:TODO under the unified \
         cascade — the kernel never returns None when a parent zone exists in the same layer"
    );
}

// ---------------------------------------------------------------------------
// Sanity — fixture has the shape we asserted on.
//
// Light-touch verification that the realistic-app builder produces what
// the tests above assume. If the fixture drifts (e.g. a renamed moniker,
// a dropped column), this test surfaces the drift before the directional
// assertions degrade into harder-to-read failures.
// ---------------------------------------------------------------------------

/// The fixture registers nine cards across three columns with the
/// expected `task:T<row><letter>` monikers. Used as a tripwire against
/// future fixture edits that would invalidate the directional tests
/// silently.
#[test]
fn fixture_has_nine_cards_across_three_columns() {
    let app = RealisticApp::new();
    let mut card_monikers: Vec<String> = app
        .registry()
        .leaves_iter()
        .filter(|f| f.moniker.as_str().starts_with("task:"))
        .map(|f| f.moniker.as_str().to_string())
        .collect();
    card_monikers.sort();
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
        card_monikers, expected_sorted,
        "fixture must register exactly nine cards across columns A, B, C and rows 1, 2, 3"
    );
}
