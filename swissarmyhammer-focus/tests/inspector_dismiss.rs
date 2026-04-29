//! Source-of-truth integration tests for **drill-out at the inspector
//! layer root** in the spatial-nav kernel.
//!
//! These tests pin the kernel-level contract that the React-side dismiss
//! chain (`nav.drillOut` → `app.dismiss` → `DismissCmd::execute` →
//! `inspector_close`) depends on: [`SpatialRegistry::drill_out`] returns
//! `focused_fq` (echoes the input) exactly when the focused scope
//! has no `parent_zone` to walk to. The React adapter detects the
//! FQM-equality and falls through to `app.dismiss`, which in turn
//! pops the topmost inspector panel. See the no-silent-dropout contract
//! on [`swissarmyhammer_focus::navigate`] for the reasoning.
//!
//! Built against the realistic-app fixture in [`fixtures::RealisticApp`],
//! which constructs a [`SpatialRegistry`] mirroring the production tree:
//! a window-root layer with the navbar + perspective-bar + board zones,
//! plus a separate inspector layer whose panel zone (`panel:task:T1A`)
//! has `parent_zone = None` and three field-row zones nested inside it.
//!
//! # Why these tests live here
//!
//! The frontend "Escape closes the inspector" chain has five owners
//! (`cm-submit-cancel.ts`, inline `<input>` handlers, `nav.drillOut`,
//! `app.dismiss`, `<InspectorsContainer>`) and the bug
//! [`01KQ9TVZYXN65JHA479D1CS91T`] could fail at any seam. The kernel-
//! level seam — does `drill_out(panel_zone)` actually echo so
//! the React adapter falls through to dismiss? — is the cheapest to
//! pin first. If the kernel disagreed, no amount of React-side wiring
//! would close the inspector with one Escape; if the kernel agrees and
//! the inspector still does not close, the bug lives in the React
//! chain (proven elsewhere by the browser-mode tests).
//!
//! # Test cases
//!
//! - `drill_out_panel_zone_echoes_focused_fq` — the panel zone IS
//!   the layer-root scope of the inspector layer. Drilling out of it
//!   must return the panel's own FQM so the React adapter detects
//!   equality and dispatches `app.dismiss`.
//! - `drill_out_field_inside_panel_returns_panel_fq` — focus on a
//!   field row inside the panel walks up to the panel zone first, not
//!   straight to dismiss. The dismiss step happens on the *next* Escape.
//! - `drill_out_panel_with_no_inspector_layer_does_not_collapse_to_window`
//!   — guards the layer-boundary contract: a registry that registered
//!   only window-layer scopes (no inspector layer) must NOT have any
//!   scope whose drill-out target points back at a window-layer zone.
//!
//! [`SpatialRegistry::drill_out`]: swissarmyhammer_focus::SpatialRegistry::drill_out
//! [`fixtures::RealisticApp`]: ./fixtures/mod.rs
//! [`01KQ9TVZYXN65JHA479D1CS91T`]: # "Escape does not close the inspector"

mod fixtures;

use swissarmyhammer_focus::SpatialRegistry;

use fixtures::RealisticApp;

// ---------------------------------------------------------------------------
// drill_out — Panel zone (layer-root scope of the inspector layer)
// ---------------------------------------------------------------------------

/// The panel zone sits directly under the inspector layer with no parent
/// zone — drill-out must echo the focused FQM.
///
/// This is the kernel half of the "Escape closes the inspector" chain.
/// The React adapter (`nav.drillOut` in `app-shell.tsx`) interprets
/// the FQM-equality (result == focused_fq) as "fall through
/// to `app.dismiss`", which in turn pops the inspector stack via
/// `DismissCmd::execute`.
#[test]
fn drill_out_panel_zone_echoes_focused_fq() {
    let app = RealisticApp::new();
    let panel_fq = app.inspector_panel_fq();

    let target = app.registry().drill_out(panel_fq.clone(), &panel_fq);

    assert_eq!(
        target, panel_fq,
        "panel zone is a layer-root scope (parent_zone = None); drill_out \
         must echo the focused FQM so the React adapter detects equality \
         and falls through to app.dismiss",
    );
}

// ---------------------------------------------------------------------------
// drill_out — Field zone inside the panel
// ---------------------------------------------------------------------------

