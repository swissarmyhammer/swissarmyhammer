//! Source-of-truth integration tests for **vertical (Up/Down) navigation
//! between inspector field zones** in the spatial-nav kernel, under the
//! **unified cascade** policy.
//!
//! These tests pin the kernel-level half of the user-facing bug pinned by
//! [`01KQAXS8QKWCKFK8ENEMN7WHR1`] (Inspector field Up/Down returns None).
//! Per the architectural fix in [`01KQAW97R9XTCNR1PJAWYSKBC7`],
//! [`BeamNavStrategy::next`] no longer returns `Option<Moniker>` — it
//! always returns a [`Moniker`] (echoing the focused moniker on no-op or
//! torn state). The diagnostic question is now: when the user presses
//! ArrowDown on field 1, does the kernel return field 2's moniker (motion)
//! or echo `field:task:T1A.title` (no motion)?
//!
//! Built against the realistic-app fixture in [`fixtures::RealisticApp`],
//! which mirrors the production inspector tree:
//!
//! ```text
//! inspector layer (parent = window layer)
//! └── panel:task:T1A      (parent_zone = None)
//!     ├── field:task:T1A.title       (parent_zone = panel:task:T1A)
//!     ├── field:task:T1A.status      (parent_zone = panel:task:T1A)
//!     └── field:task:T1A.assignees   (parent_zone = panel:task:T1A)
//! ```
//!
//! Field rows are stacked vertically with non-overlapping rects and a
//! shared horizontal extent, so iter 0 (same-kind peers sharing
//! `parent_zone`) sees them as in-beam Down/Up neighbors.
//!
//! # What this pins
//!
//! - **Down between adjacent fields** — `next(field_1, Down) ==
//!   field_2.moniker`, `next(field_2, Down) == field_3.moniker`. Iter 0
//!   match: same-kind (zone) peers sharing the panel's `parent_zone`,
//!   in-beam on the horizontal axis, vertically below.
//! - **Up between adjacent fields** — symmetric.
//! - **Down off the bottom field** — `next(field_3, Down) ==
//!   panel.moniker`. Iter 0 misses (no field below); escalation lifts to
//!   the panel zone; iter 1 has no sibling panels in the inspector
//!   layer; cascade falls through to drill-out and returns the parent
//!   zone's moniker. The user is NOT stuck on the focused moniker — the
//!   architectural contract guarantees the cascade returns the panel
//!   moniker, not the field's own moniker.
//!
//! If any of these assertions fail, the bug is in the kernel cascade
//! (`navigate.rs`). If they all pass, the kernel is correct for this
//! shape and any user-reported "vertical nav doesn't work in the
//! inspector" bug lives in the React registration site (mounted shape
//! diverges from the fixture). The companion frontend diagnostic test
//! (`entity-inspector.field-up-down.diagnostic.browser.test.tsx`) snaps
//! the registered shape and pins which seam diverges.
//!
//! [`01KQAXS8QKWCKFK8ENEMN7WHR1`]: # "Inspector field Up/Down returns None"
//! [`01KQAW97R9XTCNR1PJAWYSKBC7`]: # "eliminate Option<Moniker>"
//! [`BeamNavStrategy::next`]: swissarmyhammer_focus::BeamNavStrategy
//! [`Moniker`]: swissarmyhammer_focus::Moniker

mod fixtures;

use swissarmyhammer_focus::{BeamNavStrategy, Direction, Moniker, NavStrategy, SpatialKey};

use fixtures::RealisticApp;

/// Convenience: run [`BeamNavStrategy::next`] against the fixture's
/// registry from the named [`SpatialKey`] in the named [`Direction`].
///
/// Resolves the focused entry's moniker from the fixture registry —
/// under the no-silent-dropout contract every nav call needs the
/// focused moniker alongside the focused key. Mirrors the helper in
/// `tests/card_directional_nav.rs` so tests in the suite read the same
/// way.
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
// Field-zone keys / monikers — shared by all tests below.
// ---------------------------------------------------------------------------

/// SpatialKey for the title field zone (top of the inspector panel).
fn field_title_key() -> SpatialKey {
    SpatialKey::from_string("k_field_t1a_title")
}

/// Moniker for the title field zone.
fn field_title_moniker() -> Moniker {
    Moniker::from_string("field:task:T1A.title")
}

/// SpatialKey for the status field zone (middle row).
fn field_status_key() -> SpatialKey {
    SpatialKey::from_string("k_field_t1a_status")
}

/// Moniker for the status field zone.
fn field_status_moniker() -> Moniker {
    Moniker::from_string("field:task:T1A.status")
}

/// SpatialKey for the assignees field zone (bottom row).
fn field_assignees_key() -> SpatialKey {
    SpatialKey::from_string("k_field_t1a_assignees")
}

/// Moniker for the assignees field zone.
fn field_assignees_moniker() -> Moniker {
    Moniker::from_string("field:task:T1A.assignees")
}

/// Moniker for the inspector panel zone (parent of all three fields).
fn panel_moniker() -> Moniker {
    Moniker::from_string("panel:task:T1A")
}

// ---------------------------------------------------------------------------
// Down — vertical advance through field zones inside the inspector panel.
// ---------------------------------------------------------------------------

