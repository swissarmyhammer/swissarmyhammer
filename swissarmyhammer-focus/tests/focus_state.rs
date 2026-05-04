//! Integration tests for `swissarmyhammer_focus::SpatialState`.
//!
//! Headless pattern matching `tests/focus_registry.rs` — pure Rust,
//! no Tauri runtime, no jsdom. Every focus mutation runs through the
//! public `SpatialState` surface and is asserted by inspecting the
//! returned `FocusChangedEvent` plus subsequent `focused_in` reads.
//!
//! These tests cover the per-window focus invariants under the
//! path-monikers identifier model:
//!
//! - `focus` updates per-window state and returns a `FocusChangedEvent`
//!   whose `window_label` matches the scope's window (derived from the
//!   registry).
//! - Focus in window A does not perturb `focus_by_window` for window B.
//! - Unregistering the focused scope clears that window's slot only.
//! - `FocusChangedEvent.next_segment` is `Some(scope.segment.clone())`
//!   whenever `next_fq` is `Some`.
//!
//! `SpatialState` does not maintain a per-FQM entry table — the
//! registry is the single source of truth for window/segment metadata.
//! These tests construct a [`SpatialRegistry`] and pass it to every
//! mutating call.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    FocusChangedEvent, FocusLayer, FocusScope, FullyQualifiedMoniker, LayerName, Pixels, Rect,
    SegmentMoniker, SpatialRegistry, SpatialState, WindowLabel,
};

/// Build a single-window registry with a leaf scope at `fq` whose
/// segment is `segment`.
fn registry_with_scope(window: &str, layer: &str, fq: &str, segment: &str) -> SpatialRegistry {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(FocusLayer {
        fq: FullyQualifiedMoniker::from_string(layer),
        segment: SegmentMoniker::from_string("window"),
        name: LayerName::from_string("window"),
        parent: None,
        window_label: WindowLabel::from_string(window),
        last_focused: None,
    });
    reg.register_scope(FocusScope {
        fq: FullyQualifiedMoniker::from_string(fq),
        segment: SegmentMoniker::from_string(segment),
        rect: Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        },
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone: None,
        overrides: HashMap::new(),
        last_focused: None,
    });
    reg
}

/// Add a second leaf scope to an existing registry under the same layer.
fn add_scope(reg: &mut SpatialRegistry, layer: &str, fq: &str, segment: &str) {
    reg.register_scope(FocusScope {
        fq: FullyQualifiedMoniker::from_string(fq),
        segment: SegmentMoniker::from_string(segment),
        rect: Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        },
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone: None,
        overrides: HashMap::new(),
        last_focused: None,
    });
}

/// `focus` updates per-window state and returns a `FocusChangedEvent`
/// whose `window_label` matches the focused scope's window.
#[test]
fn focus_updates_per_window_state_and_emits_with_window_label() {
    let mut registry = registry_with_scope("main", "/L", "/L/scope-1", "task:01ABC");
    let mut state = SpatialState::new();
    let fq = FullyQualifiedMoniker::from_string("/L/scope-1");

    let event = state
        .focus(&mut registry, fq.clone())
        .expect("focus must emit on first move");

    assert_eq!(event.window_label, WindowLabel::from_string("main"));
    assert_eq!(event.prev_fq, None, "no prior focus → prev_fq None");
    assert_eq!(event.next_fq, Some(fq.clone()));
    assert_eq!(
        state.focused_in(&WindowLabel::from_string("main")),
        Some(&fq),
    );
}

/// Focus in window A must not affect `focus_by_window[B]`. Two windows
/// hold independent slots — focus moves in one window cannot displace
/// focus in another.
#[test]
fn focus_in_a_does_not_affect_focus_in_b() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(FocusLayer {
        fq: FullyQualifiedMoniker::from_string("/La"),
        segment: SegmentMoniker::from_string("window"),
        name: LayerName::from_string("window"),
        parent: None,
        window_label: WindowLabel::from_string("window-a"),
        last_focused: None,
    });
    reg.push_layer(FocusLayer {
        fq: FullyQualifiedMoniker::from_string("/Lb"),
        segment: SegmentMoniker::from_string("window"),
        name: LayerName::from_string("window"),
        parent: None,
        window_label: WindowLabel::from_string("window-b"),
        last_focused: None,
    });
    add_scope(&mut reg, "/La", "/La/a-1", "task:A");
    add_scope(&mut reg, "/Lb", "/Lb/b-1", "task:B");
    add_scope(&mut reg, "/La", "/La/a-2", "task:A2");

    let a_fq = FullyQualifiedMoniker::from_string("/La/a-1");
    let b_fq = FullyQualifiedMoniker::from_string("/Lb/b-1");
    let a2_fq = FullyQualifiedMoniker::from_string("/La/a-2");

    let mut state = SpatialState::new();
    state.focus(&mut reg, a_fq.clone()).expect("focus a");
    state.focus(&mut reg, b_fq.clone()).expect("focus b");

    let event = state.focus(&mut reg, a2_fq.clone()).expect("focus a2");

    assert_eq!(event.window_label, WindowLabel::from_string("window-a"));
    assert_eq!(event.prev_fq, Some(a_fq));
    assert_eq!(event.next_fq, Some(a2_fq.clone()));
    assert_eq!(
        state.focused_in(&WindowLabel::from_string("window-a")),
        Some(&a2_fq),
    );
    assert_eq!(
        state.focused_in(&WindowLabel::from_string("window-b")),
        Some(&b_fq),
        "window B's focus must not have changed"
    );
}

