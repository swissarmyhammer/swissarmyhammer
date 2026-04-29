//! Source-of-truth integration tests for **vertical (Up/Down) navigation
//! between inspector field zones** in the spatial-nav kernel, under the
//! **unified cascade** policy.
//!
//! These tests pin the kernel-level half of the user-facing bug pinned by
//! [`01KQAXS8QKWCKFK8ENEMN7WHR1`] (Inspector field Up/Down returns None).
//! Per the architectural fix in [`01KQAW97R9XTCNR1PJAWYSKBC7`],
//! [`BeamNavStrategy::next`] no longer returns `Option<FullyQualifiedMoniker>` — it
//! always returns a [`FullyQualifiedMoniker`] (echoing the focused FQM on
//! no-op or torn state). The diagnostic question is now: when the user
//! presses ArrowDown on field 1, does the kernel return field 2's FQM
//! (motion) or echo `field:task:T1A.title` (no motion)?
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
//! [`01KQAXS8QKWCKFK8ENEMN7WHR1`]: # "Inspector field Up/Down returns None"
//! [`01KQAW97R9XTCNR1PJAWYSKBC7`]: # "eliminate Option<Moniker>"
//! [`BeamNavStrategy::next`]: swissarmyhammer_focus::BeamNavStrategy

mod fixtures;

use swissarmyhammer_focus::{BeamNavStrategy, Direction, FullyQualifiedMoniker, NavStrategy};

use fixtures::RealisticApp;

/// Convenience: run [`BeamNavStrategy::next`] against the fixture's
/// registry from the named [`FullyQualifiedMoniker`] in the named
/// [`Direction`].
///
/// Resolves the focused entry's segment from the fixture registry —
/// under the no-silent-dropout contract every nav call needs the
/// focused segment alongside the focused FQM.
fn nav(app: &RealisticApp, from: &FullyQualifiedMoniker, dir: Direction) -> FullyQualifiedMoniker {
    let focused_segment = app
        .registry()
        .find_by_fq(from)
        .map(|e| e.segment().clone())
        .unwrap_or_else(|| panic!("nav called with unregistered FQM {from:?}"));
    BeamNavStrategy::new().next(app.registry(), from, &focused_segment, dir)
}

// ---------------------------------------------------------------------------
// Down — vertical advance through field zones inside the inspector panel.
// ---------------------------------------------------------------------------

/// Pressing Down from the title field advances to the status field.
#[test]
fn down_from_field_1_lands_on_field_2_in_inspector_panel() {
    let app = RealisticApp::new();
    assert_eq!(
        nav(&app, &app.inspector_field_title_fq(), Direction::Down),
        app.inspector_field_status_fq(),
        "down from field:task:T1A.title must land on field:task:T1A.status \
         (iter 0, same-kind peer sharing panel:task:T1A as parent_zone)"
    );
}

/// Pressing Down from the status field advances to the assignees field.
#[test]
fn down_from_field_2_lands_on_field_3() {
    let app = RealisticApp::new();
    assert_eq!(
        nav(&app, &app.inspector_field_status_fq(), Direction::Down),
        app.inspector_field_assignees_fq(),
        "down from field:task:T1A.status must land on field:task:T1A.assignees \
         (iter 0, same-kind peer sharing panel:task:T1A as parent_zone)"
    );
}

// ---------------------------------------------------------------------------
// Up — vertical retreat through field zones, symmetric to Down.
// ---------------------------------------------------------------------------

/// Pressing Up from the status field retreats to the title field.
#[test]
fn up_from_field_2_lands_on_field_1() {
    let app = RealisticApp::new();
    assert_eq!(
        nav(&app, &app.inspector_field_status_fq(), Direction::Up),
        app.inspector_field_title_fq(),
        "up from field:task:T1A.status must land on field:task:T1A.title \
         (iter 0, same-kind peer sharing panel:task:T1A as parent_zone)"
    );
}

// ---------------------------------------------------------------------------
// Drill-out — Down past the bottom field returns the panel FQM.
// ---------------------------------------------------------------------------

/// Pressing Down from the bottom field has no in-beam peer and no
/// sibling panel below — the cascade returns the panel zone's FQM
/// via drill-out.
#[test]
fn down_from_last_field_returns_panel_fq_via_drill_out() {
    let app = RealisticApp::new();
    let target = nav(&app, &app.inspector_field_assignees_fq(), Direction::Down);
    assert_eq!(
        target,
        app.inspector_panel_fq(),
        "down from field:task:T1A.assignees must return panel:task:T1A via \
         drill-out: iter 0 misses (no field below), iter 1 has no sibling \
         panels in the inspector layer, cascade falls through to drill-out \
         and returns the parent zone's FQM. The architectural contract \
         guarantees this is NOT the focused field's own FQM."
    );
    assert_ne!(
        target,
        app.inspector_field_assignees_fq(),
        "drill-out at the bottom of the inspector must not echo the focused \
         field's FQM — that would be the silent-no-motion regression \
         the architectural fix in 01KQAW97R9XTCNR1PJAWYSKBC7 eliminated"
    );
}

// ---------------------------------------------------------------------------
// Up off the top field — symmetric drill-out edge.
// ---------------------------------------------------------------------------

/// Pressing Up from the top field has no in-beam peer above, and the
/// cascade escalates to the panel which has no sibling panels — the
/// cascade returns the panel zone's FQM via drill-out.
#[test]
fn up_from_first_field_returns_panel_fq_via_drill_out() {
    let app = RealisticApp::new();
    let target = nav(&app, &app.inspector_field_title_fq(), Direction::Up);
    assert_eq!(
        target,
        app.inspector_panel_fq(),
        "up from field:task:T1A.title must return panel:task:T1A via \
         drill-out: iter 0 misses (no field above), iter 1 has no sibling \
         panels in the inspector layer, cascade falls through to drill-out"
    );
    assert_ne!(
        target,
        app.inspector_field_title_fq(),
        "drill-out at the top of the inspector must not echo the focused \
         field's FQM"
    );
}
