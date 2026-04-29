//! Source-of-truth integration tests for **Left/Right arrow navigation
//! among the navbar's sibling entries** under the unified cascade.
//!
//! Built against the realistic-app fixture in `tests/fixtures/mod.rs`,
//! whose `ui:navbar` zone holds — left to right — the
//! `ui:navbar.board-selector` leaf, the `ui:navbar.inspect` leaf, the
//! `field:board:b1.percent_complete` field zone, and the
//! `ui:navbar.search` leaf.

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
// Right — horizontal advance through the navbar's leaf siblings.
// ---------------------------------------------------------------------------

/// Pressing `Right` from `ui:navbar.board-selector` (the leftmost leaf)
/// lands on `ui:navbar.inspect` — the next sibling leaf to its right.
#[test]
fn navbar_right_from_board_selector_lands_on_inspect() {
    let app = RealisticApp::new();
    let from = app.navbar_board_selector_fq();
    assert_eq!(
        nav(&app, &from, Direction::Right),
        app.navbar_inspect_fq(),
        "Right from ui:navbar.board-selector must land on ui:navbar.inspect \
         (in-zone leaf peer to the right)"
    );
}

/// Pressing `Right` from `ui:navbar.inspect` lands on
/// `ui:navbar.search` — the next sibling **leaf** to its right.
#[test]
fn navbar_right_from_inspect_lands_on_search() {
    let app = RealisticApp::new();
    let from = app.navbar_inspect_fq();
    assert_eq!(
        nav(&app, &from, Direction::Right),
        app.navbar_search_fq(),
        "Right from ui:navbar.inspect must land on ui:navbar.search \
         (next leaf to the right under the unified cascade's same-kind iter-0 filter; \
         the percent-complete field zone is skipped because cardinal nav stays within \
         the leaf kind — drill in to reach it)"
    );
}

/// Pressing `Right` from the percent-complete field **zone** drills
/// out to `ui:navbar`.
#[test]
fn navbar_right_from_percent_field_zone_drills_out_to_navbar() {
    let app = RealisticApp::new();
    let from = app.navbar_percent_field_fq();
    assert_eq!(
        nav(&app, &from, Direction::Right),
        app.navbar_fq(),
        "Right from field:board:b1.percent_complete must drill out to ui:navbar \
         (zone-only iter 0 has no sibling zones inside ui:navbar; iter 1's parent \
         ui:navbar sits at the layer root with no Right peer at that level; the \
         cascade falls back to the parent zone — the kernel never crosses kinds \
         via cardinal nav and never returns None when a parent zone exists)"
    );
}

// ---------------------------------------------------------------------------
// Left — horizontal retreat through the navbar's leaf siblings.
// ---------------------------------------------------------------------------

/// Pressing `Left` from `ui:navbar.search` (the rightmost leaf) lands
/// on `ui:navbar.inspect` — the next sibling **leaf** to its left.
#[test]
fn navbar_left_walks_symmetric_path() {
    let app = RealisticApp::new();

    // Step 1: search → inspect (skipping the field zone via same-kind
    // filter).
    let from = app.navbar_search_fq();
    assert_eq!(
        nav(&app, &from, Direction::Left),
        app.navbar_inspect_fq(),
        "Left from ui:navbar.search must land on ui:navbar.inspect \
         (the next leaf to the left under same-kind iter-0 filter; the percent-complete \
         field zone is skipped — symmetric to the Right case)"
    );

    // Step 2: inspect → board-selector (in-zone leaf peer to the left).
    let from = app.navbar_inspect_fq();
    assert_eq!(
        nav(&app, &from, Direction::Left),
        app.navbar_board_selector_fq(),
        "Left from ui:navbar.inspect must land on ui:navbar.board-selector \
         (in-zone leaf peer to the left)"
    );
}

/// Pressing `Right` from the rightmost leaf `ui:navbar.search` drills
/// out to `ui:navbar`.
#[test]
fn navbar_right_from_rightmost_leaf_drills_out_to_navbar() {
    let app = RealisticApp::new();
    let from = app.navbar_search_fq();
    let result = nav(&app, &from, Direction::Right);

    // No-bounce-back: the answer must not be any previous navbar
    // entry.
    let forbidden = [
        app.navbar_search_fq(),
        app.navbar_inspect_fq(),
        app.navbar_board_selector_fq(),
        app.navbar_percent_field_fq(),
    ];
    assert!(
        !forbidden.contains(&result),
        "Right from ui:navbar.search must not bounce back to a navbar entry, got {result:?}",
    );
    // Pin the specific drill-out outcome under the unified cascade.
    assert_eq!(
        result,
        app.navbar_fq(),
        "Right from ui:navbar.search must drill out to ui:navbar — iter 0 finds no \
         leaf peer right of search, iter 1's parent ui:navbar has no Right peer at \
         the layer root, and the cascade falls back to the parent zone itself rather \
         than returning None or bouncing back"
    );
}

// ---------------------------------------------------------------------------
// Sanity — fixture has the navbar shape we asserted on.
// ---------------------------------------------------------------------------

/// The fixture registers four entries inside `ui:navbar`: three leaves
/// and one field zone.
#[test]
fn fixture_navbar_has_three_leaves_and_one_field_zone() {
    let app = RealisticApp::new();

    let navbar_zone_fq = app.navbar_fq();

    let mut leaf_segments: Vec<String> = app
        .registry()
        .leaves_iter()
        .filter(|s| s.parent_zone.as_ref() == Some(&navbar_zone_fq))
        .map(|s| s.segment.as_str().to_string())
        .collect();
    leaf_segments.sort();
    assert_eq!(
        leaf_segments,
        vec![
            "ui:navbar.board-selector".to_string(),
            "ui:navbar.inspect".to_string(),
            "ui:navbar.search".to_string(),
        ],
        "fixture must register exactly three navbar leaves with the production segments"
    );

    let zone_segments: Vec<String> = app
        .registry()
        .zones_iter()
        .filter(|z| z.parent_zone.as_ref() == Some(&navbar_zone_fq))
        .map(|z| z.segment.as_str().to_string())
        .collect();
    assert_eq!(
        zone_segments,
        vec!["field:board:b1.percent_complete".to_string()],
        "fixture must register the percent-complete field as a zone child of ui:navbar"
    );
}
