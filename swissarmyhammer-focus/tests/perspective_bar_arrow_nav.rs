//! Source-of-truth integration tests for **Left/Right arrow navigation
//! among the perspective bar's sibling tab leaves** under the unified
//! cascade.
//!
//! Built against the realistic-app fixture in `tests/fixtures/mod.rs`,
//! whose `ui:perspective-bar` zone holds — left to right — three
//! `perspective_tab:{id}` leaves (`perspective_tab:p1`,
//! `perspective_tab:p2`, `perspective_tab:p3`). All three share the same
//! `parent_zone = ui:perspective-bar` and the same horizontal strip, so
//! the user's keystrokes `Right` / `Left` should walk between them in
//! visual order — that is the contract these tests pin.
//!
//! # Why these tests are this card's source of truth
//!
//! The user reported on kanban card `01KQ9Z56M556DQHYMA502B9FKB` two
//! symptoms on the perspective bar:
//!
//! 1. No visible focus indicator on a focused perspective tab.
//! 2. Arrow Left / Right between sibling perspective tabs didn't traverse.
//!
//! Symptom 1 is a React-side seam — covered by
//! `kanban-app/ui/src/components/perspective-tab-bar.focus-indicator.browser.test.tsx`.
//! Symptom 2 is a kernel-side seam: from a focused perspective-tab leaf,
//! what does [`BeamNavStrategy::next`] return for `Direction::Right` /
//! `Direction::Left`? If the kernel returns the next-sibling moniker
//! the production app should walk; if it returns something else (or
//! `None`) the bug is in the kernel. Pinning the answer in Rust
//! integration tests removes any JS-shadow-registry mimicry layer.
//!
//! # Why the middle tab is wider
//!
//! Production widens the active perspective tab's leaf rect because the
//! active tab renders extra inline chrome (`<FilterFocusButton>` and
//! `<GroupPopoverButton>`) inside the same `<FocusScope>` wrapper. The
//! fixture mirrors that with p2's 160 px width vs. p1 / p3's 96 px so
//! beam search runs against the same rect-growth pattern the user
//! produces by clicking p2. The contract pinned here is that beam
//! search picks the next tab to the right by **left-edge ordering** —
//! the wider middle tab does not break the rect-based picks for sibling
//! tabs.
//!
//! # The unified cascade these tests pin
//!
//! Under the unified-policy supersession card
//! [`01KQ7S6WHK9RCCG2R4FN474EFD`] the cascade is:
//!
//! 1. **Iter 0** — same-kind peer search at the focused entry's level
//!    (leaves consider only leaf siblings, zones consider only zone
//!    siblings), filtered by the `parent_zone` and the in-beam Android
//!    score.
//! 2. **Iter 1** — escalate to the focused entry's `parent_zone` and
//!    search its sibling zones for an in-beam peer.
//! 3. **Drill-out** — when neither iter finds a peer, return the parent
//!    zone itself.
//!
//! For the perspective bar this means:
//!
//! - Right from `perspective_tab:p1` → iter 0 considers only sibling
//!   leaves under `ui:perspective-bar` → the next leaf to the right is
//!   `perspective_tab:p2`. Iter 0 wins.
//! - Right from `perspective_tab:p2` → iter 0 considers only sibling
//!   leaves → `perspective_tab:p3` is the next leaf to the right. Iter
//!   0 wins regardless of the wider p2 rect because beam search orders
//!   by left edge.
//! - Right from `perspective_tab:p3` (the rightmost leaf) → iter 0 has
//!   no sibling leaf to the right; iter 1 escalates to
//!   `ui:perspective-bar` and searches its sibling zones at the layer
//!   root (`ui:navbar`, `ui:board`); none are in-beam to the right.
//!   The drill-out fallback returns the parent zone itself,
//!   `ui:perspective-bar`. The user is `None`-stuck only on the next
//!   press from `ui:perspective-bar`, which sits at the layer root.
//!
//! Left walks the symmetric path.
//!
//! # The Add-perspective `+` button is intentionally non-spatial
//!
//! The production perspective bar renders an `AddPerspectiveButton`
//! after the rightmost tab, but it is NOT wrapped in `<FocusScope>` and
//! is therefore NOT registered with the spatial graph. The fixture
//! mirrors this by registering only the three tab leaves; `nav.right`
//! from `perspective_tab:p3` cannot land on the `+` button because the
//! kernel has no record of it. The
//! `perspective_right_from_rightmost_tab_drills_out_to_perspective_bar`
//! test pins the deliberate non-reachability.
//!
//! [`01KQ7S6WHK9RCCG2R4FN474EFD`]: # "unified-policy supersession card"
//! [`BeamNavStrategy::next`]: swissarmyhammer_focus::BeamNavStrategy::next

