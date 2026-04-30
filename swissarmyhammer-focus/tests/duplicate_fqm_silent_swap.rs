//! Pin the contract that same-shape duplicate FQM registrations are
//! silent and structural-mismatch duplicates still surface as errors.
//!
//! Background: production logs were flooded with
//! `duplicate FQM registration replaces prior scope` warnings on every
//! board interaction. The cause was the legitimate
//! placeholder→real-mount swap path: `usePlaceholderRegistration` in
//! `kanban-app/ui/src/components/column-view.tsx` registers a
//! placeholder scope per off-screen task; when a task scrolls into view
//! (or the virtualizer first measures and mounts the visible window)
//! its `<EntityCard>` `<FocusScope>` registers at the same FQM with an
//! identical structural shape — only the rect differs (placeholder
//! estimate vs. real `getBoundingClientRect()`). The placeholder hook
//! unregisters its entry on the next render commit, but in between the
//! kernel sees a same-FQM re-registration that is part of the
//! intentional swap. React StrictMode dev-mode double effects and
//! ResizeObserver-driven rect refreshes tread the same path.
//!
//! Contract pinned by these tests:
//!
//! 1. **Same-shape re-register is silent.** When the existing entry's
//!    `(segment, layer_fq, parent_zone, overrides)` tuple and kind
//!    discriminator match the new entry's, the registry replaces in
//!    place without emitting `tracing::error!`. Rect (and zones'
//!    `last_focused`) may differ — those are mutable runtime state.
//!
//! 2. **Structural-mismatch still warns.** When any of segment,
//!    layer_fq, parent_zone, overrides, or kind discriminator differs
//!    between the existing and new entry, the kernel emits
//!    `tracing::error!` with the structural-mismatch flags so the bug
//!    is visible in logs. The registry still replaces (consistency
//!    over silence) but the log surface stays available for genuine
//!    programmer mistakes.
//!
//! See the docstring on
//! [`swissarmyhammer_focus::SpatialRegistry::register_scope`] for the
//! full rationale.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use swissarmyhammer_focus::{
    Direction, FocusScope, FocusZone, FullyQualifiedMoniker, Pixels, Rect, SegmentMoniker,
    SpatialRegistry,
};
use tracing::{
    field::{Field, Visit},
    span::Attributes,
    Event, Id, Level, Subscriber,
};
use tracing_subscriber::{layer::Context, prelude::*, registry::LookupSpan, Layer};

// ---------------------------------------------------------------------------
// Tracing capture — record ERROR events emitted while a closure runs.
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
struct CapturedEvent {
    fields: HashMap<String, String>,
}

struct FieldVisitor<'a>(&'a mut HashMap<String, String>);

impl<'a> Visit for FieldVisitor<'a> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0
            .insert(field.name().to_string(), format!("{value:?}"));
    }
}

struct CapturingLayer {
    events: Arc<Mutex<Vec<CapturedEvent>>>,
}

impl<S> Layer<S> for CapturingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if *event.metadata().level() != Level::ERROR {
            return;
        }
        let mut captured = CapturedEvent::default();
        let mut visitor = FieldVisitor(&mut captured.fields);
        event.record(&mut visitor);
        self.events.lock().unwrap().push(captured);
    }

    fn on_new_span(&self, _attrs: &Attributes<'_>, _id: &Id, _ctx: Context<'_, S>) {}
}

fn capture_errors<F, R>(f: F) -> (R, Vec<CapturedEvent>)
where
    F: FnOnce() -> R,
{
    let events = Arc::new(Mutex::new(Vec::new()));
    let layer = CapturingLayer {
        events: events.clone(),
    };
    let subscriber = tracing_subscriber::registry().with(layer);
    let result = tracing::subscriber::with_default(subscriber, f);
    let captured = events.lock().unwrap().clone();
    (result, captured)
}

// ---------------------------------------------------------------------------
// Builders.
// ---------------------------------------------------------------------------

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        x: Pixels::new(x),
        y: Pixels::new(y),
        width: Pixels::new(w),
        height: Pixels::new(h),
    }
}

fn fq(path: &str) -> FullyQualifiedMoniker {
    FullyQualifiedMoniker::from_string(path)
}

fn make_scope(
    path: &str,
    segment: &str,
    layer: &str,
    parent_zone: Option<&str>,
    r: Rect,
) -> FocusScope {
    FocusScope {
        fq: fq(path),
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: fq(layer),
        parent_zone: parent_zone.map(fq),
        overrides: HashMap::new(),
    }
}

