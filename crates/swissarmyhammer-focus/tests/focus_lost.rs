//! Integration tests for `SpatialState::focus_lost`.
//!
//! Snapshot-driven unmount-detection: when the focused scope unmounts on
//! the React side, React builds a snapshot whose `scopes` set has had
//! the lost FQM already removed and dispatches `spatial_focus_lost`. The
//! kernel runs the fallback cascade against the snapshot.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    FocusLayer, FullyQualifiedMoniker, LayerName, NavSnapshot, Pixels, Rect, SegmentMoniker,
    SnapshotScope, SpatialRegistry, SpatialState, WindowLabel,
};

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        x: Pixels::new(x),
        y: Pixels::new(y),
        width: Pixels::new(w),
        height: Pixels::new(h),
    }
}

fn fq(s: &str) -> FullyQualifiedMoniker {
    FullyQualifiedMoniker::from_string(s)
}

fn layer_node(fq_str: &str, window: &str, parent: Option<&str>) -> FocusLayer {
    FocusLayer {
        fq: FullyQualifiedMoniker::from_string(fq_str),
        segment: SegmentMoniker::from_string("window"),
        name: LayerName::from_string("window"),
        parent: parent.map(FullyQualifiedMoniker::from_string),
        window_label: WindowLabel::from_string(window),
        last_focused: None,
    }
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

/// Sibling fallback: lost scope had a sibling under the same parent
/// zone.
#[test]
fn focus_lost_picks_sibling_in_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer_node("/L", "main", None));
    let mut state = SpatialState::new();

    // Pre-unmount snapshot: parent zone with two children.
    let pre_unmount = NavSnapshot {
        layer_fq: fq("/L"),
        scopes: vec![
            snap("/L/zone", None, rect(0.0, 0.0, 100.0, 100.0)),
            snap("/L/zone/lost", Some("/L/zone"), rect(0.0, 0.0, 10.0, 10.0)),
            snap(
                "/L/zone/sibling",
                Some("/L/zone"),
                rect(20.0, 0.0, 10.0, 10.0),
            ),
        ],
    };

    state
        .focus(&mut reg, &pre_unmount, fq("/L/zone/lost"))
        .expect("focus lost initially");

    // Post-unmount snapshot: lost scope removed.
    let post_unmount = NavSnapshot {
        layer_fq: fq("/L"),
        scopes: vec![
            snap("/L/zone", None, rect(0.0, 0.0, 100.0, 100.0)),
            snap(
                "/L/zone/sibling",
                Some("/L/zone"),
                rect(20.0, 0.0, 10.0, 10.0),
            ),
        ],
    };

    let event = state
        .focus_lost(
            &mut reg,
            &post_unmount,
            &fq("/L/zone/lost"),
            Some(&fq("/L/zone")),
            &fq("/L"),
            rect(0.0, 0.0, 10.0, 10.0),
        )
        .expect("focus_lost emits");

    assert_eq!(event.next_fq, Some(fq("/L/zone/sibling")));
}

