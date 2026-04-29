//! Layer 1 tests for the path-monikers refactor.
//!
//! These tests pin the kernel-level contract that the spatial-nav kernel
//! uses **one** identifier shape per primitive: the
//! [`FullyQualifiedMoniker`]. The path IS the spatial key. Consumers
//! declare a relative [`SegmentMoniker`] when constructing a primitive,
//! and the FQM is composed by parent/child nesting on the consumer side
//! before being passed to the kernel.
//!
//! This file is the source-of-truth pin for the six tests called out in
//! the parent path-monikers card (`01KQD6064G1C1RAXDFPJVT1F46`):
//!
//! - `register_zone_keyed_by_fq_moniker`
//! - `two_zones_same_segment_different_layers_have_distinct_fq_keys`
//! - `find_by_fq_unknown_path_returns_none_and_traces_error`
//! - `cascade_does_not_cross_layers`
//! - `segment_moniker_does_not_compile_at_fq_lookup_callsite` (compile-fail trybuild-style; here asserted via type-system shape only)
//! - `register_with_duplicate_fq_logs_error_and_replaces`
//!
//! The cross-layer cascade test mirrors the bug confirmed in the running
//! app's log: an inspector field zone and a board field zone with the
//! same segment moniker resolve to *distinct* FQMs (one under
//! `/window/inspector/...` and one under `/window/board/.../card:T1/`).
//! ArrowDown inside the inspector must stay inside the inspector layer.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    BeamNavStrategy, Direction, FocusLayer, FocusScope, FocusZone, FullyQualifiedMoniker,
    LayerName, NavStrategy, Pixels, Rect, SegmentMoniker, SpatialRegistry, WindowLabel,
};

// ---------------------------------------------------------------------------
// Shared helpers — keep the assembly readable.
// ---------------------------------------------------------------------------

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

fn seg(s: &str) -> SegmentMoniker {
    SegmentMoniker::from_string(s)
}

fn make_layer(layer_fq: &str, role: &str, parent: Option<&str>) -> FocusLayer {
    FocusLayer {
        fq: fq(layer_fq),
        segment: seg(role),
        name: LayerName::from_string(role),
        parent: parent.map(fq),
        window_label: WindowLabel::from_string("main"),
        last_focused: None,
    }
}

fn make_zone(
    fq_str: &str,
    segment: &str,
    layer_fq: &str,
    parent_zone: Option<&str>,
    r: Rect,
) -> FocusZone {
    FocusZone {
        fq: fq(fq_str),
        segment: seg(segment),
        rect: r,
        layer_fq: fq(layer_fq),
        parent_zone: parent_zone.map(fq),
        last_focused: None,
        overrides: HashMap::new(),
    }
}

#[allow(dead_code)]
fn make_leaf(
    fq_str: &str,
    segment: &str,
    layer_fq: &str,
    parent_zone: Option<&str>,
    r: Rect,
) -> FocusScope {
    FocusScope {
        fq: fq(fq_str),
        segment: seg(segment),
        rect: r,
        layer_fq: fq(layer_fq),
        parent_zone: parent_zone.map(fq),
        overrides: HashMap::new(),
    }
}

// ---------------------------------------------------------------------------
// Test 1: register_zone_keyed_by_fq_moniker
// ---------------------------------------------------------------------------

/// Registering a zone at a specific FQM and looking it up by exact
/// path returns the same registered entry. The FQM IS the key — no
/// UUID sidecar, no leaf-form fallback.
#[test]
fn register_zone_keyed_by_fq_moniker() {
    let mut reg = SpatialRegistry::new();

    reg.push_layer(make_layer("/window", "window", None));
    reg.push_layer(make_layer(
        "/window/inspector",
        "inspector",
        Some("/window"),
    ));

    reg.register_zone(make_zone(
        "/window/inspector/field:T1.title",
        "field:T1.title",
        "/window/inspector",
        None,
        rect(0.0, 0.0, 100.0, 30.0),
    ));

    let found = reg
        .find_by_fq(&fq("/window/inspector/field:T1.title"))
        .expect("registered FQM resolves");
    assert_eq!(found.segment(), &seg("field:T1.title"));
}

// ---------------------------------------------------------------------------
// Test 2: two_zones_same_segment_different_layers_have_distinct_fq_keys
// ---------------------------------------------------------------------------

