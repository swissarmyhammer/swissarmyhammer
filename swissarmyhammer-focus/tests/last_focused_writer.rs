//! Integration tests pinning the kernel writer for
//! `FocusScope.last_focused` and `FocusLayer.last_focused`.
//!
//! These tests exercise the production cascade arms
//! [`FallbackResolution::FallbackParentZoneLastFocused`] and
//! [`FallbackResolution::FallbackParentLayerLastFocused`] without
//! hand-populating the `last_focused` slots — the kernel must originate
//! the writes itself as focus moves into deeply nested scopes.
//!
//! Before the writer existed, the cascade arms were unreachable in
//! production: the `last_focused` field was reserved drill-out memory
//! that nothing wrote to. The corresponding fixture-driven tests in
//! `tests/fallback.rs` hand-set `last_focused` at registration time and
//! so could not catch the latent missing-writer bug.
//!
//! Each test below builds a registry, focuses one scope to record the
//! desired `last_focused` slot, focuses a separate "lost" scope inside
//! a child container, and verifies the resolver picks the recorded
//! path when the lost scope is unregistered. The test geometry includes
//! a *closer* candidate so a fall-through to the nearest-scan arm would
//! land on a different FQM — the assertion proves the resolver picked
//! the recorded path, not nearest.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    FallbackResolution, FocusLayer, FocusScope, FullyQualifiedMoniker, LayerName,
    Pixels, Rect, SegmentMoniker, SpatialRegistry, SpatialState, WindowLabel,
};

/// Build a `Rect` from raw `f64` coordinates.
fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        x: Pixels::new(x),
        y: Pixels::new(y),
        width: Pixels::new(w),
        height: Pixels::new(h),
    }
}

/// FQM for a primitive registered directly under a parent path, e.g.
/// `fq("/L", "ui:foo")` -> `/L/ui:foo`.
fn fq(parent: &str, segment: &str) -> FullyQualifiedMoniker {
    FullyQualifiedMoniker::from_string(format!("{parent}/{segment}"))
}

/// Build a [`FocusScope`] with the given identity and rect. `last_focused`
/// always starts as `None`; the kernel writer is expected to populate it
/// as focus moves through descendants of this scope.
fn scope(
    fq_str: &str,
    segment: &str,
    layer: &str,
    parent_zone: Option<FullyQualifiedMoniker>,
    r: Rect,
) -> FocusScope {
    FocusScope {
        fq: FullyQualifiedMoniker::from_string(fq_str),
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone,
        last_focused: None,
        overrides: HashMap::new(),
    }
}

/// Build a [`FocusLayer`] tied to a window. `last_focused` always starts
/// as `None`; the kernel writer is expected to populate it as focus
/// moves through descendants of this layer.
fn layer(fq_str: &str, segment: &str, window: &str, parent: Option<&str>) -> FocusLayer {
    FocusLayer {
        fq: FullyQualifiedMoniker::from_string(fq_str),
        segment: SegmentMoniker::from_string(segment),
        name: LayerName::from_string("window"),
        parent: parent.map(FullyQualifiedMoniker::from_string),
        window_label: WindowLabel::from_string(window),
        last_focused: None,
    }
}

// ---------------------------------------------------------------------------
// FallbackParentZoneLastFocused — kernel writer pins the scope arm
// ---------------------------------------------------------------------------

