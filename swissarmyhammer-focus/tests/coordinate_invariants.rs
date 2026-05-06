//! Integration test for the coordinate-system invariant validators.
//!
//! Pins the contract from
//! `swissarmyhammer-focus/src/registry.rs::validate_rect_invariants`
//! and `validate_coordinate_consistency`:
//!
//!   1. The kernel emits log events on bad input but never panics or
//!      returns an error — registration / nav stays best-effort.
//!   2. With a mix of viewport-relative and document-relative rects in
//!      one layer, beam search still produces *some* FQM (the
//!      no-silent-dropout contract holds even with bad input).
//!
//! The validators themselves (debug-only `tracing::error!` /
//! `tracing::warn!` events) are unit-tested in `registry::tests`; this
//! integration test focuses on the user-visible contract that the
//! kernel does not crash or refuse to compute on bad geometry.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    BeamNavStrategy, Direction, FocusLayer, FocusScope, FullyQualifiedMoniker, LayerName,
    NavStrategy, Pixels, Rect, SegmentMoniker, SpatialRegistry, WindowLabel,
};

// ---------------------------------------------------------------------------
// Builders
// ---------------------------------------------------------------------------

/// Build a `Rect` from raw f64 components. Tests use the kernel's
/// `Pixels::new` constructor everywhere; this helper hides the noise.
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

fn segment(s: &str) -> SegmentMoniker {
    SegmentMoniker::from_string(s)
}

fn make_layer(layer_fq: &str, window: &str) -> FocusLayer {
    FocusLayer {
        fq: fq(layer_fq),
        segment: segment(layer_fq.rsplit('/').next().unwrap_or(layer_fq)),
        name: LayerName::from_string("window"),
        parent: None,
        window_label: WindowLabel::from_string(window),
        last_focused: None,
    }
}

fn make_zone(fq_str: &str, layer: &str, parent_zone: Option<&str>, r: Rect) -> FocusScope {
    FocusScope {
        fq: fq(fq_str),
        segment: segment(fq_str.rsplit('/').next().unwrap_or(fq_str)),
        rect: r,
        layer_fq: fq(layer),
        parent_zone: parent_zone.map(fq),
        last_focused: None,
        overrides: HashMap::new(),
    }
}