/// A field row inside the panel drills out to the panel zone, not to
/// `None`. The panel-zone-to-`None` step happens on the *next* Escape;
/// drill-out is repeated, walking the zone chain one hop at a time
/// before any dismiss fires.
///
/// This is the user-visible "first Escape navigates inside the panel,
/// second Escape closes the inspector" behavior.
#[test]
fn drill_out_field_inside_panel_returns_panel_fq() {
    let app = RealisticApp::new();
    let field_fq = app.inspector_field_title_fq();

    let target = app.registry().drill_out(field_fq.clone(), &field_fq);

    assert_eq!(
        target,
        app.inspector_panel_fq(),
        "drill-out from a field row must walk to the panel zone first; \
         the dismiss fall-through happens on the next Escape from the panel",
    );
}

// ---------------------------------------------------------------------------
// drill_out — Layer-boundary contract guard
// ---------------------------------------------------------------------------

/// Build a minimal window-only registry with a board zone and a card
/// inside it — no inspector layer at all. Drilling out from the
/// topmost board-side scope (the board zone, which sits directly
/// under the window-root layer with `parent_zone = None`) must echo
/// the focused FQM rather than collapsing across the absent inspector
/// layer.
///
/// Guards the layer-boundary contract from
/// `swissarmyhammer-focus/tests/navigate.rs::nav_never_crosses_layer_boundary_within_one_window`
/// in the drill-out direction: a registry without an inspector layer
/// has no path for drill-out to escape the window layer, so the
/// kernel must echo and let the React side handle the
/// missing-layer case.
#[test]
fn drill_out_panel_with_no_inspector_layer_does_not_collapse_to_window() {
    use std::collections::HashMap;

    use swissarmyhammer_focus::{
        FocusLayer, FocusScope, FocusZone, FullyQualifiedMoniker, LayerName, Pixels, Rect,
        SegmentMoniker, WindowLabel,
    };

    let mut reg = SpatialRegistry::new();

    // Window-root layer only — no inspector.
    let window_fq = FullyQualifiedMoniker::root(&SegmentMoniker::from_string("window"));
    reg.push_layer(FocusLayer {
        fq: window_fq.clone(),
        segment: SegmentMoniker::from_string("window"),
        name: LayerName::from_string("window"),
        parent: None,
        window_label: WindowLabel::from_string("main"),
        last_focused: None,
    });

    // Board zone sits directly under the window-root layer
    // (`parent_zone = None`) — the same shape as the panel zone in the
    // inspector-present fixture. Card leaf inside it for completeness.
    let board_rect = Rect {
        x: Pixels::new(0.0),
        y: Pixels::new(0.0),
        width: Pixels::new(1400.0),
        height: Pixels::new(900.0),
    };
    let board_fq =
        FullyQualifiedMoniker::compose(&window_fq, &SegmentMoniker::from_string("ui:board"));
    reg.register_zone(FocusZone {
        fq: board_fq.clone(),
        segment: SegmentMoniker::from_string("ui:board"),
        rect: board_rect,
        layer_fq: window_fq.clone(),
        parent_zone: None,
        last_focused: None,
        overrides: HashMap::new(),
    });
    let card_fq =
        FullyQualifiedMoniker::compose(&board_fq, &SegmentMoniker::from_string("task:T1A"));
    reg.register_scope(FocusScope {
        fq: card_fq,
        segment: SegmentMoniker::from_string("task:T1A"),
        rect: Rect {
            x: Pixels::new(8.0),
            y: Pixels::new(8.0),
            width: Pixels::new(400.0),
            height: Pixels::new(72.0),
        },
        layer_fq: window_fq,
        parent_zone: Some(board_fq.clone()),
        overrides: HashMap::new(),
    });

    // Drill-out from the board zone — the topmost board-side scope —
    // must echo the focused FQM. The board zone has
    // `parent_zone = None`, matching the panel-zone shape; the kernel
    // must not synthesise a path back into a non-existent layer.
    let target = reg.drill_out(board_fq.clone(), &board_fq);
    assert_eq!(
        target, board_fq,
        "board zone has no parent_zone; drill_out must echo the focused \
         FQM even when no inspector layer exists, so the React adapter \
         detects equality and dispatches app.dismiss (which is a no-op when \
         nothing is open)",
    );
}
