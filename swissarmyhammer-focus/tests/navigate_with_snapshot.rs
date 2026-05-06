//! Integration tests for `SpatialState::navigate_with_snapshot`.
//!
//! Snapshot-driven nav must agree with the registry-backed
//! `navigate_with` for matching scope sets — the parity invariant the
//! divergence diagnostic in the kanban-app `spatial_navigate` adapter
//! relies on. Each test sets up a registry, builds a `NavSnapshot`
//! mirroring it, runs both paths, and asserts the resulting
//! `FocusChangedEvent.next_fq` matches.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    BeamNavStrategy, Direction, FocusLayer, FocusOverrides, FocusScope, FullyQualifiedMoniker,
    LayerName, NavSnapshot, Pixels, Rect, SegmentMoniker, SnapshotScope, SpatialRegistry,
    SpatialState, WindowLabel,
};

const LAYER: &str = "/L";

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        x: Pixels::new(x),
        y: Pixels::new(y),
        width: Pixels::new(w),
        height: Pixels::new(h),
    }
}

fn make_layer() -> FocusLayer {
    FocusLayer {
        fq: FullyQualifiedMoniker::from_string(LAYER),
        segment: SegmentMoniker::from_string("window"),
        name: LayerName::from_string("window"),
        parent: None,
        window_label: WindowLabel::from_string("main"),
        last_focused: None,
    }
}

fn fq(s: &str) -> FullyQualifiedMoniker {
    FullyQualifiedMoniker::from_string(s)
}

fn seg(s: &str) -> SegmentMoniker {
    SegmentMoniker::from_string(s)
}

fn make_scope(fq_str: &str, segment: &str, parent_zone: Option<&str>, r: Rect) -> FocusScope {
    FocusScope {
        fq: fq(fq_str),
        segment: seg(segment),
        rect: r,
        layer_fq: fq(LAYER),
        parent_zone: parent_zone.map(fq),
        overrides: HashMap::new(),
        last_focused: None,
    }
}

fn snapshot_from_registry(reg: &SpatialRegistry) -> NavSnapshot {
    let layer_fq = fq(LAYER);
    NavSnapshot {
        scopes: reg
            .scopes_in_layer(&layer_fq)
            .map(|s| SnapshotScope {
                fq: s.fq.clone(),
                rect: s.rect,
                parent_zone: s.parent_zone.clone(),
                nav_override: s.overrides.clone(),
            })
            .collect(),
        layer_fq,
    }
}

fn run_both_paths(
    reg: &mut SpatialRegistry,
    state: &mut SpatialState,
    from: FullyQualifiedMoniker,
    direction: Direction,
) -> (Option<FullyQualifiedMoniker>, Option<FullyQualifiedMoniker>) {
    state
        .focus(reg, from.clone())
        .expect("focus must succeed before nav");

    let strategy = BeamNavStrategy::new();
    let registry_event =
        state
            .clone()
            .navigate_with(&mut reg.clone(), &strategy, from.clone(), direction);
    let registry_target = registry_event
        .and_then(|e| e.next_fq)
        .or(Some(from.clone()));

    let snapshot = snapshot_from_registry(reg);
    let snapshot_event =
        state
            .clone()
            .navigate_with_snapshot(&mut reg.clone(), &snapshot, from.clone(), direction);
    let snapshot_target = snapshot_event.and_then(|e| e.next_fq).or(Some(from));

    (registry_target, snapshot_target)
}

/// Cardinal `Down` picks the same target via either path.
#[test]
fn cardinal_down_parity() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer());
    reg.register_scope(make_scope(
        "/L/top",
        "top",
        None,
        rect(0.0, 0.0, 50.0, 30.0),
    ));
    reg.register_scope(make_scope(
        "/L/bottom",
        "bottom",
        None,
        rect(0.0, 100.0, 50.0, 30.0),
    ));

    let mut state = SpatialState::new();
    let (a, b) = run_both_paths(&mut reg, &mut state, fq("/L/top"), Direction::Down);
    assert_eq!(a, b);
    assert_eq!(a, Some(fq("/L/bottom")));
}

/// Cardinal `Up` picks the same target via either path.
#[test]
fn cardinal_up_parity() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer());
    reg.register_scope(make_scope(
        "/L/top",
        "top",
        None,
        rect(0.0, 0.0, 50.0, 30.0),
    ));
    reg.register_scope(make_scope(
        "/L/bottom",
        "bottom",
        None,
        rect(0.0, 100.0, 50.0, 30.0),
    ));

    let mut state = SpatialState::new();
    let (a, b) = run_both_paths(&mut reg, &mut state, fq("/L/bottom"), Direction::Up);
    assert_eq!(a, b);
    assert_eq!(a, Some(fq("/L/top")));
}

/// Cardinal `Left` picks the same target via either path.
#[test]
fn cardinal_left_parity() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer());
    reg.register_scope(make_scope("/L/l", "l", None, rect(0.0, 0.0, 50.0, 30.0)));
    reg.register_scope(make_scope("/L/r", "r", None, rect(100.0, 0.0, 50.0, 30.0)));

    let mut state = SpatialState::new();
    let (a, b) = run_both_paths(&mut reg, &mut state, fq("/L/r"), Direction::Left);
    assert_eq!(a, b);
    assert_eq!(a, Some(fq("/L/l")));
}

