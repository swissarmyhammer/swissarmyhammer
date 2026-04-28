//! Source-of-truth integration tests for **Left/Right arrow navigation
//! among the navbar's sibling entries** under the unified cascade.
//!
//! Built against the realistic-app fixture in `tests/fixtures/mod.rs`,
//! whose `ui:navbar` zone holds — left to right — the
//! `ui:navbar.board-selector` leaf, the `ui:navbar.inspect` leaf, the
//! `field:board:b1.percent_complete` field zone, and the
//! `ui:navbar.search` leaf. Three of those four are `<FocusScope>` leaves
//! in production; the percent-complete field is itself a `<FocusZone>`
//! (see `kanban-app/ui/src/components/fields/field.tsx`). All four share
//! the same `parent_zone = ui:navbar` and the same horizontal strip, so
//! the user's keystrokes `Right` / `Left` should walk between them in
//! visual order — that is the contract these tests pin.
//!
//! # Why the kernel-level tests live here
//!
//! The user reported two symptoms on the navbar (kanban card
//! `01KQ9XWHP2Y5H1QB5B3RJFEBBR`):
//!
//! 1. No visible focus indicator on a focused navbar leaf.
//! 2. Arrow Left / Right between sibling navbar entries didn't traverse.
//!
//! Symptom 1 is a React-side seam (subscription / render). Symptom 2 is
//! a kernel-side seam: from a focused navbar leaf, what does
//! [`BeamNavStrategy::next`] return for `Direction::Right` /
//! `Direction::Left`? If the kernel returns the next-sibling moniker the
//! production app should walk; if it returns something else (or `None`)
//! the bug is in the kernel and a fix moves down here.
//!
//! Pinning the answer in Rust integration tests removes the JS-shadow-
//! registry mimicry layer the directional-nav supersession card
//! [`01KQ7STZN3G5N2WB3FF4PM4DKX`] explicitly retired.
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
//! For the navbar this means:
//!
//! - Right from `ui:navbar.board-selector` (a leaf) → iter 0 considers
//!   only sibling leaves under `ui:navbar` (it skips the field zone) →
//!   the next leaf to the right is `ui:navbar.inspect`. Iter 0 wins.
//! - Right from `ui:navbar.inspect` (a leaf) → iter 0 considers only
//!   sibling leaves → the next leaf to the right is `ui:navbar.search`
//!   (the percent-complete zone is filtered out as the wrong kind). The
//!   field zone is **skipped** at the kernel level. The user reaches it
//!   by drilling in (Enter / `<spatial_drill_in>`), not by cardinal nav.
//! - Right from `field:board:b1.percent_complete` (a zone) → iter 0
//!   considers only sibling zones — there are none under `ui:navbar`.
//!   Iter 1 escalates to `ui:navbar` and searches its sibling zones at
//!   the layer root (`ui:perspective-bar`, `ui:board`); none are
//!   in-beam to the right. The drill-out fallback returns the parent
//!   zone itself: `ui:navbar`. The user reaches `ui:navbar.search` from
//!   the field zone by drilling out (Right → `ui:navbar`, then Right
//!   again returns `None` per trajectory A's `ui:navbar → None`).
//! - Right from `ui:navbar.search` (the rightmost leaf) → iter 0 has
//!   no sibling leaf to the right; iter 1 escalates to `ui:navbar` and
//!   finds no Right zone peer at the layer root; the drill-out fallback
//!   returns `ui:navbar` itself. The user is `None`-stuck only on the
//!   next press from `ui:navbar`, which has no parent zone.
//!
//! Left walks the symmetric path.
//!
//! # Same-kind filtering rationale
//!
//! Same-kind filtering at iter 0 is the kernel's design choice from the
//! unified-policy card. Pulling both kinds into iter 0 would let a leaf
//! navigate **into** a zone via cardinal nav, blurring the drill-in
//! semantics the kernel reserves for Enter. The visible cost is that a
//! `<Field>` zone embedded between two leaves in the same row is
//! skipped by Right/Left — but that is the correct semantics for the
//! unified policy: cardinal nav stays within a kind; users cross kinds
//! by drilling in or out.
//!
//! [`01KQ7STZN3G5N2WB3FF4PM4DKX`]: # "directional-nav supersession card"
//! [`01KQ7S6WHK9RCCG2R4FN474EFD`]: # "unified-policy supersession card"
//! [`BeamNavStrategy::next`]: swissarmyhammer_focus::BeamNavStrategy::next

mod fixtures;

use swissarmyhammer_focus::{BeamNavStrategy, Direction, Moniker, NavStrategy, SpatialKey};

use fixtures::RealisticApp;