fn make_scope(fq_str: &str, layer: &str, parent_zone: Option<&str>, r: Rect) -> FocusScope {
    FocusScope {
        fq: fq(fq_str),
        segment: segment(fq_str.rsplit('/').next().unwrap_or(fq_str)),
        rect: r,
        layer_fq: fq(layer),
        parent_zone: parent_zone.map(fq),
        overrides: HashMap::new(),
        last_focused: None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// A registry assembled with rects in mixed coordinate systems — half
/// viewport-relative (`(0..200, 0..100)`), half document-relative
/// (`(50000..50200, 30000..30100)` — far beyond any real viewport) —
/// must not panic when navigated. The kernel is best-effort: it logs
/// the violation and keeps returning FQMs from beam search.
#[test]
fn nav_with_mixed_coordinate_systems_does_not_panic() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", "main"));
    reg.register_scope(make_zone(
        "/L/parent",
        "/L",
        None,
        rect(0.0, 0.0, 200.0, 100.0),
    ));

    // Viewport-relative leaves — clustered near the origin.
    reg.register_scope(make_scope(
        "/L/parent/a",
        "/L",
        Some("/L/parent"),
        rect(0.0, 0.0, 50.0, 30.0),
    ));
    reg.register_scope(make_scope(
        "/L/parent/b",
        "/L",
        Some("/L/parent"),
        rect(100.0, 0.0, 50.0, 30.0),
    ));

    // Document-relative leaves — far beyond any plausible viewport,
    // mimicking the bug class where a callsite computed `offsetTop`
    // / `offsetLeft` instead of `getBoundingClientRect()`.
    reg.register_scope(make_scope(
        "/L/parent/c",
        "/L",
        Some("/L/parent"),
        rect(50_000.0, 30_000.0, 50.0, 30.0),
    ));
    reg.register_scope(make_scope(
        "/L/parent/d",
        "/L",
        Some("/L/parent"),
        rect(50_100.0, 30_000.0, 50.0, 30.0),
    ));

    let strategy = BeamNavStrategy;

    // Drive every cardinal direction from a viewport-relative leaf.
    for dir in [
        Direction::Up,
        Direction::Down,
        Direction::Left,
        Direction::Right,
    ] {
        let target = strategy.next(&reg, &fq("/L/parent/a"), &segment("a"), dir);
        // The no-silent-dropout contract: every nav must return a
        // valid FQM in the same layer (or stay-put on the focused
        // FQM). That contract holds even when half the candidate
        // rects are nonsense.
        let entry = reg
            .find_by_fq(&target)
            .expect("nav target must resolve in registry");
        assert_eq!(
            entry.layer_fq,
            fq("/L"),
            "nav must stay within the layer even with mixed-coord rects"
        );
    }

    // Drive every cardinal direction from a document-relative leaf.
    for dir in [
        Direction::Up,
        Direction::Down,
        Direction::Left,
        Direction::Right,
    ] {
        let target = strategy.next(&reg, &fq("/L/parent/c"), &segment("c"), dir);
        let entry = reg
            .find_by_fq(&target)
            .expect("nav target must resolve in registry");
        assert_eq!(entry.layer_fq, fq("/L"));
    }
}

/// Registering a zero-size or negative-dim rect must not panic — the
/// kernel logs the violation and keeps the registry consistent so
/// subsequent nav still produces results.
#[test]
fn registration_with_bad_rects_does_not_panic() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", "main"));
    reg.register_scope(make_zone(
        "/L/parent",
        "/L",
        None,
        rect(0.0, 0.0, 200.0, 100.0),
    ));

    // Negative width — would never come from `getBoundingClientRect()`.
    reg.register_scope(make_scope(
        "/L/parent/neg",
        "/L",
        Some("/L/parent"),
        rect(0.0, 0.0, -10.0, 30.0),
    ));
    // Zero height.
    reg.register_scope(make_scope(
        "/L/parent/zero",
        "/L",
        Some("/L/parent"),
        rect(50.0, 0.0, 50.0, 0.0),
    ));
    // NaN x — every comparison on NaN folds to Equal via `pixels_cmp`,
    // so the kernel still produces consistent (if arbitrary) order.
    reg.register_scope(make_scope(
        "/L/parent/nan",
        "/L",
        Some("/L/parent"),
        rect(f64::NAN, 0.0, 50.0, 30.0),
    ));
    // Sane sibling for nav to land on.
    reg.register_scope(make_scope(
        "/L/parent/ok",
        "/L",
        Some("/L/parent"),
        rect(150.0, 0.0, 50.0, 30.0),
    ));

    let strategy = BeamNavStrategy;
    // Even with three pathological siblings, nav from the sane leaf
    // returns a valid FQM in the same layer — the no-silent-dropout
    // contract holds.
    for dir in [
        Direction::Up,
        Direction::Down,
        Direction::Left,
        Direction::Right,
    ] {
        let target = strategy.next(&reg, &fq("/L/parent/ok"), &segment("ok"), dir);
        let entry = reg
            .find_by_fq(&target)
            .expect("nav target must resolve in registry");
        assert_eq!(entry.layer_fq, fq("/L"));
    }
}

/// `validate_coordinate_consistency` is best-effort: even on a layer
/// with badly-mixed rects, calling it does not panic and does not
/// affect the rest of the registry's state. Subsequent registration
/// and nav still work normally.
#[test]
fn validate_coordinate_consistency_is_observability_only() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", "main"));
    reg.register_scope(make_zone(
        "/L/parent",
        "/L",
        None,
        rect(0.0, 0.0, 200.0, 100.0),
    ));
    // Four near rects + one far rect — the canonical
    // half-viewport-relative-half-document-relative shape.
    reg.register_scope(make_scope(
        "/L/parent/a",
        "/L",
        Some("/L/parent"),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(make_scope(
        "/L/parent/b",
        "/L",
        Some("/L/parent"),
        rect(10.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(make_scope(
        "/L/parent/c",
        "/L",
        Some("/L/parent"),
        rect(0.0, 10.0, 10.0, 10.0),
    ));
    reg.register_scope(make_scope(
        "/L/parent/d",
        "/L",
        Some("/L/parent"),
        rect(10.0, 10.0, 10.0, 10.0),
    ));
    reg.register_scope(make_scope(
        "/L/parent/far",
        "/L",
        Some("/L/parent"),
        rect(100_000.0, 100_000.0, 10.0, 10.0),
    ));

    // The validator runs without panicking and without disturbing
    // the registry — the post-validation lookups all succeed.
    reg.validate_coordinate_consistency(&fq("/L"));
    assert!(reg.find_by_fq(&fq("/L/parent/a")).is_some());
    assert!(reg.find_by_fq(&fq("/L/parent/far")).is_some());

    // Nav from the cluster still works.
    let strategy = BeamNavStrategy;
    let target = strategy.next(&reg, &fq("/L/parent/a"), &segment("a"), Direction::Right);
    assert!(reg.find_by_fq(&target).is_some());
}