/// Cardinal `Right` picks the same target via either path.
#[test]
fn cardinal_right_parity() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer());
    reg.register_scope(make_scope("/L/l", "l", None, rect(0.0, 0.0, 50.0, 30.0)));
    reg.register_scope(make_scope("/L/r", "r", None, rect(100.0, 0.0, 50.0, 30.0)));

    let mut state = SpatialState::new();
    let (a, b) = run_both_paths(&mut reg, &mut state, fq("/L/l"), Direction::Right);
    assert_eq!(a, b);
    assert_eq!(a, Some(fq("/L/r")));
}

/// An override redirect is honored identically by both paths.
#[test]
fn override_redirect_parity() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer());

    let mut overrides: FocusOverrides = HashMap::new();
    overrides.insert(Direction::Down, Some(fq("/L/destination")));

    let mut from = make_scope("/L/from", "from", None, rect(0.0, 0.0, 50.0, 30.0));
    from.overrides = overrides;
    reg.register_scope(from);

    reg.register_scope(make_scope(
        "/L/destination",
        "destination",
        None,
        rect(500.0, 500.0, 50.0, 30.0),
    ));
    reg.register_scope(make_scope(
        "/L/below",
        "below",
        None,
        rect(0.0, 100.0, 50.0, 30.0),
    ));

    let mut state = SpatialState::new();
    let (a, b) = run_both_paths(&mut reg, &mut state, fq("/L/from"), Direction::Down);
    assert_eq!(a, b);
    assert_eq!(a, Some(fq("/L/destination")));
}

/// An explicit override wall (`Some(None)`) keeps focus on `from` via
/// either path.
#[test]
fn override_block_parity() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer());

    let mut overrides: FocusOverrides = HashMap::new();
    overrides.insert(Direction::Down, None);
    let mut from = make_scope("/L/from", "from", None, rect(0.0, 0.0, 50.0, 30.0));
    from.overrides = overrides;
    reg.register_scope(from);

    reg.register_scope(make_scope(
        "/L/below",
        "below",
        None,
        rect(0.0, 100.0, 50.0, 30.0),
    ));

    let mut state = SpatialState::new();
    let strategy = BeamNavStrategy::new();
    state
        .focus(&mut reg, fq("/L/from"))
        .expect("focus must succeed before nav");

    let registry_event =
        state
            .clone()
            .navigate_with(&mut reg.clone(), &strategy, fq("/L/from"), Direction::Down);
    let snapshot = snapshot_from_registry(&reg);
    let snapshot_event = state.clone().navigate_with_snapshot(
        &mut reg.clone(),
        &snapshot,
        fq("/L/from"),
        Direction::Down,
    );

    assert!(
        registry_event.is_none(),
        "wall override → no event from registry path"
    );
    assert!(
        snapshot_event.is_none(),
        "wall override → no event from snapshot path"
    );
}

/// At the visual edge of the layer, both paths return the focused FQM
/// (no event emitted because focus did not move).
#[test]
fn layer_bounded_edge_parity() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer());
    reg.register_scope(make_scope(
        "/L/lonely",
        "lonely",
        None,
        rect(0.0, 0.0, 50.0, 30.0),
    ));

    let mut state = SpatialState::new();
    state
        .focus(&mut reg, fq("/L/lonely"))
        .expect("focus must succeed before nav");

    let strategy = BeamNavStrategy::new();
    let registry_event = state.clone().navigate_with(
        &mut reg.clone(),
        &strategy,
        fq("/L/lonely"),
        Direction::Down,
    );
    let snapshot = snapshot_from_registry(&reg);
    let snapshot_event = state.clone().navigate_with_snapshot(
        &mut reg.clone(),
        &snapshot,
        fq("/L/lonely"),
        Direction::Down,
    );

    assert!(registry_event.is_none(), "edge → no event via registry");
    assert!(snapshot_event.is_none(), "edge → no event via snapshot");
}

/// Beam tie-break (leaf-over-container) resolves identically via either
/// path.
#[test]
fn beam_tie_break_parity() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer());

    // Focused scope at the top.
    reg.register_scope(make_scope(
        "/L/from",
        "from",
        None,
        rect(0.0, 0.0, 100.0, 30.0),
    ));
    // Container directly below — overlaps and contains the inner leaf.
    reg.register_scope(make_scope(
        "/L/container",
        "container",
        None,
        rect(0.0, 100.0, 100.0, 100.0),
    ));
    // Inner leaf with the same top edge as the container — same beam
    // score; leaves win over containers in the tie-break.
    reg.register_scope(make_scope(
        "/L/container/leaf",
        "leaf",
        Some("/L/container"),
        rect(20.0, 100.0, 60.0, 30.0),
    ));

    let mut state = SpatialState::new();
    let (a, b) = run_both_paths(&mut reg, &mut state, fq("/L/from"), Direction::Down);
    assert_eq!(a, b);
    assert_eq!(a, Some(fq("/L/container/leaf")));
}
