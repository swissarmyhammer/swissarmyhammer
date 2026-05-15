//! Pluggable focus-event sink.
//!
//! [`FocusEventSink`] is an optional sugar layer over the existing
//! [`Option<FocusChangedEvent>`] return values from the [`SpatialState`]
//! mutators. Adapters that prefer push-based event delivery (e.g. the
//! Tauri app's `app.emit("focus-changed", payload)` path) can route
//! every produced event through a sink rather than threading
//! `Option<FocusChangedEvent>` through call sites.
//!
//! Two ready-made impls ship with the kernel:
//!
//! - [`NoopSink`] — drops every event. Useful for tests / adapters that
//!   only care about return-value style.
//! - [`RecordingSink`] — collects every event in a `Mutex<Vec<…>>` for
//!   later inspection. The canonical choice for headless tests asserting
//!   the sequence of focus events triggered by a scenario.
//!
//! [`SpatialState`]: crate::state::SpatialState
//! [`FocusChangedEvent`]: crate::state::FocusChangedEvent

use std::sync::Mutex;

use crate::state::FocusChangedEvent;

/// Pluggable observer for focus-changed events.
///
/// Implementations are `Send + Sync` so adapters can store a sink behind
/// an `Arc<dyn FocusEventSink>` shared across async tasks. The sink
/// receives an event reference rather than ownership so callers can
/// also keep the event around to feed the [`Option<FocusChangedEvent>`]
/// return-value pipeline; `Clone` on [`FocusChangedEvent`] makes both
/// shapes cheap.
pub trait FocusEventSink: Send + Sync {
    /// Deliver one focus-changed event.
    ///
    /// Implementations must not panic — sinks fire on every mutation
    /// so a panicking impl would take down the focus pipeline. Errors
    /// (e.g. IPC transport failures) should be logged and swallowed.
    fn emit(&self, event: &FocusChangedEvent);
}

/// Drops every event silently.
///
/// The default `Send + Sync` implementation for tests and adapters
/// that only consume the [`Option<FocusChangedEvent>`] return-value
/// pipeline.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopSink;

impl FocusEventSink for NoopSink {
    /// Discard the event. Provided so [`NoopSink`] satisfies the trait
    /// without storage; useful as a placeholder when no observer is
    /// needed.
    fn emit(&self, _event: &FocusChangedEvent) {}
}

/// Captures every event in a [`Mutex`]-protected [`Vec`] for later
/// inspection.
///
/// The canonical sink for headless tests: spawn one, run the scenario,
/// then `lock().unwrap()` and assert on the captured event sequence.
/// `Mutex` rather than `RwLock` because writes outnumber reads on the
/// hot path and the lock is held for a single push per event.
#[derive(Debug, Default)]
pub struct RecordingSink {
    /// Collected events in arrival order. Public so test code can
    /// `lock().unwrap()` without wrapping in another helper.
    pub events: Mutex<Vec<FocusChangedEvent>>,
}

impl RecordingSink {
    /// Construct an empty recording sink. Equivalent to
    /// `RecordingSink::default()` — provided for symmetry with other
    /// `new`-flavored constructors in the kernel.
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot the captured events, draining the internal buffer.
    ///
    /// Returns the events in arrival order and resets the sink so the
    /// next scenario starts fresh. Useful between phases of a multi-
    /// step test without spinning up a new sink.
    pub fn drain(&self) -> Vec<FocusChangedEvent> {
        let mut guard = self.events.lock().unwrap();
        std::mem::take(&mut *guard)
    }
}

impl FocusEventSink for RecordingSink {
    /// Append the event to the captured list. Clones the event so
    /// callers can also feed the same value into the return-value
    /// pipeline.
    fn emit(&self, event: &FocusChangedEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FullyQualifiedMoniker, SegmentMoniker, WindowLabel};

    fn event(name: &str) -> FocusChangedEvent {
        FocusChangedEvent {
            window_label: WindowLabel::from_string("main"),
            prev_fq: None,
            next_fq: Some(FullyQualifiedMoniker::from_string(format!("/L/{name}"))),
            next_segment: Some(SegmentMoniker::from_string(name)),
        }
    }

    /// `NoopSink` accepts events without panicking. There is nothing to
    /// observe, so the test is a smoke check that the trait object is
    /// usable.
    #[test]
    fn noop_sink_drops_silently() {
        let sink: Box<dyn FocusEventSink> = Box::new(NoopSink);
        sink.emit(&event("k"));
    }

    /// `RecordingSink` collects events in arrival order — required so
    /// downstream tests can assert on the sequence of focus moves
    /// triggered by a scenario, not just the final state.
    #[test]
    fn recording_sink_collects_events_in_order() {
        let sink = RecordingSink::new();
        sink.emit(&event("first"));
        sink.emit(&event("second"));

        let captured = sink.events.lock().unwrap();
        assert_eq!(captured.len(), 2);
        assert_eq!(
            captured[0].next_fq,
            Some(FullyQualifiedMoniker::from_string("/L/first"))
        );
        assert_eq!(
            captured[1].next_fq,
            Some(FullyQualifiedMoniker::from_string("/L/second"))
        );
    }

    /// `drain` empties the buffer and returns the captured events. The
    /// next scenario starts from a clean state so a multi-phase test
    /// can assert phase-by-phase without spinning up a new sink.
    #[test]
    fn recording_sink_drain_resets_buffer() {
        let sink = RecordingSink::new();
        sink.emit(&event("a"));
        sink.emit(&event("b"));

        let drained = sink.drain();
        assert_eq!(drained.len(), 2);
        assert!(sink.events.lock().unwrap().is_empty());
    }
}
