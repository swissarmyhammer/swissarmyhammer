//! Integration tests pinning the top-level `last_focused_by_fq` map
//! semantics on `SpatialRegistry::record_focus`.
//!
//! Two properties are pinned:
//!
//! 1. After every focus mutation, every scope ancestor walked via the
//!    snapshot's `parent_zone` chain has its `last_focused_by_fq`
//!    entry updated to the focused FQM.
//! 2. `resolve_fallback`'s `FallbackParentZoneLastFocused` arm consults
//!    `last_focused_by_fq` for ancestor lookups.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    FallbackResolution, FocusLayer, FullyQualifiedMoniker, IndexedSnapshot, LayerName,
    LostFocusContext, NavSnapshot, Pixels, Rect, SegmentMoniker, SnapshotScope, SpatialRegistry,
    SpatialState, WindowLabel,
};

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        x: Pixels::new(x),
        y: Pixels::new(y),
        width: Pixels::new(w),
        height: Pixels::new(h),
    }
}

fn fq(parent: &str, segment: &str) -> FullyQualifiedMoniker {
    FullyQualifiedMoniker::from_string(format!("{parent}/{segment}"))
}

fn layer_node(fq_str: &str, segment: &str, window: &str, parent: Option<&str>) -> FocusLayer {
    FocusLayer {
        fq: FullyQualifiedMoniker::from_string(fq_str),
        segment: SegmentMoniker::from_string(segment),
        name: LayerName::from_string("window"),
        parent: parent.map(FullyQualifiedMoniker::from_string),
        window_label: WindowLabel::from_string(window),
        last_focused: None,
    }
}

fn snap_scope(
    fq: FullyQualifiedMoniker,
    parent_zone: Option<FullyQualifiedMoniker>,
    r: Rect,
) -> SnapshotScope {
    SnapshotScope {
        fq,
        rect: r,
        parent_zone,
        nav_override: HashMap::new(),
        focusable: true,
    }
}

fn build_three_level_snapshot() -> (
    NavSnapshot,
    FullyQualifiedMoniker,
    FullyQualifiedMoniker,
    FullyQualifiedMoniker,
    FullyQualifiedMoniker,
) {
    let layer_fq = FullyQualifiedMoniker::from_string("/L");
    let outer_fq = fq("/L", "ui:outer");
    let inner_fq =
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:inner"));
    let remembered_fq =
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:remembered"));
    let lost_fq =
        FullyQualifiedMoniker::compose(&inner_fq, &SegmentMoniker::from_string("ui:lost"));

    let snapshot = NavSnapshot {
        layer_fq,
        scopes: vec![
            snap_scope(outer_fq.clone(), None, rect(0.0, 0.0, 500.0, 500.0)),
            snap_scope(
                inner_fq.clone(),
                Some(outer_fq.clone()),
                rect(0.0, 0.0, 100.0, 100.0),
            ),
            snap_scope(
                remembered_fq.clone(),
                Some(outer_fq.clone()),
                rect(400.0, 400.0, 10.0, 10.0),
            ),
            snap_scope(
                lost_fq.clone(),
                Some(inner_fq.clone()),
                rect(0.0, 0.0, 10.0, 10.0),
            ),
        ],
    };
    (snapshot, outer_fq, inner_fq, remembered_fq, lost_fq)
}

#[test]
fn focus_writes_last_focused_by_fq_for_each_ancestor() {
    let (snapshot, outer_fq, inner_fq, _remembered_fq, lost_fq) = build_three_level_snapshot();
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer_node("/L", "L", "main", None));

    let mut state = SpatialState::new();
    state
        .focus(&mut reg, &snapshot, lost_fq.clone())
        .expect("focus emits an event");

    assert_eq!(reg.last_focused_by_fq.get(&inner_fq), Some(&lost_fq));
    assert_eq!(reg.last_focused_by_fq.get(&outer_fq), Some(&lost_fq));
}

#[test]
fn second_focus_overwrites_ancestor_entries() {
    let (snapshot, outer_fq, _inner_fq, remembered_fq, lost_fq) = build_three_level_snapshot();
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer_node("/L", "L", "main", None));

    let mut state = SpatialState::new();
    state
        .focus(&mut reg, &snapshot, lost_fq)
        .expect("first focus emits");
    state
        .focus(&mut reg, &snapshot, remembered_fq.clone())
        .expect("second focus emits");

    assert_eq!(reg.last_focused_by_fq.get(&outer_fq), Some(&remembered_fq));
}

#[test]
fn empty_registry_has_empty_map() {
    let reg = SpatialRegistry::new();
    assert!(reg.last_focused_by_fq.is_empty());
}

#[test]
fn fallback_reads_parent_zone_last_focused_from_map() {
    // Build a registry whose `last_focused_by_fq[outer]` points at
    // `remembered`. Then run resolve_fallback with `lost` as the lost
    // FQM (parent zone = inner, ancestor = outer); the cascade should
    // pick up the remembered entry from the map for the outer zone.
    let (snapshot, outer_fq, inner_fq, remembered_fq, lost_fq) = build_three_level_snapshot();
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer_node("/L", "L", "main", None));

    // Stake the map by focusing remembered first.
    let mut state = SpatialState::new();
    state
        .focus(&mut reg, &snapshot, remembered_fq.clone())
        .expect("focus remembered");

    assert_eq!(reg.last_focused_by_fq.get(&outer_fq), Some(&remembered_fq));

    // Build a "live" snapshot that omits the lost FQM but keeps inner +
    // outer + remembered live.
    let live_snapshot = NavSnapshot {
        layer_fq: FullyQualifiedMoniker::from_string("/L"),
        scopes: vec![
            snap_scope(outer_fq.clone(), None, rect(0.0, 0.0, 500.0, 500.0)),
            snap_scope(
                inner_fq.clone(),
                Some(outer_fq.clone()),
                rect(0.0, 0.0, 100.0, 100.0),
            ),
            snap_scope(
                remembered_fq.clone(),
                Some(outer_fq.clone()),
                rect(400.0, 400.0, 10.0, 10.0),
            ),
        ],
    };
    let view = IndexedSnapshot::new(&live_snapshot);
    let ctx = LostFocusContext {
        view: &view,
        lost_layer_fq: FullyQualifiedMoniker::from_string("/L"),
        lost_parent_zone: Some(inner_fq),
        lost_rect: rect(0.0, 0.0, 10.0, 10.0),
    };

    let resolution = state.resolve_fallback(&reg, &lost_fq, &ctx);
    match resolution {
        FallbackResolution::FallbackParentZoneLastFocused(target_fq, _) => {
            assert_eq!(target_fq, remembered_fq);
        }
        other => panic!("expected FallbackParentZoneLastFocused for remembered, got {other:?}",),
    }
}
