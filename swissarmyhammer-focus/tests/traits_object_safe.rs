//! Object-safety smoke tests for the pluggable extension traits.
//!
//! Both [`NavStrategy`] and [`FocusEventSink`] must be usable as
//! `Box<dyn …>` so adapters can store them behind a trait object
//! without monomorphisation. A regression that adds a generic method
//! or a `Self`-by-value receiver would break this property silently
//! at the call site; pinning it as a test surfaces the breakage at
//! crate compile time.

use std::sync::Mutex;

use swissarmyhammer_focus::{
    BeamNavStrategy, Direction, FocusChangedEvent, FocusEventSink, Moniker, NavStrategy, NoopSink,
    RecordingSink, SpatialKey, SpatialRegistry, WindowLabel,
};

/// `NavStrategy` is object-safe: `Box<dyn NavStrategy>` compiles and
/// dispatches its only method.
#[test]
fn nav_strategy_is_object_safe() {
    let strategy: Box<dyn NavStrategy> = Box::new(BeamNavStrategy::new());
    let registry = SpatialRegistry::new();
    let result = strategy.next(
        &registry,
        &SpatialKey::from_string("ghost"),
        Direction::Right,
    );
    assert!(result.is_none());
}

/// `FocusEventSink` is object-safe in both ready-made impls.
/// `Box<dyn FocusEventSink>` for `NoopSink` and `RecordingSink` both
/// compile and accept events.
#[test]
fn focus_event_sink_is_object_safe() {
    let noop: Box<dyn FocusEventSink> = Box::new(NoopSink);
    let recorder: Box<dyn FocusEventSink> = Box::new(RecordingSink::new());

    let event = FocusChangedEvent {
        window_label: WindowLabel::from_string("main"),
        prev_key: None,
        next_key: Some(SpatialKey::from_string("k")),
        next_moniker: Some(Moniker::from_string("ui:k")),
    };
    noop.emit(&event);
    recorder.emit(&event);
}

/// `RecordingSink` collects events in arrival order — required so a
/// scenario can assert "two events fired, in this order" without
/// reaching for ad-hoc capture machinery.
#[test]
fn recording_sink_collects_two_events_in_order() {
    let sink = RecordingSink::new();

    let first = FocusChangedEvent {
        window_label: WindowLabel::from_string("main"),
        prev_key: None,
        next_key: Some(SpatialKey::from_string("first")),
        next_moniker: Some(Moniker::from_string("ui:first")),
    };
    let second = FocusChangedEvent {
        window_label: WindowLabel::from_string("main"),
        prev_key: Some(SpatialKey::from_string("first")),
        next_key: Some(SpatialKey::from_string("second")),
        next_moniker: Some(Moniker::from_string("ui:second")),
    };

    sink.emit(&first);
    sink.emit(&second);

    let events = sink.events.lock().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0], first);
    assert_eq!(events[1], second);
}

/// `RecordingSink::events` is a `Mutex<Vec<FocusChangedEvent>>` so the
/// `Send + Sync` bounds on the trait are honored — wrapping the sink
/// in an `Arc` and shipping it across threads must compile. The body
/// re-locks the same mutex from the spawned thread to make sure the
/// type bounds are real, not just declared.
#[test]
fn recording_sink_is_send_sync_via_arc() {
    let sink = std::sync::Arc::new(RecordingSink::new());
    let sink_for_thread = std::sync::Arc::clone(&sink);

    let handle = std::thread::spawn(move || {
        sink_for_thread.emit(&FocusChangedEvent {
            window_label: WindowLabel::from_string("main"),
            prev_key: None,
            next_key: Some(SpatialKey::from_string("k")),
            next_moniker: Some(Moniker::from_string("ui:k")),
        });
    });
    handle.join().unwrap();

    assert_eq!(sink.events.lock().unwrap().len(), 1);
}

/// Sanity: `Mutex<Vec<FocusChangedEvent>>` is the storage shape we
/// document in the docs — a typo here would mask a regression where
/// the lock primitive changes silently.
#[test]
fn recording_sink_storage_is_mutex_protected() {
    let sink = RecordingSink::new();
    // The field's static type is `Mutex<Vec<FocusChangedEvent>>`.
    // Asserting the borrow type pins the storage shape against silent
    // changes (e.g. swapping in `RwLock` or `parking_lot::Mutex`) that
    // would change downstream lock-handling code.
    let _: &Mutex<Vec<FocusChangedEvent>> = &sink.events;
}