fn make_zone(
    path: &str,
    segment: &str,
    layer: &str,
    parent_zone: Option<&str>,
    r: Rect,
) -> FocusZone {
    FocusZone {
        fq: fq(path),
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: fq(layer),
        parent_zone: parent_zone.map(fq),
        last_focused: None,
        overrides: HashMap::new(),
    }
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

/// The placeholder→real-mount swap path: the same FQM is registered
/// twice, with identical `(segment, layer_fq, parent_zone, overrides)`
/// tuple and the same kind, only `rect` differs. The kernel must
/// replace the prior entry without emitting an error event — this
/// pattern fires once per off-screen card every render and would
/// otherwise dominate the log.
#[test]
fn same_shape_scope_re_register_emits_no_error() {
    let path = "/window/board/column:done/task:T1";
    let layer = "/window";
    let parent_zone = Some("/window/board/column:done");

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        reg.register_scope(make_scope(
            path,
            "task:T1",
            layer,
            parent_zone,
            rect(0.0, 0.0, 320.0, 80.0),
        ));
        // Real-mount swap: same shape, different rect.
        reg.register_scope(make_scope(
            path,
            "task:T1",
            layer,
            parent_zone,
            rect(12.0, 240.0, 320.0, 96.0),
        ));
    });

    assert!(
        events.is_empty(),
        "same-shape scope re-register must not emit error events; got: {events:?}"
    );
}

/// Same contract for zones — the placeholder→real-mount swap also
/// applies when the column virtualizer's batch path hits a zone (e.g.
/// nested-card sub-trees on rare layouts).
#[test]
fn same_shape_zone_re_register_emits_no_error() {
    let path = "/window/board/column:done";
    let layer = "/window";
    let parent_zone = Some("/window/board");

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        reg.register_zone(make_zone(
            path,
            "column:done",
            layer,
            parent_zone,
            rect(0.0, 0.0, 320.0, 800.0),
        ));
        reg.register_zone(make_zone(
            path,
            "column:done",
            layer,
            parent_zone,
            rect(0.0, 0.0, 320.0, 820.0),
        ));
    });

    assert!(
        events.is_empty(),
        "same-shape zone re-register must not emit error events; got: {events:?}"
    );
}

/// A genuine duplicate-path bug: two `<FocusScope>` mounts whose
/// composed paths collide but with different `parent_zone` (i.e. the
/// React contexts disagreed about the enclosing zone). This is exactly
/// the kind of programmer mistake the warning was added to catch — it
/// MUST still trip `tracing::error!` so the bug stays visible.
#[test]
fn structural_mismatch_parent_zone_emits_error() {
    let path = "/window/board/some/path/leaf";
    let layer = "/window";

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        reg.register_scope(make_scope(
            path,
            "leaf",
            layer,
            Some("/window/board/zone:a"),
            rect(0.0, 0.0, 10.0, 10.0),
        ));
        // Same FQM, different parent_zone — structural mismatch.
        reg.register_scope(make_scope(
            path,
            "leaf",
            layer,
            Some("/window/board/zone:b"),
            rect(0.0, 0.0, 10.0, 10.0),
        ));
    });

    assert_eq!(
        events.len(),
        1,
        "parent_zone mismatch must emit exactly one error event; got: {events:?}"
    );
    let ev = &events[0];
    assert_eq!(
        ev.fields.get("op").map(String::as_str),
        Some("register_scope")
    );
    assert_eq!(
        ev.fields.get("parent_zone_differs").map(String::as_str),
        Some("true"),
        "the error must surface the parent_zone_differs flag"
    );
}

/// Different segment at the same FQM — composeFq always uses the
/// segment as the trailing component, so this case is hard to produce
/// in real React code (it would mean the segment string passed to
/// `<FocusScope>` differs from the segment in the composed FQM). The
/// kernel still treats it as a structural mismatch.
#[test]
fn structural_mismatch_segment_emits_error() {
    let path = "/window/leaf";
    let layer = "/window";

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        reg.register_scope(make_scope(
            path,
            "leaf-a",
            layer,
            None,
            rect(0.0, 0.0, 10.0, 10.0),
        ));
        reg.register_scope(make_scope(
            path,
            "leaf-b",
            layer,
            None,
            rect(0.0, 0.0, 10.0, 10.0),
        ));
    });

    assert_eq!(events.len(), 1, "segment mismatch must emit one error");
    assert_eq!(
        events[0].fields.get("segment_differs").map(String::as_str),
        Some("true"),
        "the error must surface the segment_differs flag"
    );
}