/// Convenience: run [`BeamNavStrategy::next`] against the fixture's
/// registry from the named [`SpatialKey`] in the named [`Direction`].
///
/// Resolves the focused entry's moniker from the fixture registry —
/// under the no-silent-dropout contract every nav call needs the
/// focused moniker alongside the focused key.
fn nav(app: &RealisticApp, from: &SpatialKey, dir: Direction) -> Moniker {
    let focused_moniker = app
        .registry()
        .leaves_iter()
        .map(|f| (&f.key, &f.moniker))
        .chain(app.registry().zones_iter().map(|z| (&z.key, &z.moniker)))
        .find(|(k, _)| **k == *from)
        .map(|(_, m)| m.clone())
        .unwrap_or_else(|| panic!("nav called with unregistered key {from:?}"));
    BeamNavStrategy::new().next(app.registry(), from, &focused_moniker, dir)
}

// ---------------------------------------------------------------------------
// Right — horizontal advance through the navbar's leaf siblings.
// ---------------------------------------------------------------------------

/// Pressing `Right` from `ui:navbar.board-selector` (the leftmost leaf)
/// lands on `ui:navbar.inspect` — the next sibling leaf to its right.
/// Iter 0 (in-zone same-kind peer search) wins: both are leaves with
/// `parent_zone = ui:navbar`, and `ui:navbar.inspect` is the closest
/// in-beam Right peer.
#[test]
fn navbar_right_from_board_selector_lands_on_inspect() {
    let app = RealisticApp::new();
    let from = app.navbar_board_selector_key();
    assert_eq!(
        nav(&app, &from, Direction::Right),
        Moniker::from_string("ui:navbar.inspect"),
        "Right from ui:navbar.board-selector must land on ui:navbar.inspect \
         (in-zone leaf peer to the right)"
    );
}

/// Pressing `Right` from `ui:navbar.inspect` lands on
/// `ui:navbar.search` — the next sibling **leaf** to its right. The
/// `field:board:b1.percent_complete` zone sits between inspect and
/// search visually, but iter 0's same-kind filter excludes the zone
/// from a leaf-origin search. Cardinal nav stays within the leaf
/// kind; the user crosses into the field zone by drilling in (Enter),
/// not by Right.
///
/// This is the unified-cascade contract: same-kind filtering is the
/// kernel's design decision and tests pin the visible consequence.
/// Mixing kinds at iter 0 was rejected by the unified-policy
/// supersession card [`01KQ7S6WHK9RCCG2R4FN474EFD`] because it would
/// let a focused card land on a field zone inside an adjacent card on
/// vertical nav — the same hazard applies horizontally here.
#[test]
fn navbar_right_from_inspect_lands_on_search() {
    let app = RealisticApp::new();
    let from = app.navbar_inspect_key();
    assert_eq!(
        nav(&app, &from, Direction::Right),
        Moniker::from_string("ui:navbar.search"),
        "Right from ui:navbar.inspect must land on ui:navbar.search \
         (next leaf to the right under the unified cascade's same-kind iter-0 filter; \
         the percent-complete field zone is skipped because cardinal nav stays within \
         the leaf kind — drill in to reach it)"
    );
}