/// Unregistering the focused scope clears that window's focus slot, and
/// emits a `Some(prev) → None` event.
#[test]
fn unregister_of_focused_fq_clears_only_that_windows_focus() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(FocusLayer {
        fq: FullyQualifiedMoniker::from_string("/La"),
        segment: SegmentMoniker::from_string("window"),
        name: LayerName::from_string("window"),
        parent: None,
        window_label: WindowLabel::from_string("window-a"),
        last_focused: None,
    });
    reg.push_layer(FocusLayer {
        fq: FullyQualifiedMoniker::from_string("/Lb"),
        segment: SegmentMoniker::from_string("window"),
        name: LayerName::from_string("window"),
        parent: None,
        window_label: WindowLabel::from_string("window-b"),
        last_focused: None,
    });
    add_scope(&mut reg, "/La", "/La/a-1", "task:A");
    add_scope(&mut reg, "/Lb", "/Lb/b-1", "task:B");

    let a_fq = FullyQualifiedMoniker::from_string("/La/a-1");
    let b_fq = FullyQualifiedMoniker::from_string("/Lb/b-1");

    let mut state = SpatialState::new();
    state.focus(&mut reg, a_fq.clone()).expect("focus a");
    state.focus(&mut reg, b_fq.clone()).expect("focus b");

    let event = state
        .handle_unregister(&mut reg, &a_fq)
        .expect("unregistering the focused FQM emits a clear event");

    assert_eq!(event.window_label, WindowLabel::from_string("window-a"));
    assert_eq!(event.prev_fq, Some(a_fq));
    assert_eq!(event.next_fq, None);
    assert_eq!(event.next_segment, None);
    assert_eq!(
        state.focused_in(&WindowLabel::from_string("window-a")),
        None,
    );
    assert_eq!(
        state.focused_in(&WindowLabel::from_string("window-b")),
        Some(&b_fq),
    );
}

/// Unregistering an unfocused scope is a silent registry update — no
/// event is emitted because no claim callback needs to fire.
#[test]
fn unregister_of_unfocused_fq_emits_no_event() {
    let mut reg = registry_with_scope("main", "/L", "/L/focused", "task:F");
    add_scope(&mut reg, "/L", "/L/other", "task:O");

    let focused = FullyQualifiedMoniker::from_string("/L/focused");
    let other = FullyQualifiedMoniker::from_string("/L/other");

    let mut state = SpatialState::new();
    state.focus(&mut reg, focused.clone()).expect("focus focused");

    assert!(state.handle_unregister(&mut reg, &other).is_none());
    assert_eq!(
        state.focused_in(&WindowLabel::from_string("main")),
        Some(&focused),
    );
}

/// `FocusChangedEvent.next_segment` is `Some(scope.segment.clone())`
/// whenever `next_fq` is `Some`.
#[test]
fn next_segment_matches_scope_segment_when_next_fq_is_some() {
    let mut registry = registry_with_scope("main", "/L", "/L/scope-1", "task:01XYZ");
    let mut state = SpatialState::new();
    let fq = FullyQualifiedMoniker::from_string("/L/scope-1");

    let event = state.focus(&mut registry, fq).expect("focus emits");
    assert_eq!(
        event.next_segment,
        Some(SegmentMoniker::from_string("task:01XYZ"))
    );
}

/// Focusing the already-focused FQM is a no-op — the adapter would
/// otherwise emit a redundant `focus-changed` event that React would
/// have to filter.
#[test]
fn focus_no_op_when_already_focused_in_that_window() {
    let mut registry = registry_with_scope("main", "/L", "/L/k", "task:01");
    let mut state = SpatialState::new();
    let fq = FullyQualifiedMoniker::from_string("/L/k");

    assert!(state.focus(&mut registry, fq.clone()).is_some());
    let second: Option<FocusChangedEvent> = state.focus(&mut registry, fq);
    assert!(second.is_none());
}

