//! Integration tests for `SpatialState::focus_lost`.
//!
//! Snapshot-driven unmount-detection: when the focused scope unmounts on
//! the React side, React builds a snapshot whose `scopes` set has had
//! the lost FQM already removed and dispatches `spatial_focus_lost`. The
//! kernel runs the same fallback cascade as `handle_unregister`, but
//! reads the in-layer scope walk from the snapshot instead of the
//! registry.
//!
//! These tests pin three properties:
//!
//! - **Parity**: `focus_lost` and `handle_unregister` produce the same
//!   `FocusChangedEvent` for a given pre-unmount registry state.
//! - **Coexistence dedup**: both IPCs may fire on the same unmount in
//!   either order; only one transition is observed.
//! - **Cascade reach**: the snapshot path can resolve through every
//!   `FallbackResolution` variant.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    FocusChangedEvent, FocusLayer, FocusOverrides, FocusScope, FullyQualifiedMoniker, LayerName,
    NavSnapshot, Pixels, Rect, SegmentMoniker, SnapshotScope, SpatialRegistry, SpatialState,
    WindowLabel,
};

// ---------------------------------------------------------------------------
// Builders
// ---------------------------------------------------------------------------

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        x: Pixels::new(x),
        y: Pixels::new(y),
        width: Pixels::new(w),
        height: Pixels::new(h),
    }
}

fn fq_in_layer(layer_path: &str, segment: &str) -> FullyQualifiedMoniker {
    FullyQualifiedMoniker::from_string(format!("{layer_path}/{segment}"))
}

fn leaf(
    fq: FullyQualifiedMoniker,
    segment: &str,
    layer: &str,
    parent_zone: Option<FullyQualifiedMoniker>,
    r: Rect,
) -> FocusScope {
    FocusScope {
        fq,
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone,
        overrides: HashMap::new(),
        last_focused: None,
    }
}

fn zone_with_last(
    fq: FullyQualifiedMoniker,
    segment: &str,
    layer: &str,
    parent_zone: Option<FullyQualifiedMoniker>,
    last_focused: Option<FullyQualifiedMoniker>,
    r: Rect,
) -> FocusScope {
    FocusScope {
        fq,
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone,
        last_focused,
        overrides: HashMap::new(),
    }
}

fn make_layer(
    fq_str: &str,
    segment: &str,
    window: &str,
    parent: Option<&str>,
    last_focused: Option<FullyQualifiedMoniker>,
) -> FocusLayer {
    FocusLayer {
        fq: FullyQualifiedMoniker::from_string(fq_str),
        segment: SegmentMoniker::from_string(segment),
        name: LayerName::from_string("window"),
        parent: parent.map(FullyQualifiedMoniker::from_string),
        window_label: WindowLabel::from_string(window),
        last_focused,
    }
}

/// Build the snapshot React would dispatch at `focus_lost` time:
/// every live scope under `layer_fq`, with `omit_fq` excluded so the
/// lost FQM is correctly absent.
fn snapshot_excluding(
    registry: &SpatialRegistry,
    layer_fq: &FullyQualifiedMoniker,
    omit_fq: &FullyQualifiedMoniker,
) -> NavSnapshot {
    let scopes = registry
        .scopes_in_layer(layer_fq)
        .filter(|s| &s.fq != omit_fq)
        .map(|s| SnapshotScope {
            fq: s.fq.clone(),
            rect: s.rect,
            parent_zone: s.parent_zone.clone(),
            nav_override: FocusOverrides::new(),
        })
        .collect();
    NavSnapshot {
        layer_fq: layer_fq.clone(),
        scopes,
    }
}

// ---------------------------------------------------------------------------
// Parity: focus_lost matches handle_unregister
// ---------------------------------------------------------------------------

/// `focus_lost` produces the same `FocusChangedEvent` as the registry-
/// driven `handle_unregister` for a sibling-in-zone fallback.
#[test]
fn focus_lost_matches_handle_unregister_for_sibling_in_zone() {
    let (event_unregister, event_focus_lost) = run_both_paths_for_sibling_setup();
    assert_eq!(event_focus_lost, event_unregister);
}