/// Parent-zone last-focused fallback: when there is no sibling but the
/// parent zone has a recorded `last_focused_by_fq`, that target wins.
#[test]
fn focus_lost_picks_parent_zone_last_focused() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer_node("/L", "main", None));
    let mut state = SpatialState::new();

    // Pre-unmount: outer zone has a remembered child + an inner zone
    // containing only the lost FQM.
    let pre_unmount = NavSnapshot {
        layer_fq: fq("/L"),
        scopes: vec![
            snap("/L/outer", None, rect(0.0, 0.0, 200.0, 200.0)),
            snap(
                "/L/outer/inner",
                Some("/L/outer"),
                rect(0.0, 0.0, 100.0, 100.0),
            ),
            snap(
                "/L/outer/inner/lost",
                Some("/L/outer/inner"),
                rect(0.0, 0.0, 10.0, 10.0),
            ),
            snap(
                "/L/outer/remembered",
                Some("/L/outer"),
                rect(150.0, 150.0, 10.0, 10.0),
            ),
        ],
    };

    // Stake the map: focus remembered first, then focus the lost FQM.
    state
        .focus(&mut reg, &pre_unmount, fq("/L/outer/remembered"))
        .expect("focus remembered seeds last_focused_by_fq[outer]");
    state
        .focus(&mut reg, &pre_unmount, fq("/L/outer/inner/lost"))
        .expect("focus lost overwrites last_focused_by_fq for outer");

    // Re-stake the map by focusing remembered then lost so that
    // last_focused_by_fq[outer] points at the lost FQM (last winner).
    // To exercise the cascade, manually set the slot to remembered:
    reg.last_focused_by_fq.clear();
    reg.last_focused_by_fq
        .insert(fq("/L/outer"), fq("/L/outer/remembered"));

    // Post-unmount: only outer + inner + remembered survive.
    let post_unmount = NavSnapshot {
        layer_fq: fq("/L"),
        scopes: vec![
            snap("/L/outer", None, rect(0.0, 0.0, 200.0, 200.0)),
            snap(
                "/L/outer/inner",
                Some("/L/outer"),
                rect(0.0, 0.0, 100.0, 100.0),
            ),
            snap(
                "/L/outer/remembered",
                Some("/L/outer"),
                rect(150.0, 150.0, 10.0, 10.0),
            ),
        ],
    };

    let event = state
        .focus_lost(
            &mut reg,
            &post_unmount,
            &fq("/L/outer/inner/lost"),
            Some(&fq("/L/outer/inner")),
            &fq("/L"),
            rect(0.0, 0.0, 10.0, 10.0),
        )
        .expect("focus_lost emits");

    assert_eq!(event.next_fq, Some(fq("/L/outer/remembered")));
}

/// No live targets in the layer → cascade falls through to NoFocus and
/// the window's focus slot is cleared.
#[test]
fn focus_lost_emits_none_when_layer_is_empty() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer_node("/L", "main", None));
    let mut state = SpatialState::new();

    // Seed focus on a single scope.
    let pre = NavSnapshot {
        layer_fq: fq("/L"),
        scopes: vec![snap("/L/lost", None, rect(0.0, 0.0, 10.0, 10.0))],
    };
    state
        .focus(&mut reg, &pre, fq("/L/lost"))
        .expect("focus seed");

    // Post-unmount: the layer is empty.
    let post = NavSnapshot {
        layer_fq: fq("/L"),
        scopes: vec![],
    };
    let event = state
        .focus_lost(
            &mut reg,
            &post,
            &fq("/L/lost"),
            None,
            &fq("/L"),
            rect(0.0, 0.0, 10.0, 10.0),
        )
        .expect("focus_lost emits");

    assert_eq!(event.next_fq, None);
    assert_eq!(event.prev_fq, Some(fq("/L/lost")));
    assert!(state
        .focused_in(&WindowLabel::from_string("main"))
        .is_none());
}

/// `focus_lost` on a non-focused FQM is a no-op.
#[test]
fn focus_lost_on_unfocused_fq_is_noop() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer_node("/L", "main", None));
    let mut state = SpatialState::new();

    let pre = NavSnapshot {
        layer_fq: fq("/L"),
        scopes: vec![
            snap("/L/a", None, rect(0.0, 0.0, 10.0, 10.0)),
            snap("/L/b", None, rect(20.0, 0.0, 10.0, 10.0)),
        ],
    };
    state.focus(&mut reg, &pre, fq("/L/a")).expect("focus a");

    let post = NavSnapshot {
        layer_fq: fq("/L"),
        scopes: vec![snap("/L/a", None, rect(0.0, 0.0, 10.0, 10.0))],
    };

    // `/L/b` was never focused — focus_lost should be a no-op.
    let event = state.focus_lost(
        &mut reg,
        &post,
        &fq("/L/b"),
        None,
        &fq("/L"),
        rect(20.0, 0.0, 10.0, 10.0),
    );

    assert!(event.is_none());
    assert_eq!(
        state.focused_in(&WindowLabel::from_string("main")),
        Some(&fq("/L/a")),
        "unrelated focus must not be perturbed",
    );
}
