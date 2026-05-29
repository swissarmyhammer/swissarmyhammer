//! Integration tests for `SpatialState::navigate`.
//!
//! Each test stands up a registry layer + snapshot and asserts that
//! `state.navigate(...)` lands on the expected target (or short-circuits
//! to `None` for stay-put / torn-state outcomes).

use std::collections::HashMap;

use swissarmyhammer_focus::{
    Direction, FocusLayer, FullyQualifiedMoniker, LayerName, NavSnapshot, Pixels, Rect,
    SegmentMoniker, SnapshotScope, SpatialRegistry, SpatialState, WindowLabel,
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

fn snap(fq_str: &str, parent_zone: Option<&str>, r: Rect) -> SnapshotScope {
    SnapshotScope {
        fq: fq(fq_str),
        rect: r,
        parent_zone: parent_zone.map(fq),
        nav_override: HashMap::new(),
        focusable: true,
    }
}

fn snapshot(scopes: Vec<SnapshotScope>) -> NavSnapshot {
    NavSnapshot {
        layer_fq: fq(LAYER),
        scopes,
    }
}

fn nav_to(
    snapshot: &NavSnapshot,
    from: FullyQualifiedMoniker,
    direction: Direction,
) -> Option<FullyQualifiedMoniker> {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer());
    let mut state = SpatialState::new();
    state
        .focus(&mut reg, snapshot, from.clone())
        .expect("focus seed");
    state
        .navigate(&mut reg, snapshot, from, direction)
        .and_then(|e| e.next_fq)
}

#[test]
fn down_picks_below_neighbor() {
    let snapshot = snapshot(vec![
        snap("/L/top", None, rect(0.0, 0.0, 10.0, 10.0)),
        snap("/L/bottom", None, rect(0.0, 20.0, 10.0, 10.0)),
    ]);
    let target = nav_to(&snapshot, fq("/L/top"), Direction::Down);
    assert_eq!(target, Some(fq("/L/bottom")));
}

#[test]
fn empty_half_plane_returns_none() {
    let snapshot = snapshot(vec![snap("/L/only", None, rect(0.0, 0.0, 10.0, 10.0))]);
    // Pathfinding echoes focused FQM; navigate detects "already focused"
    // and short-circuits.
    let target = nav_to(&snapshot, fq("/L/only"), Direction::Right);
    assert!(target.is_none(), "lonely scope must not move on Right");
}

#[test]
fn override_redirect_target_wins() {
    let mut src = snap("/L/src", None, rect(0.0, 0.0, 10.0, 10.0));
    src.nav_override
        .insert(Direction::Right, Some(fq("/L/jump")));
    let snapshot = snapshot(vec![
        src,
        snap("/L/jump", None, rect(100.0, 0.0, 10.0, 10.0)),
    ]);
    let target = nav_to(&snapshot, fq("/L/src"), Direction::Right);
    assert_eq!(target, Some(fq("/L/jump")));
}

#[test]
fn override_wall_blocks_navigation() {
    let mut src = snap("/L/src", None, rect(0.0, 0.0, 10.0, 10.0));
    src.nav_override.insert(Direction::Right, None);
    let snapshot = snapshot(vec![
        src,
        snap("/L/neighbor", None, rect(20.0, 0.0, 10.0, 10.0)),
    ]);
    let target = nav_to(&snapshot, fq("/L/src"), Direction::Right);
    assert!(
        target.is_none(),
        "wall must take precedence over geometric pick"
    );
}

#[test]
fn first_picks_topmost_then_leftmost_child() {
    let snapshot = snapshot(vec![
        snap("/L/parent", None, rect(0.0, 0.0, 300.0, 300.0)),
        snap(
            "/L/parent/alpha",
            Some("/L/parent"),
            rect(0.0, 0.0, 50.0, 30.0),
        ),
        snap(
            "/L/parent/beta",
            Some("/L/parent"),
            rect(100.0, 0.0, 50.0, 30.0),
        ),
        snap(
            "/L/parent/gamma",
            Some("/L/parent"),
            rect(0.0, 100.0, 50.0, 30.0),
        ),
    ]);
    let target = nav_to(&snapshot, fq("/L/parent"), Direction::First);
    assert_eq!(target, Some(fq("/L/parent/alpha")));
}

#[test]
fn last_picks_bottommost_then_rightmost_child() {
    let snapshot = snapshot(vec![
        snap("/L/parent", None, rect(0.0, 0.0, 300.0, 300.0)),
        snap(
            "/L/parent/alpha",
            Some("/L/parent"),
            rect(0.0, 0.0, 50.0, 30.0),
        ),
        snap(
            "/L/parent/beta",
            Some("/L/parent"),
            rect(100.0, 0.0, 50.0, 30.0),
        ),
        snap(
            "/L/parent/gamma",
            Some("/L/parent"),
            rect(0.0, 100.0, 50.0, 30.0),
        ),
    ]);
    let target = nav_to(&snapshot, fq("/L/parent"), Direction::Last);
    assert_eq!(target, Some(fq("/L/parent/gamma")));
}
