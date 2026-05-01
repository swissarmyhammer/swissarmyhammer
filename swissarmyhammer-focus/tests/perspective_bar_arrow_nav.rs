//! Source-of-truth integration tests for **Left/Right arrow navigation
//! among the perspective bar's sibling tab leaves** under the unified
//! cascade.
//!
//! Built against the realistic-app fixture in `tests/fixtures/mod.rs`,
//! whose `ui:perspective-bar` zone holds three `perspective_tab:{id}`
//! leaves laid out left-to-right.

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
// Right — horizontal advance through the perspective bar's tab leaves.
// ---------------------------------------------------------------------------

/// Pressing `Right` from `perspective_tab:p1` (the leftmost tab) lands
/// on `perspective_tab:p2`.
#[test]
fn perspective_right_from_leftmost_tab_lands_on_middle_tab() {
    let app = RealisticApp::new();
    let from = app.perspective_tab_p1_fq();
    assert_eq!(
        nav(&app, &from, Direction::Right),
        app.perspective_tab_p2_fq(),
        "Right from perspective_tab:p1 must land on perspective_tab:p2 \
         (in-zone leaf peer to the right)"
    );
}

/// Pressing `Right` from the wider middle tab `perspective_tab:p2`
/// lands on `perspective_tab:p3`.
#[test]
fn perspective_right_from_middle_active_tab_lands_on_rightmost_tab() {
    let app = RealisticApp::new();
    let from = app.perspective_tab_p2_fq();
    assert_eq!(
        nav(&app, &from, Direction::Right),
        app.perspective_tab_p3_fq(),
        "Right from perspective_tab:p2 must land on perspective_tab:p3 \
         (in-zone leaf peer to the right; the wider middle-tab rect does \
         not break left-edge ordering)"
    );
}

// ---------------------------------------------------------------------------
// Left — horizontal retreat through the perspective bar's tab leaves.
// ---------------------------------------------------------------------------

/// Pressing `Left` from `perspective_tab:p3` walks the symmetric path
/// back to `perspective_tab:p1`.
#[test]
fn perspective_left_walks_symmetric_path() {
    let app = RealisticApp::new();

    // Step 1: p3 → p2.
    let from = app.perspective_tab_p3_fq();
    assert_eq!(
        nav(&app, &from, Direction::Left),
        app.perspective_tab_p2_fq(),
        "Left from perspective_tab:p3 must land on perspective_tab:p2 \
         (in-zone leaf peer to the left)"
    );

    // Step 2: p2 → p1.
    let from = app.perspective_tab_p2_fq();
    assert_eq!(
        nav(&app, &from, Direction::Left),
        app.perspective_tab_p1_fq(),
        "Left from perspective_tab:p2 must land on perspective_tab:p1 \
         (in-zone leaf peer to the left)"
    );
}

// ---------------------------------------------------------------------------
// Right from the rightmost tab — drill-out per unified policy.
// ---------------------------------------------------------------------------

/// Pressing `Right` from the rightmost tab `perspective_tab:p3` drills
/// out to `ui:perspective-bar`.
#[test]
fn perspective_right_from_rightmost_tab_drills_out_to_perspective_bar() {
    let app = RealisticApp::new();
    let from = app.perspective_tab_p3_fq();
    let result = nav(&app, &from, Direction::Right);

    // No-bounce-back: result must not be any previous perspective tab.
    let forbidden = [
        app.perspective_tab_p1_fq(),
        app.perspective_tab_p2_fq(),
        app.perspective_tab_p3_fq(),
    ];
    assert!(
        !forbidden.contains(&result),
        "Right from perspective_tab:p3 must not bounce back to a previous tab, got {result:?}",
    );

    // Pin the specific drill-out outcome under the unified cascade.
    assert_eq!(
        result,
        app.perspective_bar_fq(),
        "Right from perspective_tab:p3 must drill out to ui:perspective-bar — iter 0 \
         finds no leaf peer right of p3, iter 1's parent ui:perspective-bar has no \
         Right peer at the layer root, and the cascade falls back to the parent zone \
         itself rather than returning None or bouncing back"
    );
}

// ---------------------------------------------------------------------------
// Sanity — fixture has the perspective-bar shape we asserted on.
// ---------------------------------------------------------------------------

/// The fixture registers exactly three perspective tab leaves inside
/// `ui:perspective-bar`.
#[test]
fn fixture_perspective_bar_has_three_tab_leaves() {
    let app = RealisticApp::new();

    let bar_zone_fq = app.perspective_bar_fq();

    let mut tab_segments: Vec<String> = app
        .registry()
        .leaves_iter()
        .filter(|s| s.parent_zone.as_ref() == Some(&bar_zone_fq))
        .map(|s| s.segment.as_str().to_string())
        .collect();
    tab_segments.sort();
    assert_eq!(
        tab_segments,
        vec![
            "perspective_tab:p1".to_string(),
            "perspective_tab:p2".to_string(),
            "perspective_tab:p3".to_string(),
        ],
        "fixture must register exactly three perspective tab leaves with the production \
         perspective_tab:{{id}} segment shape"
    );

    // No zone children inside the perspective bar — the bar holds tab
    // leaves only.
    let zone_segments: Vec<String> = app
        .registry()
        .zones_iter()
        .filter(|z| z.parent_zone.as_ref() == Some(&bar_zone_fq))
        .map(|z| z.segment.as_str().to_string())
        .collect();
    assert!(
        zone_segments.is_empty(),
        "fixture must register no zone children of ui:perspective-bar; \
         the Add-perspective button is intentionally non-spatial chrome \
         (got {zone_segments:?})"
    );
}
