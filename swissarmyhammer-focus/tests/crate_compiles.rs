//! Smoke test: every public type re-exported from the crate root is
//! reachable and constructible.
//!
//! The check is structural — if any of the types disappear from the
//! crate-root re-exports, this test fails to compile before any other
//! consumer sees a breaking change.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    Direction, FocusChangedEvent, FocusEventSink, FocusLayer, FocusOverrides,
    FullyQualifiedMoniker, IndexedSnapshot, LayerName, NavSnapshot, NoopSink, Pixels,
    RecordingSink, Rect, SegmentMoniker, SnapshotScope, SpatialRegistry, SpatialState, WindowLabel,
};

/// Touch every public type so the import block above is not dead code.
#[test]
fn every_public_type_is_constructible_or_referenced() {
    let _direction = Direction::Up;
    let _pixels = Pixels::new(0.0);
    let rect = Rect {
        x: Pixels::new(0.0),
        y: Pixels::new(0.0),
        width: Pixels::new(0.0),
        height: Pixels::new(0.0),
    };
    let _segment = SegmentMoniker::from_string("k");
    let layer_fq = FullyQualifiedMoniker::from_string("/L");
    let _name = LayerName::from_string("window");
    let _window = WindowLabel::from_string("main");

    let _flayer = FocusLayer {
        fq: layer_fq.clone(),
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

    let _overrides: FocusOverrides = HashMap::new();
    let snapshot = NavSnapshot {
        layer_fq: layer_fq.clone(),
        scopes: vec![SnapshotScope {
            fq: FullyQualifiedMoniker::from_string("/L/k"),
            rect,
            parent_zone: None,
            nav_override: HashMap::new(),
        }],
    };
    let _indexed = IndexedSnapshot::new(&snapshot);

    let _registry = SpatialRegistry::new();
    let _state = SpatialState::new();

    let _noop: Box<dyn FocusEventSink> = Box::new(NoopSink);
    let _recorder: Box<dyn FocusEventSink> = Box::new(RecordingSink::new());
}