/// Different layer_fq at the same FQM — this would mean the consumer
/// somehow registered the same path under two different
/// `useEnclosingLayerFq()` contexts. Structurally inconsistent; must
/// trip the error.
#[test]
fn structural_mismatch_layer_fq_emits_error() {
    let path = "/window/leaf";

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        reg.register_scope(make_scope(
            path,
            "leaf",
            "/window",
            None,
            rect(0.0, 0.0, 10.0, 10.0),
        ));
        reg.register_scope(make_scope(
            path,
            "leaf",
            "/window/inspector",
            None,
            rect(0.0, 0.0, 10.0, 10.0),
        ));
    });

    assert_eq!(events.len(), 1, "layer_fq mismatch must emit one error");
    assert_eq!(
        events[0].fields.get("layer_differs").map(String::as_str),
        Some("true"),
        "the error must surface the layer_differs flag"
    );
}

/// Kind flip: an FQM previously registered as a leaf scope is
/// re-registered as a zone (or vice versa). This is a real bug — the
/// React adapter on the consumer side must not flip a primitive's
/// kind. The error log surfaces the mismatch, and the `apply_batch`
/// path also enforces this via [`BatchRegisterError::KindMismatch`]
/// for atomicity. The single-entry path here uses the error log only;
/// the registry replaces in place because there is no error return on
/// these methods.
#[test]
fn structural_mismatch_kind_flip_emits_error() {
    let path = "/window/x";
    let layer = "/window";

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        reg.register_scope(make_scope(
            path,
            "x",
            layer,
            None,
            rect(0.0, 0.0, 10.0, 10.0),
        ));
        // Re-register as a zone — kind flip.
        reg.register_zone(make_zone(
            path,
            "x",
            layer,
            None,
            rect(0.0, 0.0, 10.0, 10.0),
        ));
    });

    assert_eq!(events.len(), 1, "kind flip must emit one error");
    assert_eq!(
        events[0].fields.get("op").map(String::as_str),
        Some("register_zone")
    );
    assert_eq!(
        events[0].fields.get("kind_flipped").map(String::as_str),
        Some("true"),
        "the error must surface the kind_flipped flag"
    );
}

/// Different `overrides` map at the same FQM — same shape on every
/// other field. Override directives are the closest thing to a
/// "behavior" knob on the registration, so disagreeing override sets
/// indicate the consumer's two render paths configured the same FQM
/// inconsistently. Trip the error.
#[test]
fn structural_mismatch_overrides_emits_error() {
    let path = "/window/x";
    let layer = "/window";
    let target = fq("/window/y");

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        let mut first = make_scope(path, "x", layer, None, rect(0.0, 0.0, 10.0, 10.0));
        first
            .overrides
            .insert(Direction::Right, Some(target.clone()));
        reg.register_scope(first);

        // Empty overrides map this time — different from the prior
        // entry's `{Right: Some(/window/y)}`.
        let second = make_scope(path, "x", layer, None, rect(0.0, 0.0, 10.0, 10.0));
        reg.register_scope(second);
    });

    assert_eq!(events.len(), 1, "overrides mismatch must emit one error");
    assert_eq!(
        events[0].fields.get("overrides_differ").map(String::as_str),
        Some("true"),
        "the error must surface the overrides_differ flag"
    );
}

/// Burst test mirroring the production scenario: 50 placeholder
/// scopes registered (off-screen tasks) then 50 same-shape real
/// `<FocusScope>` registrations as the cards mount with their real
/// rects. ZERO error events. This is the contract that pins the
/// 30-second-board-interaction acceptance criterion: a burst of
/// same-shape re-registrations must not flood the log.
#[test]
fn fifty_placeholder_to_real_swaps_are_silent() {
    let layer = "/window";
    let parent_zone = Some("/window/board/column:done");
    const N: usize = 50;

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();

        // Phase 1 — placeholders (estimated rects).
        for i in 0..N {
            let path = format!("/window/board/column:done/task:T{i}");
            let segment = format!("task:T{i}");
            reg.register_scope(make_scope(
                &path,
                &segment,
                layer,
                parent_zone,
                rect(0.0, (i as f64) * 80.0, 320.0, 80.0),
            ));
        }

        // Phase 2 — real-mount swaps (real rects).
        for i in 0..N {
            let path = format!("/window/board/column:done/task:T{i}");
            let segment = format!("task:T{i}");
            reg.register_scope(make_scope(
                &path,
                &segment,
                layer,
                parent_zone,
                rect(12.0, (i as f64) * 78.5 + 4.0, 308.0, 76.0),
            ));
        }
    });

    assert!(
        events.is_empty(),
        "burst of same-shape placeholder→real-mount swaps must be silent; \
         got {} error event(s): {events:?}",
        events.len()
    );
}
