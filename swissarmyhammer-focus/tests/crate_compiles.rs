//! Smoke test: every public type re-exported from the crate root is
//! reachable and constructible.
//!
//! The check is structural — if any of the types disappear from the
//! crate-root re-exports (e.g. an accidental `pub(crate)` regression),
//! this test fails to compile before any other consumer sees a
//! breaking change.
//!
//! Mirrors the canonical `crate_compiles.rs` pattern used elsewhere in
//! the workspace (e.g. `swissarmyhammer-fields`) where the test exists
//! purely to pin the public surface against silent removal.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    BeamNavStrategy, Direction, FocusChangedEvent, FocusEventSink, FocusLayer, FocusScope,
    FocusZone, FullyQualifiedMoniker, LayerName, NavStrategy, NoopSink, Pixels, RecordingSink,
    Rect, SegmentMoniker, SpatialRegistry, SpatialState, WindowLabel,
};

/// Touch every public type so the import block above is not dead code.
/// Keeps the smoke test honest: "compiles" means each name is actually
/// reachable, not just parseable as an import path.
#[test]
fn every_public_type_is_constructible_or_referenced() {
    let _direction = Direction::Up;
    let _pixels = Pixels::new(0.0);
    let _rect = Rect {
        x: Pixels::new(0.0),
        y: Pixels::new(0.0),
        width: Pixels::new(0.0),
        height: Pixels::new(0.0),
    };
    let _segment = SegmentMoniker::from_string("k");
    let _fq = FullyQualifiedMoniker::from_string("/L/k");
    let _layer_fq = FullyQualifiedMoniker::from_string("/L");
    let _name = LayerName::from_string("window");
    let _window = WindowLabel::from_string("main");

    // The leaf scope primitive — the Rust peer of React's `<FocusScope>`.
    let _scope = FocusScope {
        fq: FullyQualifiedMoniker::from_string("/L/k"),
        segment: SegmentMoniker::from_string("k"),
        rect: _rect,
        layer_fq: FullyQualifiedMoniker::from_string("/L"),
        parent_zone: None,
        overrides: HashMap::new(),
    };
    let _zone = FocusZone {
        fq: FullyQualifiedMoniker::from_string("/L/z"),
        segment: SegmentMoniker::from_string("z"),
        rect: _rect,
        layer_fq: FullyQualifiedMoniker::from_string("/L"),
        parent_zone: None,
        last_focused: None,
        overrides: HashMap::new(),
    };
    let _flayer = FocusLayer {
        fq: FullyQualifiedMoniker::from_string("/L"),
        segment: SegmentMoniker::from_string("L"),
        name: LayerName::from_string("window"),
        parent: None,
        window_label: WindowLabel::from_string("main"),
        last_focused: None,
    };
    let _event = FocusChangedEvent {
        window_label: WindowLabel::from_string("main"),
        prev_fq: None,
        next_fq: None,
        next_segment: None,
    };

    let _registry = SpatialRegistry::new();
    let _state = SpatialState::new();

    let _strategy: Box<dyn NavStrategy> = Box::new(BeamNavStrategy::new());
    let _noop: Box<dyn FocusEventSink> = Box::new(NoopSink);
    let _recorder: Box<dyn FocusEventSink> = Box::new(RecordingSink::new());
}