fn run_both_paths_for_sibling_setup() -> (FocusChangedEvent, FocusChangedEvent) {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", "L", "main", None, None));
    let zone_fq = fq_in_layer("/L", "ui:zone");
    reg.register_scope(zone_with_last(
        zone_fq.clone(),
        "ui:zone",
        "/L",
        None,
        None,
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    let lost_fq = FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:lost"));
    let sib_fq = FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:sib"));
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/L",
        Some(zone_fq.clone()),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        sib_fq.clone(),
        "ui:sib",
        "/L",
        Some(zone_fq.clone()),
        rect(20.0, 0.0, 10.0, 10.0),
    ));

    // Registry path on a clone.
    let mut reg_a = reg.clone();
    let mut state_a = SpatialState::new();
    state_a.focus(&mut reg_a, lost_fq.clone()).expect("focus");
    let event_a = state_a
        .handle_unregister(&mut reg_a, &lost_fq, None)
        .expect("registry path emits");

    // Snapshot path on a separate clone.
    let mut reg_b = reg;
    let mut state_b = SpatialState::new();
    state_b.focus(&mut reg_b, lost_fq.clone()).expect("focus");
    let layer_fq = FullyQualifiedMoniker::from_string("/L");
    let snapshot = snapshot_excluding(&reg_b, &layer_fq, &lost_fq);
    let event_b = state_b
        .focus_lost(
            &mut reg_b,
            &snapshot,
            &lost_fq,
            Some(&zone_fq),
            &layer_fq,
            rect(0.0, 0.0, 10.0, 10.0),
        )
        .expect("snapshot path emits");

    assert_eq!(event_a.next_fq, Some(sib_fq));
    (event_a, event_b)
}

/// `focus_lost` clears the window slot when no fallback is reachable,
/// matching `handle_unregister`'s no-fallback behaviour.
#[test]
fn focus_lost_clears_focus_when_no_fallback() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", "L", "main", None, None));
    let lost_fq = fq_in_layer("/L", "ui:lost");
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/L",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state.focus(&mut reg, lost_fq.clone()).expect("focus");

    let layer_fq = FullyQualifiedMoniker::from_string("/L");
    let snapshot = snapshot_excluding(&reg, &layer_fq, &lost_fq);
    let event = state
        .focus_lost(
            &mut reg,
            &snapshot,
            &lost_fq,
            None,
            &layer_fq,
            rect(0.0, 0.0, 10.0, 10.0),
        )
        .expect("clear event");

    assert_eq!(event.window_label, WindowLabel::from_string("main"));
    assert_eq!(event.prev_fq, Some(lost_fq));
    assert_eq!(event.next_fq, None);
    assert_eq!(event.next_segment, None);
    assert_eq!(state.focused_in(&WindowLabel::from_string("main")), None);
}