/// Pins the scope `last_focused` writer.
///
/// Layout: `/L` window root layer → `outer` zone → either `inner`
/// (which holds `lost`) or sibling leaves of `inner`. The "remembered"
/// leaf sits in `outer` at a far rect; a "nearest-other" leaf sits in
/// `outer` close to the lost rect so a nearest-scan fallback would
/// pick `nearest-other`, NOT `remembered`.
///
/// Sequence:
/// 1. focus `lost` (deeply nested in inner): writer records lost on
///    inner.last_focused AND outer.last_focused.
/// 2. focus `remembered` (a sibling of inner inside outer): writer
///    overwrites outer.last_focused = remembered (writer walks the
///    new focus's ancestor chain, not the old one).
/// 3. resolve_fallback(lost): inner is empty after excluding `lost`,
///    so phase 1 walks up to outer; outer.last_focused = remembered
///    is registered and same-window → returns
///    `FallbackParentZoneLastFocused(remembered)`.
///
/// Without the writer, outer.last_focused stays `None` and the
/// resolver falls through to `FallbackParentZoneNearest(nearest-other)`
/// — making the assertion fail.
#[test]
fn writer_makes_parent_zone_last_focused_reachable() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));

    let outer_fq = fq("/L", "ui:outer");
    reg.register_scope(scope(
        outer_fq.as_ref(),
        "ui:outer",
        "/L",
        None,
        rect(0.0, 0.0, 500.0, 500.0),
    ));

    let remembered_fq = FullyQualifiedMoniker::compose(
        &outer_fq,
        &SegmentMoniker::from_string("ui:remembered"),
    );
    reg.register_scope(scope(
        remembered_fq.as_ref(),
        "ui:remembered",
        "/L",
        Some(outer_fq.clone()),
        rect(400.0, 400.0, 10.0, 10.0),
    ));

    // A *closer* sibling that would win on `FallbackParentZoneNearest`.
    // Its presence proves the resolver picked `remembered` via
    // `last_focused` rather than nearest-scan.
    let nearest_other_fq = FullyQualifiedMoniker::compose(
        &outer_fq,
        &SegmentMoniker::from_string("ui:nearest-other"),
    );
    reg.register_scope(scope(
        nearest_other_fq.as_ref(),
        "ui:nearest-other",
        "/L",
        Some(outer_fq.clone()),
        rect(20.0, 20.0, 10.0, 10.0),
    ));

    let inner_fq = FullyQualifiedMoniker::compose(
        &outer_fq,
        &SegmentMoniker::from_string("ui:inner"),
    );
    reg.register_scope(scope(
        inner_fq.as_ref(),
        "ui:inner",
        "/L",
        Some(outer_fq),
        rect(0.0, 0.0, 100.0, 100.0),
    ));

    let lost_fq =
        FullyQualifiedMoniker::compose(&inner_fq, &SegmentMoniker::from_string("ui:lost"));
    reg.register_scope(scope(
        lost_fq.as_ref(),
        "ui:lost",
        "/L",
        Some(inner_fq.clone()),
        rect(0.0, 0.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state
        .focus(&mut reg, lost_fq.clone())
        .expect("focus lost — writer records on inner + outer");
    state
        .focus(&mut reg, remembered_fq.clone())
        .expect("focus remembered — writer overwrites outer.last_focused");

    let resolution = state.resolve_fallback(&reg, &lost_fq);
    match resolution {
        FallbackResolution::FallbackParentZoneLastFocused(target_fq, target_segment) => {
            assert_eq!(
                target_fq, remembered_fq,
                "outer.last_focused must point at `remembered` (set by the kernel writer)",
            );
            assert_eq!(
                target_segment,
                SegmentMoniker::from_string("ui:remembered"),
            );
        }
        other => panic!(
            "expected FallbackParentZoneLastFocused (cascade arm pinned by the kernel writer), got {other:?}",
        ),
    }
}

// ---------------------------------------------------------------------------
// FallbackParentLayerLastFocused — kernel writer pins the layer arm
// ---------------------------------------------------------------------------

/// Pins the layer `last_focused` writer.
///
/// Layout: `/root` parent layer holds `root_leaf` (the recorded path)
/// and `nearest_other` (a closer leaf that would win on nearest-scan).
/// `/root/child` holds the lost leaf alone.
///
/// Sequence:
/// 1. focus `lost` (in `/root/child`): writer records lost on
///    child.last_focused AND root.last_focused.
/// 2. focus `root_leaf` (in `/root`): writer overwrites
///    root.last_focused = root_leaf (the writer walks the new focus's
///    ancestor layer chain).
/// 3. resolve_fallback(lost): phase 1 finds `/root/child` empty after
///    excluding `lost` (its sole leaf); phase 2 walks to `/root`;
///    root.last_focused = root_leaf is registered and same-window →
///    returns `FallbackParentLayerLastFocused(root_leaf)`.
///
/// Without the writer, root.last_focused stays `None` and the
/// resolver falls through to `FallbackParentLayerNearest(nearest-other)`
/// — making the assertion fail.
#[test]
fn writer_makes_parent_layer_last_focused_reachable() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/root", "root", "main", None));
    reg.push_layer(layer("/root/child", "child", "main", Some("/root")));

    let root_leaf_fq = fq("/root", "ui:root-leaf");
    reg.register_scope(scope(
        root_leaf_fq.as_ref(),
        "ui:root-leaf",
        "/root",
        None,
        rect(20.0, 20.0, 10.0, 10.0),
    ));

    // A *closer* root-layer leaf that would win on
    // `FallbackParentLayerNearest`. Its presence proves the resolver
    // picked `root_leaf` via `last_focused` rather than
    // nearest-in-layer.
    let nearest_other_fq = fq("/root", "ui:nearest-other");
    reg.register_scope(scope(
        nearest_other_fq.as_ref(),
        "ui:nearest-other",
        "/root",
        None,
        rect(0.0, 0.0, 5.0, 5.0),
    ));

    let lost_fq = fq("/root/child", "ui:lost");
    reg.register_scope(scope(
        lost_fq.as_ref(),
        "ui:lost",
        "/root/child",
        None,
        rect(100.0, 100.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state
        .focus(&mut reg, lost_fq.clone())
        .expect("focus lost — writer records on child + root layers");
    state
        .focus(&mut reg, root_leaf_fq.clone())
        .expect("focus root_leaf — writer overwrites root.last_focused");

    let resolution = state.resolve_fallback(&reg, &lost_fq);
    match resolution {
        FallbackResolution::FallbackParentLayerLastFocused(target_fq, target_segment) => {
            assert_eq!(
                target_fq, root_leaf_fq,
                "root.last_focused must point at `root_leaf` (set by the kernel writer)",
            );
            assert_eq!(
                target_segment,
                SegmentMoniker::from_string("ui:root-leaf"),
            );
        }
        other => panic!(
            "expected FallbackParentLayerLastFocused (cascade arm pinned by the kernel writer), got {other:?}",
        ),
    }
}