/// Register two zones whose `SegmentMoniker` is identical but whose
/// FQMs differ — one under the inspector layer, one under the board's
/// card. Both must be findable as distinct entries. This is the
/// structural fix for the production bug where a flat moniker
/// resolved non-deterministically across two registrations.
#[test]
fn two_zones_same_segment_different_layers_have_distinct_fq_keys() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/window", "window", None));
    reg.push_layer(make_layer(
        "/window/inspector",
        "inspector",
        Some("/window"),
    ));

    // Board path: /window/board/column:todo/card:T1/field:T1.title
    reg.register_zone(make_zone(
        "/window/board",
        "board",
        "/window",
        None,
        rect(0.0, 100.0, 800.0, 600.0),
    ));
    reg.register_zone(make_zone(
        "/window/board/column:todo",
        "column:todo",
        "/window",
        Some("/window/board"),
        rect(0.0, 100.0, 400.0, 600.0),
    ));
    reg.register_zone(make_zone(
        "/window/board/column:todo/card:T1",
        "card:T1",
        "/window",
        Some("/window/board/column:todo"),
        rect(0.0, 100.0, 400.0, 100.0),
    ));
    reg.register_zone(make_zone(
        "/window/board/column:todo/card:T1/field:T1.title",
        "field:T1.title",
        "/window",
        Some("/window/board/column:todo/card:T1"),
        rect(8.0, 108.0, 384.0, 30.0),
    ));

    // Inspector path: /window/inspector/field:T1.title
    reg.register_zone(make_zone(
        "/window/inspector/field:T1.title",
        "field:T1.title",
        "/window/inspector",
        None,
        rect(900.0, 100.0, 384.0, 30.0),
    ));

    let board_field = reg
        .find_by_fq(&fq("/window/board/column:todo/card:T1/field:T1.title"))
        .expect("board field FQM resolves");
    let inspector_field = reg
        .find_by_fq(&fq("/window/inspector/field:T1.title"))
        .expect("inspector field FQM resolves");

    // Both share the same SegmentMoniker but live at distinct FQMs.
    assert_eq!(board_field.segment(), &seg("field:T1.title"));
    assert_eq!(inspector_field.segment(), &seg("field:T1.title"));
    assert_ne!(board_field.fq(), inspector_field.fq());
}

// ---------------------------------------------------------------------------
// Test 3: find_by_fq_unknown_path_returns_none_and_traces_error
// ---------------------------------------------------------------------------

/// `find_by_fq` returns `None` for a path that has not been registered.
/// The kernel does not silently substitute — torn / unknown lookup is
/// surfaced via the `None` return and (in higher-level callers) a
/// `tracing::error!` per the no-silent-dropout contract.
#[test]
fn find_by_fq_unknown_path_returns_none_and_traces_error() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/window", "window", None));
    reg.register_zone(make_zone(
        "/window/board",
        "board",
        "/window",
        None,
        rect(0.0, 0.0, 100.0, 100.0),
    ));

    assert!(reg.find_by_fq(&fq("/window/does-not-exist")).is_none());
}

// ---------------------------------------------------------------------------
// Test 4: cascade_does_not_cross_layers
// ---------------------------------------------------------------------------