/// Pressing Down from the title field advances to the status field.
///
/// Iter 0 hit: status is a same-kind (zone) peer sharing the panel's
/// `parent_zone`, vertically below the title field, in-beam on the
/// horizontal axis (both fields span the inspector body's full width).
/// This is the user's experience navigating from the topmost editable
/// field to the next one with a single ArrowDown press.
#[test]
fn down_from_field_1_lands_on_field_2_in_inspector_panel() {
    let app = RealisticApp::new();
    assert_eq!(
        nav(&app, &field_title_key(), Direction::Down),
        field_status_moniker(),
        "down from field:task:T1A.title must land on field:task:T1A.status \
         (iter 0, same-kind peer sharing panel:task:T1A as parent_zone)"
    );
}

/// Pressing Down from the status field advances to the assignees field.
///
/// Same iter-0 in-zone peer search, one slot further down. Pinning both
/// pairs of adjacent fields catches a regression where iter 0 returns
/// the *first* same-kind peer regardless of beam scoring (which would
/// also have made `down_from_field_1_lands_on_field_2` pass against the
/// fixture).
#[test]
fn down_from_field_2_lands_on_field_3() {
    let app = RealisticApp::new();
    assert_eq!(
        nav(&app, &field_status_key(), Direction::Down),
        field_assignees_moniker(),
        "down from field:task:T1A.status must land on field:task:T1A.assignees \
         (iter 0, same-kind peer sharing panel:task:T1A as parent_zone)"
    );
}

// ---------------------------------------------------------------------------
// Up — vertical retreat through field zones, symmetric to Down.
// ---------------------------------------------------------------------------

/// Pressing Up from the status field retreats to the title field.
///
/// Iter 0 hit: title is a same-kind peer above status, in-beam on the
/// horizontal axis. Symmetric counterpart to
/// `down_from_field_1_lands_on_field_2`.
#[test]
fn up_from_field_2_lands_on_field_1() {
    let app = RealisticApp::new();
    assert_eq!(
        nav(&app, &field_status_key(), Direction::Up),
        field_title_moniker(),
        "up from field:task:T1A.status must land on field:task:T1A.title \
         (iter 0, same-kind peer sharing panel:task:T1A as parent_zone)"
    );
}

// ---------------------------------------------------------------------------
// Drill-out — Down past the bottom field returns the panel moniker.
// ---------------------------------------------------------------------------

/// Pressing Down from the bottom field has no in-beam peer and no
/// sibling panel below — the cascade returns the panel zone's moniker
/// via drill-out.
///
/// This is the assertion that pins "vertical nav does not silently
/// return the focused moniker at the bottom of the inspector body". The
/// architectural contract guarantees that when iter 0 misses AND iter 1
/// (sibling panels in the inspector layer) misses, the cascade falls
/// through to drill-out and returns the parent zone — not the focused
/// entry's own moniker. The React adapter then sees a different moniker
/// and moves focus to the panel; the user perceives this as "the focus
/// indicator jumps from the last field to the panel itself".
///
/// If this test ever fails by returning `field:task:T1A.assignees`, the
/// kernel regressed into the silent-no-motion behavior that the
/// architectural fix in `01KQAW97R9XTCNR1PJAWYSKBC7` eliminated.
#[test]
fn down_from_last_field_returns_panel_moniker_via_drill_out() {
    let app = RealisticApp::new();
    let target = nav(&app, &field_assignees_key(), Direction::Down);
    assert_eq!(
        target,
        panel_moniker(),
        "down from field:task:T1A.assignees must return panel:task:T1A via \
         drill-out: iter 0 misses (no field below), iter 1 has no sibling \
         panels in the inspector layer, cascade falls through to drill-out \
         and returns the parent zone's moniker. The architectural contract \
         guarantees this is NOT the focused field's own moniker."
    );
    assert_ne!(
        target,
        field_assignees_moniker(),
        "drill-out at the bottom of the inspector must not echo the focused \
         field's moniker — that would be the silent-no-motion regression \
         the architectural fix in 01KQAW97R9XTCNR1PJAWYSKBC7 eliminated"
    );
}

// ---------------------------------------------------------------------------
// Up off the top field — symmetric drill-out edge.
// ---------------------------------------------------------------------------

/// Pressing Up from the top field has no in-beam peer above, and the
/// cascade escalates to the panel which has no sibling panels — the
/// cascade returns the panel zone's moniker via drill-out.
///
/// Symmetric counterpart to
/// `down_from_last_field_returns_panel_moniker_via_drill_out`. Pinning
/// both edges catches a regression where one direction silently echoes
/// the focused moniker while the other drills out correctly.
#[test]
fn up_from_first_field_returns_panel_moniker_via_drill_out() {
    let app = RealisticApp::new();
    let target = nav(&app, &field_title_key(), Direction::Up);
    assert_eq!(
        target,
        panel_moniker(),
        "up from field:task:T1A.title must return panel:task:T1A via \
         drill-out: iter 0 misses (no field above), iter 1 has no sibling \
         panels in the inspector layer, cascade falls through to drill-out"
    );
    assert_ne!(
        target,
        field_title_moniker(),
        "drill-out at the top of the inspector must not echo the focused \
         field's moniker"
    );
}
