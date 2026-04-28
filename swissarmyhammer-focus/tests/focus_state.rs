//! Integration tests for `swissarmyhammer_focus::SpatialState`.
//!
//! Headless pattern matching `tests/resolve_focused_column.rs` — pure Rust,
//! no Tauri runtime, no jsdom. Every focus mutation runs through the public
//! `SpatialState` surface and is asserted by inspecting the returned
//! `FocusChangedEvent` plus subsequent `focused_in` reads.
//!
//! These tests cover the per-window focus invariants that the spatial-nav
//! card spec calls out by name:
//!
//! - `focus` updates per-window state and returns a `FocusChangedEvent`
//!   whose `window_label` matches the scope's window (derived from the
//!   registry).
//! - Focus in window A does not perturb `focus_by_window` for window B.
//! - Unregistering the focused scope clears that window's slot only.
//! - `FocusChangedEvent.next_moniker` is `Some(scope.moniker().clone())`
//!   whenever `next_key` is `Some`.
//!
//! `SpatialState` no longer maintains a per-key entry table — the
//! registry is the single source of truth for window/moniker metadata.
//! These tests construct a [`SpatialRegistry`] and pass it to every
//! mutating call.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    FocusChangedEvent, FocusLayer, FocusScope, LayerKey, LayerName, Moniker, Pixels, Rect,
    SpatialKey, SpatialRegistry, SpatialState, WindowLabel,
};

/// Build a single-window registry with a leaf scope bound to
/// `(window, moniker)`.
fn registry_with_scope(window: &str, layer: &str, key: &str, moniker: &str) -> SpatialRegistry {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(FocusLayer {
        key: LayerKey::from_string(layer),
        name: LayerName::from_string("window"),
        parent: None,
        window_label: WindowLabel::from_string(window),
        last_focused: None,
    });
    reg.register_scope(FocusScope {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect: Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        },
        layer_key: LayerKey::from_string(layer),
        parent_zone: None,
        overrides: HashMap::new(),
    });
    reg
}

/// Add a second leaf scope to an existing registry under the same layer.
/// Used to set up "focus transfer within window" scenarios.
fn add_scope(reg: &mut SpatialRegistry, layer: &str, key: &str, moniker: &str) {
    reg.register_scope(FocusScope {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect: Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        },
        layer_key: LayerKey::from_string(layer),
        parent_zone: None,
        overrides: HashMap::new(),
    });
}

/// `focus` updates per-window state and returns a `FocusChangedEvent`
/// whose `window_label` matches the focused scope's window.
#[test]
fn focus_updates_per_window_state_and_emits_with_window_label() {
    let registry = registry_with_scope("main", "L", "scope-1", "task:01ABC");
    let mut state = SpatialState::new();
    let key = SpatialKey::from_string("scope-1");

    let event = state
        .focus(&registry, key.clone())
        .expect("focus must emit on first move");

    assert_eq!(
        event.window_label,
        WindowLabel::from_string("main"),
        "window_label on the event must match the scope's window"
    );
    assert_eq!(event.prev_key, None, "no prior focus → prev_key None");
    assert_eq!(event.next_key, Some(key.clone()));
    assert_eq!(
        state.focused_in(&WindowLabel::from_string("main")),
        Some(&key),
        "main window's focus slot must point at the freshly focused key"
    );
}

/// Focus in window A must not affect `focus_by_window[B]`. Two windows
/// hold independent slots — focus moves in one window cannot displace
/// focus in another.
#[test]
fn focus_in_a_does_not_affect_focus_in_b() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(FocusLayer {
        key: LayerKey::from_string("La"),
        name: LayerName::from_string("window"),
        parent: None,
        window_label: WindowLabel::from_string("window-a"),
        last_focused: None,
    });
    reg.push_layer(FocusLayer {
        key: LayerKey::from_string("Lb"),
        name: LayerName::from_string("window"),
        parent: None,
        window_label: WindowLabel::from_string("window-b"),
        last_focused: None,
    });
    add_scope(&mut reg, "La", "a-1", "task:A");
    add_scope(&mut reg, "Lb", "b-1", "task:B");
    add_scope(&mut reg, "La", "a-2", "task:A2");

    let a_key = SpatialKey::from_string("a-1");
    let b_key = SpatialKey::from_string("b-1");
    let a2_key = SpatialKey::from_string("a-2");

    let mut state = SpatialState::new();
    state.focus(&reg, a_key.clone()).expect("focus a");
    state.focus(&reg, b_key.clone()).expect("focus b");

    // Re-focusing within window A: window B's slot stays put.
    let event = state.focus(&reg, a2_key.clone()).expect("focus a2");

    assert_eq!(event.window_label, WindowLabel::from_string("window-a"));
    assert_eq!(event.prev_key, Some(a_key));
    assert_eq!(event.next_key, Some(a2_key.clone()));
    assert_eq!(
        state.focused_in(&WindowLabel::from_string("window-a")),
        Some(&a2_key),
        "window A's focus moved to a2"
    );
    assert_eq!(
        state.focused_in(&WindowLabel::from_string("window-b")),
        Some(&b_key),
        "window B's focus must not have changed"
    );
}