mod fixtures;

use swissarmyhammer_focus::{BeamNavStrategy, Direction, Moniker, NavStrategy, SpatialKey};

use fixtures::RealisticApp;

/// Convenience: run [`BeamNavStrategy::next`] against the fixture's
/// registry from the named [`SpatialKey`] in the named [`Direction`].
fn nav(app: &RealisticApp, from: &SpatialKey, dir: Direction) -> Option<Moniker> {
    BeamNavStrategy::new().next(app.registry(), from, dir)
}

// ---------------------------------------------------------------------------
// Right — horizontal advance through the perspective bar's tab leaves.
// ---------------------------------------------------------------------------

/// Pressing `Right` from `perspective_tab:p1` (the leftmost tab) lands
/// on `perspective_tab:p2` — the next sibling leaf to its right. Iter 0
/// (in-zone same-kind peer search) wins: both are leaves with
/// `parent_zone = ui:perspective-bar`, and `perspective_tab:p2` is the
/// closest in-beam Right peer.
#[test]
fn perspective_right_from_leftmost_tab_lands_on_middle_tab() {
    let app = RealisticApp::new();
    let from = app.perspective_tab_p1_key();
    assert_eq!(
        nav(&app, &from, Direction::Right),
        Some(Moniker::from_string("perspective_tab:p2")),
        "Right from perspective_tab:p1 must land on perspective_tab:p2 \
         (in-zone leaf peer to the right)"
    );
}

/// Pressing `Right` from the wider middle tab `perspective_tab:p2`
/// lands on `perspective_tab:p3` — the next sibling leaf to its right.
///
/// p2 is wider than p1 / p3 in the fixture (160 px vs. 96 px) to mirror
/// the production active-tab rect growth: the active perspective
/// renders inline `<FilterFocusButton>` and `<GroupPopoverButton>`
/// siblings inside the same `<FocusScope>` wrapper, growing the leaf's
/// bounding rect. The contract pinned here is that beam search picks
/// the next tab to the right by **left-edge ordering** — the wider
/// middle tab does not break the rect-based picks for sibling tabs.
///
/// This is the kernel-side counterpart to the
/// `focus_indicator_renders_when_active_tab_is_focused` browser test in
/// `kanban-app/ui/src/components/perspective-tab-bar.focus-indicator.browser.test.tsx`.
#[test]
fn perspective_right_from_middle_active_tab_lands_on_rightmost_tab() {
    let app = RealisticApp::new();
    let from = app.perspective_tab_p2_key();
    assert_eq!(
        nav(&app, &from, Direction::Right),
        Some(Moniker::from_string("perspective_tab:p3")),
        "Right from perspective_tab:p2 must land on perspective_tab:p3 \
         (in-zone leaf peer to the right; the wider middle-tab rect does \
         not break left-edge ordering)"
    );
}

// ---------------------------------------------------------------------------
// Left — horizontal retreat through the perspective bar's tab leaves.
// ---------------------------------------------------------------------------

/// Pressing `Left` from `perspective_tab:p3` (the rightmost tab) walks
/// the symmetric path back to `perspective_tab:p1`. Each step is an
/// in-zone leaf-peer match at iter 0.
#[test]
fn perspective_left_walks_symmetric_path() {
    let app = RealisticApp::new();

    // Step 1: p3 → p2 (in-zone leaf peer to the left).
    let from = app.perspective_tab_p3_key();
    assert_eq!(
        nav(&app, &from, Direction::Left),
        Some(Moniker::from_string("perspective_tab:p2")),
        "Left from perspective_tab:p3 must land on perspective_tab:p2 \
         (in-zone leaf peer to the left)"
    );

    // Step 2: p2 → p1 (in-zone leaf peer to the left).
    let from = app.perspective_tab_p2_key();
    assert_eq!(
        nav(&app, &from, Direction::Left),
        Some(Moniker::from_string("perspective_tab:p1")),
        "Left from perspective_tab:p2 must land on perspective_tab:p1 \
         (in-zone leaf peer to the left)"
    );
}

// ---------------------------------------------------------------------------
// Right from the rightmost tab — drill-out per unified policy.
// ---------------------------------------------------------------------------