/// With both an inspector-layer field zone and a board-layer field zone
/// sharing the same `SegmentMoniker`, `next` from the inspector field
/// must never land on the board field — the layer is a hard boundary.
/// Today's bug: the flat `Moniker` lookup picked the board's UUID
/// non-deterministically; with FQMs as keys the lookup is exact and
/// the navigator's per-layer scoping holds.
#[test]
fn cascade_does_not_cross_layers() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/window", "window", None));
    reg.push_layer(make_layer(
        "/window/inspector",
        "inspector",
        Some("/window"),
    ));

    // Inspector layer: a panel zone with two field zones stacked vertically.
    reg.register_zone(make_zone(
        "/window/inspector/panel",
        "panel",
        "/window/inspector",
        None,
        rect(900.0, 100.0, 400.0, 400.0),
    ));
    reg.register_zone(make_zone(
        "/window/inspector/panel/field:T1.title",
        "field:T1.title",
        "/window/inspector",
        Some("/window/inspector/panel"),
        rect(908.0, 108.0, 384.0, 30.0),
    ));
    reg.register_zone(make_zone(
        "/window/inspector/panel/field:T1.status",
        "field:T1.status",
        "/window/inspector",
        Some("/window/inspector/panel"),
        rect(908.0, 148.0, 384.0, 30.0),
    ));

    // Window layer: a board with a card holding a same-segment field zone.
    reg.register_zone(make_zone(
        "/window/board",
        "board",
        "/window",
        None,
        rect(0.0, 100.0, 800.0, 600.0),
    ));
    reg.register_zone(make_zone(
        "/window/board/card:T1",
        "card:T1",
        "/window",
        Some("/window/board"),
        rect(8.0, 108.0, 400.0, 100.0),
    ));
    reg.register_zone(make_zone(
        "/window/board/card:T1/field:T1.title",
        "field:T1.title",
        "/window",
        Some("/window/board/card:T1"),
        rect(16.0, 116.0, 384.0, 30.0),
    ));

    // Press Down from the inspector's title field. The result must be
    // the inspector's status field (or stay-put if nothing matches inside
    // the inspector layer); it must NEVER cross to the board.
    let strategy = BeamNavStrategy::new();
    let from_fq = fq("/window/inspector/panel/field:T1.title");
    let from_segment = seg("field:T1.title");
    let result = strategy.next(&reg, &from_fq, &from_segment, Direction::Down);

    // Whatever the navigator chooses, it must live in the inspector layer.
    let target = reg
        .find_by_fq(&result)
        .expect("navigator returns a registered FQM");
    assert_eq!(
        target.layer_fq(),
        &fq("/window/inspector"),
        "Down from inspector field must stay inside the inspector layer; got {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// Test 5: segment_moniker_does_not_compile_at_fq_lookup_callsite
// ---------------------------------------------------------------------------

/// The newtype safety net: `find_by_fq` accepts only `&FullyQualifiedMoniker`.
/// Passing a `SegmentMoniker` is a compile error. We assert the
/// type-system shape at runtime by verifying that `find_by_fq`'s
/// signature distinguishes the two newtypes — a `SegmentMoniker` and
/// a `FullyQualifiedMoniker` constructed from the same string are NOT
/// interchangeable.
///
/// A true compile-fail check belongs in a separate `trybuild`-driven
/// test crate; this assertion enforces the runtime distinguishability
/// (different types, different `Hash` namespaces) so a downstream
/// rename or accidental `String`-aliasing would still fail this test.
#[test]
fn segment_moniker_does_not_compile_at_fq_lookup_callsite() {
    use std::any::TypeId;

    // The two newtypes must be distinct types — not a type alias.
    assert_ne!(
        TypeId::of::<SegmentMoniker>(),
        TypeId::of::<FullyQualifiedMoniker>(),
        "SegmentMoniker and FullyQualifiedMoniker must be distinct types"
    );

    // Neither newtype is a `String` alias — both are wrappers.
    assert_ne!(TypeId::of::<SegmentMoniker>(), TypeId::of::<String>());
    assert_ne!(
        TypeId::of::<FullyQualifiedMoniker>(),
        TypeId::of::<String>()
    );

    // Compose smoke-check: composition produces a path-shaped FQM.
    let parent = FullyQualifiedMoniker::root(&seg("window"));
    assert_eq!(parent.as_str(), "/window");
    let child = FullyQualifiedMoniker::compose(&parent, &seg("inspector"));
    assert_eq!(child.as_str(), "/window/inspector");
    let grandchild = FullyQualifiedMoniker::compose(&child, &seg("field:T1.title"));
    assert_eq!(grandchild.as_str(), "/window/inspector/field:T1.title");
}

// ---------------------------------------------------------------------------
// Test 6: register_with_duplicate_fq_logs_error_and_replaces
// ---------------------------------------------------------------------------

/// Registering the same FQM twice replaces the prior entry — same
/// semantics as today's "register_zone replaces any prior scope under
/// the same key", just on the new identifier. A real duplicate FQM is
/// a programmer mistake (two `<FocusZone>` with the same composed
/// path), so the kernel surfaces the duplication via `tracing::error!`
/// while keeping the registry in a consistent state.
#[test]
fn register_with_duplicate_fq_logs_error_and_replaces() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/window", "window", None));

    let path = "/window/board/card:T1";

    // First registration.
    reg.register_zone(make_zone(
        path,
        "card:T1",
        "/window",
        None,
        rect(0.0, 0.0, 100.0, 100.0),
    ));
    let first = reg.find_by_fq(&fq(path)).unwrap();
    assert_eq!(first.rect().width, Pixels::new(100.0));

    // Second registration at the same FQM with a different rect — replaces
    // the prior entry. The kernel logs an error to surface the duplicate;
    // we don't capture tracing output here (Layer 2's React tests do), but
    // the registry must end up holding the SECOND entry's data.
    reg.register_zone(make_zone(
        path,
        "card:T1",
        "/window",
        None,
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    let second = reg.find_by_fq(&fq(path)).unwrap();
    assert_eq!(
        second.rect().width,
        Pixels::new(200.0),
        "duplicate FQM registration must replace prior entry"
    );
}