/// `focus_lost` for an unfocused FQM is a no-op — the `spatial_focus_lost`
/// IPC racing `spatial_unregister_scope` (which already moved focus away)
/// must not emit a duplicate event.
#[test]
fn focus_lost_for_unfocused_fq_is_noop() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", "L", "main", None, None));
    let focused_fq = fq_in_layer("/L", "ui:focused");
    let other_fq = fq_in_layer("/L", "ui:other");
    reg.register_scope(leaf(
        focused_fq.clone(),
        "ui:focused",
        "/L",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        other_fq.clone(),
        "ui:other",
        "/L",
        None,
        rect(20.0, 0.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state.focus(&mut reg, focused_fq).expect("focus");

    // `other_fq` is registered but not focused.
    let layer_fq = FullyQualifiedMoniker::from_string("/L");
    let snapshot = snapshot_excluding(&reg, &layer_fq, &other_fq);
    let event = state.focus_lost(
        &mut reg,
        &snapshot,
        &other_fq,
        None,
        &layer_fq,
        rect(20.0, 0.0, 10.0, 10.0),
    );
    assert!(event.is_none(), "unfocused unmount produces no event");
}

// ---------------------------------------------------------------------------
// Coexistence dedup
// ---------------------------------------------------------------------------

/// When `focus_lost` runs first, the kernel applies the snapshot-driven
/// fallback. The subsequent `spatial_unregister_scope` (which calls
/// `handle_unregister`) sees the already-moved focus and returns `None`.
/// Exactly one transition is observed.
#[test]
fn coexistence_focus_lost_first_then_unregister_emits_once() {
    let mut reg = build_two_sibling_registry();
    let lost_fq = fq_in_layer("/L", "ui:zone/ui:lost");
    let sib_fq = fq_in_layer("/L", "ui:zone/ui:sib");
    let zone_fq = fq_in_layer("/L", "ui:zone");
    let layer_fq = FullyQualifiedMoniker::from_string("/L");

    let mut state = SpatialState::new();
    state.focus(&mut reg, lost_fq.clone()).expect("focus");

    // 1. spatial_focus_lost arrives first.
    let snapshot = snapshot_excluding(&reg, &layer_fq, &lost_fq);
    let first = state.focus_lost(
        &mut reg,
        &snapshot,
        &lost_fq,
        Some(&zone_fq),
        &layer_fq,
        rect(0.0, 0.0, 10.0, 10.0),
    );
    assert!(first.is_some(), "first IPC drives the transition");
    assert_eq!(first.as_ref().unwrap().next_fq, Some(sib_fq));

    // 2. spatial_unregister_scope arrives second — focus has already
    //    moved, so handle_unregister is a no-op.
    let second = state.handle_unregister(&mut reg, &lost_fq, None);
    assert!(second.is_none(), "second IPC is a no-op");
}

/// When `spatial_unregister_scope` runs first, the registry path drives
/// the fallback. The subsequent `spatial_focus_lost` sees the already-
/// moved focus and returns `None`.
#[test]
fn coexistence_unregister_first_then_focus_lost_emits_once() {
    let mut reg = build_two_sibling_registry();
    let lost_fq = fq_in_layer("/L", "ui:zone/ui:lost");
    let sib_fq = fq_in_layer("/L", "ui:zone/ui:sib");
    let zone_fq = fq_in_layer("/L", "ui:zone");
    let layer_fq = FullyQualifiedMoniker::from_string("/L");

    let mut state = SpatialState::new();
    state.focus(&mut reg, lost_fq.clone()).expect("focus");

    // Build the snapshot BEFORE unregister so it still reflects the
    // pre-unregister registry state — but with the lost FQM omitted to
    // mirror what React would dispatch.
    let snapshot = snapshot_excluding(&reg, &layer_fq, &lost_fq);

    // 1. spatial_unregister_scope arrives first (handle_unregister +
    //    registry.unregister_scope). Mirrors what
    //    `spatial_unregister_scope_inner` does.
    let first = state.handle_unregister(&mut reg, &lost_fq, None);
    reg.unregister_scope(&lost_fq);
    assert!(first.is_some(), "first IPC drives the transition");
    assert_eq!(first.as_ref().unwrap().next_fq, Some(sib_fq));

    // 2. spatial_focus_lost arrives second — focus has already moved,
    //    so focus_lost is a no-op.
    let second = state.focus_lost(
        &mut reg,
        &snapshot,
        &lost_fq,
        Some(&zone_fq),
        &layer_fq,
        rect(0.0, 0.0, 10.0, 10.0),
    );
    assert!(second.is_none(), "second IPC is a no-op");
}

fn build_two_sibling_registry() -> SpatialRegistry {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", "L", "main", None, None));
    let zone_fq = fq_in_layer("/L", "ui:zone");
    reg.register_scope(zone_with_last(
        zone_fq.clone(),
        "ui:zone",
        "/L",
        None,
        None,
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    let lost_fq = FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:lost"));
    let sib_fq = FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:sib"));
    reg.register_scope(leaf(
        lost_fq,
        "ui:lost",
        "/L",
        Some(zone_fq.clone()),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        sib_fq,
        "ui:sib",
        "/L",
        Some(zone_fq),
        rect(20.0, 0.0, 10.0, 10.0),
    ));
    reg
}

// ---------------------------------------------------------------------------
// Cascade reach: parent_zone last_focused, parent layer last_focused
// ---------------------------------------------------------------------------

/// Snapshot-vs-registry parity for a multi-level cascade: the focused
/// leaf is in an inner zone, the inner zone has no other live siblings,
/// and the cascade must walk up the parent_zone chain. Snapshot path
/// and registry path must resolve the same target.
#[test]
fn focus_lost_matches_handle_unregister_for_parent_zone_walk() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", "L", "main", None, None));
    let outer_zone = fq_in_layer("/L", "ui:outer");
    let inner_zone =
        FullyQualifiedMoniker::compose(&outer_zone, &SegmentMoniker::from_string("ui:inner"));
    let neighbor =
        FullyQualifiedMoniker::compose(&outer_zone, &SegmentMoniker::from_string("ui:neighbor"));
    reg.register_scope(zone_with_last(
        outer_zone.clone(),
        "ui:outer",
        "/L",
        None,
        None,
        rect(0.0, 0.0, 300.0, 300.0),
    ));
    reg.register_scope(zone_with_last(
        inner_zone.clone(),
        "ui:inner",
        "/L",
        Some(outer_zone.clone()),
        None,
        rect(0.0, 0.0, 50.0, 50.0),
    ));
    let lost_fq =
        FullyQualifiedMoniker::compose(&inner_zone, &SegmentMoniker::from_string("ui:lost"));
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/L",
        Some(inner_zone.clone()),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        neighbor,
        "ui:neighbor",
        "/L",
        Some(outer_zone),
        rect(100.0, 100.0, 10.0, 10.0),
    ));

    let layer_fq = FullyQualifiedMoniker::from_string("/L");

    let mut reg_a = reg.clone();
    let mut state_a = SpatialState::new();
    state_a.focus(&mut reg_a, lost_fq.clone()).expect("focus a");
    let event_a = state_a
        .handle_unregister(&mut reg_a, &lost_fq, None)
        .expect("registry path emits");

    let mut reg_b = reg;
    let mut state_b = SpatialState::new();
    state_b.focus(&mut reg_b, lost_fq.clone()).expect("focus b");
    let snapshot = snapshot_excluding(&reg_b, &layer_fq, &lost_fq);
    let event_b = state_b
        .focus_lost(
            &mut reg_b,
            &snapshot,
            &lost_fq,
            Some(&inner_zone),
            &layer_fq,
            rect(0.0, 0.0, 10.0, 10.0),
        )
        .expect("snapshot path emits");

    assert_eq!(event_a, event_b);
    assert!(event_a.next_fq.is_some(), "fallback resolves to a target");
}

/// When the entire scope tree under the lost layer empties, fallback
/// crosses into the parent layer and lands on its `last_focused`.
#[test]
fn focus_lost_walks_to_parent_layer_last_focused() {
    let mut reg = SpatialRegistry::new();
    let parent_last = fq_in_layer("/root", "ui:parent-last");
    reg.push_layer(make_layer(
        "/root",
        "root",
        "main",
        None,
        Some(parent_last.clone()),
    ));
    reg.register_scope(leaf(
        parent_last.clone(),
        "ui:parent-last",
        "/root",
        None,
        rect(50.0, 50.0, 10.0, 10.0),
    ));
    // Nested layer, no other scopes — so phase 1 (scope tree) finds
    // nothing and the cascade walks up.
    reg.push_layer(make_layer(
        "/root/nested",
        "nested",
        "main",
        Some("/root"),
        None,
    ));
    let lost_fq = fq_in_layer("/root/nested", "ui:lost");
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/root/nested",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state.focus(&mut reg, lost_fq.clone()).expect("focus");

    let layer_fq = FullyQualifiedMoniker::from_string("/root/nested");
    let snapshot = snapshot_excluding(&reg, &layer_fq, &lost_fq);
    let event = state
        .focus_lost(
            &mut reg,
            &snapshot,
            &lost_fq,
            None,
            &layer_fq,
            rect(0.0, 0.0, 10.0, 10.0),
        )
        .expect("event");

    assert_eq!(event.next_fq, Some(parent_last));
}