/// Pressing `Right` from the rightmost tab `perspective_tab:p3` drills
/// out to `ui:perspective-bar`. Iter 0 has no in-zone same-kind leaf
/// peer to the right (p3 IS the rightmost). Iter 1 escalates to
/// `ui:perspective-bar`, which sits at the layer root
/// (`parent_zone = None`); iter 1's parent search runs against the
/// layer root and finds no Right peer at the same level (no zone with
/// `parent_zone = None` is in-beam right of `ui:perspective-bar`). The
/// drill-out fallback returns the parent zone itself,
/// `ui:perspective-bar`.
///
/// This is the unified-policy "drill-out keeps the user moving"
/// contract: the cascade never returns `None` while a parent zone is
/// reachable in the same layer. The user is `None`-stuck only when the
/// focused entry sits at the very root of its layer with no parent
/// zone.
///
/// The crucial user-visible guarantees pinned by this test:
///
///   1. **No bounce-back**: the answer must not be any previous
///      perspective tab.
///   2. **No leak to non-spatial chrome**: the answer must not be the
///      Add-perspective `+` button. The production `+` button is NOT
///      wrapped in `<FocusScope>` and is therefore NOT registered with
///      the spatial graph; the fixture mirrors this by registering
///      only the three tab leaves. The kernel has no record of the
///      `+` button so it cannot land on it — but the assertion below
///      pins the deliberate non-reachability so a future fixture edit
///      that introduces a fourth leaf would surface the regression.
///   3. **Drill-out target**: the cascade falls back to the parent
///      zone `ui:perspective-bar` rather than returning `None`.
#[test]
fn perspective_right_from_rightmost_tab_drills_out_to_perspective_bar() {
    let app = RealisticApp::new();
    let from = app.perspective_tab_p3_key();
    let result = nav(&app, &from, Direction::Right);

    // No-bounce-back and no leak to non-spatial chrome: the answer must
    // not be any previous perspective tab nor the Add-perspective `+`
    // button (which is intentionally non-spatial).
    let forbidden = [
        "perspective_tab:p1",
        "perspective_tab:p2",
        "perspective_tab:p3",
        "ui:perspective-bar.add",
        "ui:perspective-bar.add-perspective",
    ];
    if let Some(m) = result.as_ref() {
        assert!(
            !forbidden.contains(&m.as_str()),
            "Right from perspective_tab:p3 must not bounce back to a previous tab \
             nor land on the non-spatial Add-perspective button, got {m:?}",
        );
    }

    // Pin the specific drill-out outcome under the unified cascade.
    assert_eq!(
        result,
        Some(Moniker::from_string("ui:perspective-bar")),
        "Right from perspective_tab:p3 must drill out to ui:perspective-bar — iter 0 \
         finds no leaf peer right of p3, iter 1's parent ui:perspective-bar has no \
         Right peer at the layer root, and the cascade falls back to the parent zone \
         itself rather than returning None or bouncing back"
    );
}

// ---------------------------------------------------------------------------
// Sanity — fixture has the perspective-bar shape we asserted on.
//
// Light-touch verification that the realistic-app builder registers
// the three perspective tab leaves with the expected monikers and
// parents. If the fixture drifts (e.g. drops a tab or renames a
// moniker), this test surfaces the drift before the directional
// assertions degrade into harder-to-read failures.
// ---------------------------------------------------------------------------

/// The fixture registers exactly three perspective tab leaves inside
/// `ui:perspective-bar` with production-shaped monikers. Used as a
/// tripwire against future fixture edits that would invalidate the
/// directional tests silently.
#[test]
fn fixture_perspective_bar_has_three_tab_leaves() {
    let app = RealisticApp::new();

    let bar_zone_key = app
        .registry()
        .zones_iter()
        .find(|z| z.moniker.as_str() == "ui:perspective-bar")
        .map(|z| z.key.clone())
        .expect("fixture must register ui:perspective-bar zone");

    let mut tab_monikers: Vec<String> = app
        .registry()
        .leaves_iter()
        .filter(|s| s.parent_zone.as_ref() == Some(&bar_zone_key))
        .map(|s| s.moniker.as_str().to_string())
        .collect();
    tab_monikers.sort();
    assert_eq!(
        tab_monikers,
        vec![
            "perspective_tab:p1".to_string(),
            "perspective_tab:p2".to_string(),
            "perspective_tab:p3".to_string(),
        ],
        "fixture must register exactly three perspective tab leaves with the production \
         perspective_tab:{{id}} moniker shape"
    );

    // No zone children inside the perspective bar — the bar holds tab
    // leaves only. The Add-perspective `+` button is intentionally
    // non-spatial in production.
    let zone_monikers: Vec<String> = app
        .registry()
        .zones_iter()
        .filter(|z| z.parent_zone.as_ref() == Some(&bar_zone_key))
        .map(|z| z.moniker.as_str().to_string())
        .collect();
    assert!(
        zone_monikers.is_empty(),
        "fixture must register no zone children of ui:perspective-bar; \
         the Add-perspective button is intentionally non-spatial chrome \
         (got {zone_monikers:?})"
    );
}
