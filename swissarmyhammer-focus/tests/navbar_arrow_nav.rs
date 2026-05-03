//! Source-of-truth integration tests for **Left/Right arrow navigation
//! among the navbar's sibling entries** under the unified cascade with
//! the **any-kind iter-0 sibling rule**.
//!
//! Built against the realistic-app fixture in `tests/fixtures/mod.rs`,
//! whose `ui:navbar` zone holds — left to right — the
//! `ui:navbar.board-selector` leaf, the `ui:navbar.inspect` leaf, the
//! `field:board:b1.percent_complete` field zone, and the
//! `ui:navbar.search` leaf.
//!
//! Under the sibling rule (`zones and scopes are siblings under a
//! parent zone`), the percent-complete field zone is a peer of the
//! navbar's leaf entries — Left/Right walks through it like any other
//! sibling. See `swissarmyhammer-focus/README.md` for the prose
//! contract and `tests/in_zone_any_kind_first.rs` for the synthetic
//! regression suite.

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

/// Pressing `Right` from `ui:navbar.inspect` lands on the
/// percent-complete field **zone** to its right — the next in-zone
/// sibling, regardless of kind.
///
/// Under the any-kind iter-0 sibling rule, the percent-complete field
/// zone (a `<FocusZone>` inside `ui:navbar`) is a peer of the leaf
/// scopes around it. `inspect` (right edge x=296) sees percent (left
/// x=304) as the geometrically closest Right candidate, beating
/// `search` (left x=1200) on distance.
#[test]
fn navbar_right_from_inspect_lands_on_percent_field_zone() {
    let app = RealisticApp::new();
    let from = app.navbar_inspect_fq();
    assert_eq!(
        nav(&app, &from, Direction::Right),
        app.navbar_percent_field_fq(),
        "Right from ui:navbar.inspect must land on the percent-complete \
         field zone (any-kind in-zone sibling under ui:navbar; the field zone \
         is geometrically closer than ui:navbar.search). Pre-fix this used to \
         skip the field zone via a same-kind iter-0 filter; the new contract \
         (zones and scopes are siblings) treats them as peers."
    );
}

/// Pressing `Right` from the percent-complete field **zone** lands on
/// `ui:navbar.search` — the next in-zone sibling to its right under
/// the any-kind iter-0 rule. Pre-fix the same-kind filter blocked
/// leaf candidates from a zone-origin search and this drilled out to
/// `ui:navbar`; the new contract makes the leaf a valid peer.
#[test]
fn navbar_right_from_percent_field_zone_lands_on_search() {
    let app = RealisticApp::new();
    let from = app.navbar_percent_field_fq();
    assert_eq!(
        nav(&app, &from, Direction::Right),
        app.navbar_search_fq(),
        "Right from field:board:b1.percent_complete must land on \
         ui:navbar.search (any-kind in-zone sibling — both share ui:navbar \
         as their parent_zone, so the leaf is a valid peer of the field zone)"
    );
}

// ---------------------------------------------------------------------------
// Left — horizontal retreat through the navbar's leaf siblings.
// ---------------------------------------------------------------------------

/// Walking Left through the navbar: `search → percent-field-zone →
/// inspect → board-selector`. Under the any-kind iter-0 rule the
/// percent-complete field zone is a peer of the leaf siblings around
/// it; the walk visits it on the way through.
#[test]
fn navbar_left_walks_symmetric_path() {
    let app = RealisticApp::new();

    // Step 1: search → percent-field-zone (any-kind in-zone Left peer
    // — the zone is geometrically closer than the inspect leaf).
    let from = app.navbar_search_fq();
    assert_eq!(
        nav(&app, &from, Direction::Left),
        app.navbar_percent_field_fq(),
        "Left from ui:navbar.search must land on the percent-complete field zone \
         (any-kind in-zone sibling — geometrically closer than the leaves further left). \
         Symmetric to Right from inspect under the new sibling rule."
    );

    // Step 2: percent-field-zone → inspect (any-kind in-zone Left peer).
    let from = app.navbar_percent_field_fq();
    assert_eq!(
        nav(&app, &from, Direction::Left),
        app.navbar_inspect_fq(),
        "Left from the percent-complete field zone must land on ui:navbar.inspect \
         (any-kind in-zone sibling)"
    );

    // Step 3: inspect → board-selector (in-zone peer to the left).
    let from = app.navbar_inspect_fq();
    assert_eq!(
        nav(&app, &from, Direction::Left),
        app.navbar_board_selector_fq(),
        "Left from ui:navbar.inspect must land on ui:navbar.board-selector \
         (in-zone peer to the left)"
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