/// Unregistering the focused scope clears that window's focus slot, and
/// emits a `Some(prev) → None` event so the React claim registry can
/// notify the losing scope. Other windows' slots stay intact.
#[test]
fn unregister_of_focused_key_clears_only_that_windows_focus() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(FocusLayer {
        key: LayerKey::from_string("La"),
        name: LayerName::from_string("window"),
        parent: None,
        window_label: WindowLabel::from_string("window-a"),
        last_focused: None,
    });
    reg.push_layer(FocusLayer {
        key: LayerKey::from_string("Lb"),
        name: LayerName::from_string("window"),
        parent: None,
        window_label: WindowLabel::from_string("window-b"),
        last_focused: None,
    });
    add_scope(&mut reg, "La", "a-1", "task:A");
    add_scope(&mut reg, "Lb", "b-1", "task:B");

    let a_key = SpatialKey::from_string("a-1");
    let b_key = SpatialKey::from_string("b-1");

    let mut state = SpatialState::new();
    state.focus(&reg, a_key.clone()).expect("focus a");
    state.focus(&reg, b_key.clone()).expect("focus b");

    let event = state
        .handle_unregister(&reg, &a_key)
        .expect("unregistering the focused key emits a clear event");

    assert_eq!(event.window_label, WindowLabel::from_string("window-a"));
    assert_eq!(event.prev_key, Some(a_key));
    assert_eq!(event.next_key, None);
    assert_eq!(event.next_moniker, None);
    assert_eq!(
        state.focused_in(&WindowLabel::from_string("window-a")),
        None,
        "window A's focus slot must be cleared after unregister"
    );
    assert_eq!(
        state.focused_in(&WindowLabel::from_string("window-b")),
        Some(&b_key),
        "window B's focus slot must be untouched"
    );
}

/// Unregistering an unfocused scope is a silent registry update — no event
/// is emitted because no claim callback needs to fire.
#[test]
fn unregister_of_unfocused_key_emits_no_event() {
    let mut reg = registry_with_scope("main", "L", "focused", "task:F");
    add_scope(&mut reg, "L", "other", "task:O");

    let focused = SpatialKey::from_string("focused");
    let other = SpatialKey::from_string("other");

    let mut state = SpatialState::new();
    state.focus(&reg, focused.clone()).expect("focus focused");

    assert!(
        state.handle_unregister(&reg, &other).is_none(),
        "unregistering an unfocused scope must not emit"
    );
    assert_eq!(
        state.focused_in(&WindowLabel::from_string("main")),
        Some(&focused),
        "focus slot must still point at the originally focused key"
    );
}

/// `FocusChangedEvent.next_moniker` is `Some(scope.moniker().clone())`
/// whenever `next_key` is `Some` — the frontend reads this field to drive
/// moniker-keyed effects without an extra IPC round-trip.
#[test]
fn next_moniker_matches_scope_moniker_when_next_key_is_some() {
    let registry = registry_with_scope("main", "L", "scope-1", "task:01XYZ");
    let mut state = SpatialState::new();
    let key = SpatialKey::from_string("scope-1");

    let event = state.focus(&registry, key).expect("focus emits");
    assert_eq!(event.next_moniker, Some(Moniker::from_string("task:01XYZ")));
}

/// Focusing the already-focused key is a no-op — the adapter would
/// otherwise emit a redundant `focus-changed` event that React would have
/// to filter. Covered here so the contract is asserted at the focus-crate
/// layer rather than relying on adapter-side coalescing.
#[test]
fn focus_no_op_when_already_focused_in_that_window() {
    let registry = registry_with_scope("main", "L", "k", "task:01");
    let mut state = SpatialState::new();
    let key = SpatialKey::from_string("k");

    assert!(state.focus(&registry, key.clone()).is_some());
    let second: Option<FocusChangedEvent> = state.focus(&registry, key);
    assert!(second.is_none());
}

/// Re-focusing through the same window populates `prev_key` correctly so
/// the React claim registry can dispatch `false` to the old key and `true`
/// to the new one in a single payload.
#[test]
fn focus_transfer_within_window_carries_prev_key() {
    let mut reg = registry_with_scope("main", "L", "first", "task:1");
    add_scope(&mut reg, "L", "second", "task:2");

    let first = SpatialKey::from_string("first");
    let second = SpatialKey::from_string("second");

    let mut state = SpatialState::new();
    state.focus(&reg, first.clone()).expect("focus first");
    let event = state.focus(&reg, second.clone()).expect("focus second");

    assert_eq!(event.prev_key, Some(first));
    assert_eq!(event.next_key, Some(second));
}