/// Re-focusing through the same window populates `prev_fq` correctly so
/// the React claim registry can dispatch `false` to the old FQM and
/// `true` to the new one in a single payload.
#[test]
fn focus_transfer_within_window_carries_prev_fq() {
    let mut reg = registry_with_scope("main", "/L", "/L/first", "task:1");
    add_scope(&mut reg, "/L", "/L/second", "task:2");

    let first = FullyQualifiedMoniker::from_string("/L/first");
    let second = FullyQualifiedMoniker::from_string("/L/second");

    let mut state = SpatialState::new();
    state.focus(&mut reg, first.clone()).expect("focus first");
    let event = state.focus(&mut reg, second.clone()).expect("focus second");

    assert_eq!(event.prev_fq, Some(first));
    assert_eq!(event.next_fq, Some(second));
}

/// `focus(fq)` resolves the FQM to a registered scope and emits the
/// expected event. Under the path-monikers model this is the only
/// focus-by-identifier API; the React side composes the FQM via
/// `FullyQualifiedMonikerContext` and dispatches it directly.
#[test]
fn focus_advances_focus_when_fq_resolves() {
    let mut registry = registry_with_scope("main", "/L", "/L/k1", "task:01ABC");
    let mut state = SpatialState::new();
    let fq = FullyQualifiedMoniker::from_string("/L/k1");

    let event = state
        .focus(&mut registry, fq.clone())
        .expect("focus emits when the FQM resolves");
    assert_eq!(event.window_label, WindowLabel::from_string("main"));
    assert_eq!(event.prev_fq, None);
    assert_eq!(event.next_fq, Some(fq.clone()));
    assert_eq!(
        event.next_segment,
        Some(SegmentMoniker::from_string("task:01ABC"))
    );

    assert_eq!(
        state.focused_in(&WindowLabel::from_string("main")),
        Some(&fq),
    );
}

/// `focus` returns `None` when the FQM is not registered. Adapters
/// surface this as `Err(_)` to the React caller so its `setFocus`
/// dispatch can `console.error` for dev visibility — kernel-side, the
/// adapter logs `tracing::error!`.
#[test]
fn focus_unknown_fq_returns_none_and_does_not_change_focus() {
    let mut registry = registry_with_scope("main", "/L", "/L/k1", "task:known");
    let mut state = SpatialState::new();

    state
        .focus(&mut registry, FullyQualifiedMoniker::from_string("/L/k1"))
        .expect("seed focus succeeds");

    let unknown = FullyQualifiedMoniker::from_string("/L/does-not-exist");
    let event = state.focus(&mut registry, unknown);
    assert!(event.is_none());

    assert_eq!(
        state.focused_in(&WindowLabel::from_string("main")),
        Some(&FullyQualifiedMoniker::from_string("/L/k1")),
    );
}

/// `clear_focus` produces a `Some(prev) → None` event so the React-side
/// `focus-changed` projection can flip the entity-focus store back to
/// `null`. This is the kernel-side API the React `setFocus(null)`
/// dispatches through.
#[test]
fn clear_focus_emits_some_to_none_event() {
    let mut registry = registry_with_scope("main", "/L", "/L/k1", "task:01");
    let mut state = SpatialState::new();
    let window = WindowLabel::from_string("main");

    state
        .focus(&mut registry, FullyQualifiedMoniker::from_string("/L/k1"))
        .expect("seed focus succeeds");
    assert_eq!(
        state.focused_in(&window),
        Some(&FullyQualifiedMoniker::from_string("/L/k1"))
    );

    let event = state
        .clear_focus(&window)
        .expect("clear_focus emits when there was prior focus");
    assert_eq!(event.window_label, window);
    assert_eq!(
        event.prev_fq,
        Some(FullyQualifiedMoniker::from_string("/L/k1"))
    );
    assert_eq!(event.next_fq, None);
    assert_eq!(event.next_segment, None);

    assert!(state.focused_in(&window).is_none());
}

/// `clear_focus` on a window with no prior focus is a no-op: returns
/// `None` so adapters do not emit a redundant `focus-changed` event.
#[test]
fn clear_focus_no_prior_focus_returns_none() {
    let mut state = SpatialState::new();
    let window = WindowLabel::from_string("main");

    let event = state.clear_focus(&window);
    assert!(event.is_none());
}