/// Pressing `Right` from the percent-complete field **zone** drills
/// out to `ui:navbar`. Iter 0 (zone-only peer search inside
/// `ui:navbar`) finds no zone siblings — the navbar's other entries
/// are all leaves, filtered out by the same-kind iter-0 rule. Iter 1
/// escalates to `ui:navbar` and searches its sibling zones at the
/// layer root (`ui:perspective-bar`, `ui:board`); none are in-beam to
/// the right of `ui:navbar`. The drill-out fallback returns the
/// parent zone itself: `ui:navbar`.
///
/// This is the unified-cascade contract: the user is never `None`-stuck
/// while a parent zone is reachable in the same layer. Cardinal nav
/// from a navbar zone-typed entry walks the field zone out into its
/// parent rather than crossing into leaf siblings, since iter 0 stays
/// within a kind. To reach `ui:navbar.search` from the field zone the
/// user presses Right (drill-out to `ui:navbar`) and then Right again
/// (`ui:navbar` is at the layer root, returning `None` — symmetry of
/// trajectory A's `ui:navbar → None`).
#[test]
fn navbar_right_from_percent_field_zone_drills_out_to_navbar() {
    let app = RealisticApp::new();
    let from = app.navbar_percent_field_key();
    assert_eq!(
        nav(&app, &from, Direction::Right),
        Moniker::from_string("ui:navbar"),
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
/// Symmetric to [`navbar_right_from_inspect_lands_on_search`]: the
/// same-kind iter-0 filter skips the percent-complete field zone, so
/// the leaf to the left of search is `ui:navbar.inspect`, not the
/// field zone visually nestled between them.
#[test]
fn navbar_left_walks_symmetric_path() {
    let app = RealisticApp::new();

    // Step 1: search → inspect (skipping the field zone via same-kind
    // filter).
    let from = app.navbar_search_key();
    assert_eq!(
        nav(&app, &from, Direction::Left),
        Moniker::from_string("ui:navbar.inspect"),
        "Left from ui:navbar.search must land on ui:navbar.inspect \
         (the next leaf to the left under same-kind iter-0 filter; the percent-complete \
         field zone is skipped — symmetric to the Right case)"
    );

    // Step 2: inspect → board-selector (in-zone leaf peer to the left).
    let from = app.navbar_inspect_key();
    assert_eq!(
        nav(&app, &from, Direction::Left),
        Moniker::from_string("ui:navbar.board-selector"),
        "Left from ui:navbar.inspect must land on ui:navbar.board-selector \
         (in-zone leaf peer to the left)"
    );
}

/// Pressing `Right` from the rightmost leaf `ui:navbar.search` drills
/// out to `ui:navbar`. Iter 0 has no in-zone same-kind leaf peer to
/// the right. Iter 1 escalates to `ui:navbar`, which sits at the layer
/// root (`parent_zone = None`); iter 1's parent search runs against
/// the layer root and finds no Right peer at the same level (no zone
/// with `parent_zone = None` is in-beam right of `ui:navbar`). The
/// drill-out fallback returns the parent zone itself, `ui:navbar`.
///
/// This is the unified-policy "drill-out keeps the user moving"
/// contract: the cascade never returns `None` while a parent zone is
/// reachable in the same layer. The user is `None`-stuck only when the
/// focused entry sits at the very root of its layer with no parent
/// zone — and that case is the **next** Right press, from `ui:navbar`,
/// which trajectory A pins as returning `None`. Coordinate with card
/// [`01KQ7S6WHK9RCCG2R4FN474EFD`]: under the unified cascade, the
/// rightmost-leaf-of-rightmost-zone case drills out one level rather
/// than terminating immediately at `None`.
///
/// The crucial user-visible guarantee: the navigator never **bounces
/// back** to a previous navbar leaf. This test verifies the explicit
/// no-bounce contract before pinning the specific drill-out target.
///
/// [`01KQ7S6WHK9RCCG2R4FN474EFD`]: # "unified-policy supersession card"
#[test]
fn navbar_right_from_rightmost_leaf_drills_out_to_navbar() {
    let app = RealisticApp::new();
    let from = app.navbar_search_key();
    let result = nav(&app, &from, Direction::Right);

    // No-bounce-back: the answer must not be any previous navbar
    // entry, including search itself (the cascade must move, not echo
    // the focused moniker, since `ui:navbar` is a registered parent).
    let forbidden = [
        "ui:navbar.search",
        "ui:navbar.inspect",
        "ui:navbar.board-selector",
        "field:board:b1.percent_complete",
    ];
    assert!(
        !forbidden.contains(&result.as_str()),
        "Right from ui:navbar.search must not bounce back to a navbar entry, got {result:?}",
    );
    // Pin the specific drill-out outcome under the unified cascade.
    assert_eq!(
        result,
        Moniker::from_string("ui:navbar"),
        "Right from ui:navbar.search must drill out to ui:navbar — iter 0 finds no \
         leaf peer right of search, iter 1's parent ui:navbar has no Right peer at \
         the layer root, and the cascade falls back to the parent zone itself rather \
         than returning None or bouncing back"
    );
}

// ---------------------------------------------------------------------------
// Sanity — fixture has the navbar shape we asserted on.
//
// Light-touch verification that the realistic-app builder registers
// the four navbar entries with the expected monikers and parents. If
// the fixture drifts (e.g. drops the percent-field zone or renames a
// navbar moniker), this test surfaces the drift before the directional
// assertions degrade into harder-to-read failures.
// ---------------------------------------------------------------------------

/// The fixture registers four entries inside `ui:navbar`: three leaves
/// and one field zone. Used as a tripwire against future fixture edits
/// that would invalidate the directional tests silently.
#[test]
fn fixture_navbar_has_three_leaves_and_one_field_zone() {
    let app = RealisticApp::new();

    let navbar_zone_key = app
        .registry()
        .zones_iter()
        .find(|z| z.moniker.as_str() == "ui:navbar")
        .map(|z| z.key.clone())
        .expect("fixture must register ui:navbar zone");

    let mut leaf_monikers: Vec<String> = app
        .registry()
        .leaves_iter()
        .filter(|s| s.parent_zone.as_ref() == Some(&navbar_zone_key))
        .map(|s| s.moniker.as_str().to_string())
        .collect();
    leaf_monikers.sort();
    assert_eq!(
        leaf_monikers,
        vec![
            "ui:navbar.board-selector".to_string(),
            "ui:navbar.inspect".to_string(),
            "ui:navbar.search".to_string(),
        ],
        "fixture must register exactly three navbar leaves with the production monikers"
    );

    let zone_monikers: Vec<String> = app
        .registry()
        .zones_iter()
        .filter(|z| z.parent_zone.as_ref() == Some(&navbar_zone_key))
        .map(|z| z.moniker.as_str().to_string())
        .collect();
    assert_eq!(
        zone_monikers,
        vec!["field:board:b1.percent_complete".to_string()],
        "fixture must register the percent-complete field as a zone child of ui:navbar"
    );
}
