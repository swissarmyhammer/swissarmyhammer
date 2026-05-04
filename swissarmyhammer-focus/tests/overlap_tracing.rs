//! Source-of-truth integration tests for the same-kind-overlap warning
//! contract on the [`SpatialRegistry`].
//!
//! When two `FocusScope`s register at the same rounded `(x, y)` in the
//! same layer — or two `FocusScope`s do — the registry emits one
//! `tracing::warn!` event flagging a likely needless-nesting candidate.
//! Cross-kind overlaps (zone-vs-scope) are NOT warned because a leaf as
//! the sole child of an unpadded zone is a normal layout. Cross-layer
//! overlaps are also not warned — different layers are different
//! surfaces.
//!
//! The contract pinned here:
//!
//! 1. Same-kind, same-(rounded x, y), same layer → exactly one `WARN`.
//! 2. Cross-kind at the same (x, y) → zero events.
//! 3. Different layers at the same (x, y) → zero events.
//! 4. Different (rounded) (x, y) → zero events.
//! 5. `update_rect` that creates an overlap → one `WARN`.
//! 6. `update_rect` repeated with the same overlap pair → no re-warn.
//! 7. Overlap cleared and re-created → fresh warn.
//! 8. `unregister_scope` releases the per-key suppression slot.
//!
//! Each test sets up a registry, captures `WARN` events while invoking
//! the registration / update path, and asserts the expected event count
//! and structured fields.
//!
//! [`SpatialRegistry`]: swissarmyhammer_focus::SpatialRegistry

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use swissarmyhammer_focus::{
    FocusLayer, FocusScope, FullyQualifiedMoniker, LayerName, Pixels, Rect,
    SegmentMoniker, SpatialRegistry, WindowLabel,
};
use tracing::{
    field::{Field, Visit},
    span::Attributes,
    Event, Id, Level, Subscriber,
};
use tracing_subscriber::{layer::Context, prelude::*, registry::LookupSpan, Layer};

// ---------------------------------------------------------------------------
// Tracing capture — custom Layer that records WARN events emitted while
// a closure runs.
// ---------------------------------------------------------------------------

/// One captured `WARN` event with the structured fields the registry
/// emits on overlap detection.
#[derive(Debug, Default, Clone)]
struct CapturedEvent {
    /// Field values rendered to string. Keys are field names; values
    /// are the rendered fmt::Debug or fmt::Display output.
    fields: HashMap<String, String>,
}

impl CapturedEvent {
    /// Read the `op` field (caller op tag — `register_scope`,
    /// `register_zone`, or `update_rect`).
    fn op(&self) -> Option<&str> {
        self.fields.get("op").map(String::as_str)
    }

    /// Read a numeric field (rendered as a string by the visitor).
    fn field(&self, name: &str) -> Option<&str> {
        self.fields.get(name).map(String::as_str)
    }
}

/// Visitor that copies each field into a `HashMap<String, String>` on
/// the captured event. Strings are stored verbatim; everything else is
/// rendered through `fmt::Debug` so numerics like `x = 100` arrive as
/// `"100"` (not `"100i64"`), matching how `tracing` formats them in
/// production.
struct FieldVisitor<'a>(&'a mut HashMap<String, String>);

impl<'a> Visit for FieldVisitor<'a> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0
            .insert(field.name().to_string(), format!("{value:?}"));
    }
}

/// Tracing [`Layer`] that records WARN-level events into a shared
/// `Vec<CapturedEvent>`.
struct CapturingLayer {
    events: Arc<Mutex<Vec<CapturedEvent>>>,
}

impl<S> Layer<S> for CapturingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if *event.metadata().level() != Level::WARN {
            return;
        }
        let mut captured = CapturedEvent::default();
        let mut visitor = FieldVisitor(&mut captured.fields);
        event.record(&mut visitor);
        self.events.lock().unwrap().push(captured);
    }

    fn on_new_span(&self, _attrs: &Attributes<'_>, _id: &Id, _ctx: Context<'_, S>) {}
}

/// Run `f` with a tracing subscriber that captures `WARN` events,
/// returning the captured events in arrival order.
fn capture_warns<F, R>(f: F) -> (R, Vec<CapturedEvent>)
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

fn make_zone(fq_str: &str, layer: &str, r: Rect) -> FocusScope {
    FocusScope {
        fq: fq(fq_str),
        segment: segment(fq_str.rsplit('/').next().unwrap_or(fq_str)),
        rect: r,
        layer_fq: fq(layer),
        parent_zone: None,
        last_focused: None,
        overrides: HashMap::new(),
    }
}

fn make_scope(fq_str: &str, layer: &str, r: Rect) -> FocusScope {
    FocusScope {
        fq: fq(fq_str),
        segment: segment(fq_str.rsplit('/').next().unwrap_or(fq_str)),
        rect: r,
        layer_fq: fq(layer),
        parent_zone: None,
        overrides: HashMap::new(),
        last_focused: None,
    }
}

/// Filter captured events down to overlap warnings only — the registry
/// emits other `WARN`-level events (e.g. window-root-corruption)
/// elsewhere, so the test suite needs to focus on the ones this ticket
/// owns. We discriminate on the `new_fq` field: every overlap warning
/// carries that field, no other warning in the registry does.
fn overlap_events(events: &[CapturedEvent]) -> Vec<&CapturedEvent> {
    events
        .iter()
        .filter(|e| e.fields.contains_key("new_fq"))
        .collect()
}

// ---------------------------------------------------------------------------
// Same-layer same-kind overlap detection.
// ---------------------------------------------------------------------------

/// Two `FocusScope`s registered at the same rounded `(x, y)` in the
/// same layer emit exactly one `WARN` whose `op = "register_zone"` and
/// whose payload identifies both FQMs and the rounded coordinates.
#[test]
fn two_zones_same_xy_same_layer_warns_once() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", "main"));

    let a = make_zone("/L/zone-a", "/L", rect(100.0, 80.0, 50.0, 30.0));
    let b = make_zone("/L/zone-b", "/L", rect(100.0, 80.0, 60.0, 40.0));

    let (_, captured) = capture_warns(|| {
        reg.register_scope(a);
        reg.register_scope(b);
    });

    let events = overlap_events(&captured);
    assert_eq!(
        events.len(),
        1,
        "two zones at same (x, y) must emit exactly one overlap WARN, got {captured:?}"
    );
    let e = events[0];
    assert_eq!(e.op(), Some("register_scope"));
    assert_eq!(e.field("x"), Some("100"));
    assert_eq!(e.field("y"), Some("80"));
    let new_fq = e.field("new_fq").expect("new_fq present");
    let overlap_fq = e.field("overlap_fq").expect("overlap_fq present");
    assert_eq!(new_fq, "/L/zone-b");
    assert_eq!(overlap_fq, "/L/zone-a");
}

/// Two `FocusScope`s registered at the same rounded `(x, y)` in the
/// same layer emit exactly one `WARN` whose `op = "register_scope"`.
#[test]
fn two_scopes_same_xy_same_layer_warns_once() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", "main"));

    let a = make_scope("/L/scope-a", "/L", rect(100.0, 80.0, 50.0, 30.0));
    let b = make_scope("/L/scope-b", "/L", rect(100.0, 80.0, 60.0, 40.0));

    let (_, captured) = capture_warns(|| {
        reg.register_scope(a);
        reg.register_scope(b);
    });

    let events = overlap_events(&captured);
    assert_eq!(
        events.len(),
        1,
        "two scopes at same (x, y) must emit exactly one overlap WARN, got {captured:?}"
    );
    let e = events[0];
    assert_eq!(e.op(), Some("register_scope"));
    assert_eq!(e.field("x"), Some("100"));
    assert_eq!(e.field("y"), Some("80"));
    assert_eq!(e.field("new_fq"), Some("/L/scope-b"));
    assert_eq!(e.field("overlap_fq"), Some("/L/scope-a"));
}

// The previous tests `zone_and_scope_same_xy_does_not_warn` and
// `scope_and_zone_same_xy_does_not_warn` asserted that the kernel
// suppressed overlap warnings when the two registrations had different
// kind discriminators (one zone + one scope). With the FocusZone /
// FocusScope collapse there is no kind, so every overlap is a single
// uniform case — those tests' premise is gone. The single replacement
// test below pins the new behaviour: any same-(x, y) overlap in the
// same layer produces one warning, regardless of who registered first.
// Kept after the collapse — every scope is uniform now.

/// Two scopes at the same `(x, y)` in the same layer emit a warning
/// even when one of them later acts as a container. Under the unified
/// primitive, "kind" is no longer a registration property; the
/// needless-nesting heuristic fires uniformly.
#[test]
fn same_xy_overlap_warns_regardless_of_descendants() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", "main"));

    let a = make_scope("/L/a", "/L", rect(100.0, 80.0, 50.0, 30.0));
    let b = make_scope("/L/b", "/L", rect(100.0, 80.0, 50.0, 30.0));
    let b_child = make_scope("/L/b/inner", "/L", rect(100.0, 80.0, 10.0, 10.0));

    let (_, captured) = capture_warns(|| {
        reg.register_scope(a);
        reg.register_scope(b);
        // Registering a child under b makes it a container; the
        // overlap warning fired earlier still stands.
        let mut child = b_child;
        child.parent_zone = Some(swissarmyhammer_focus::FullyQualifiedMoniker::from_string(
            "/L/b",
        ));
        reg.register_scope(child);
    });

    let events = overlap_events(&captured);
    assert!(
        !events.is_empty(),
        "same (x, y) overlap must emit at least one warning, got {captured:?}"
    );
}

/// Two zones at different rounded `(x, y)` (off by ≥ 1 pixel) produce
/// zero overlap warnings.
#[test]
fn two_zones_different_xy_does_not_warn() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", "main"));

    let a = make_zone("/L/zone-a", "/L", rect(100.0, 80.0, 50.0, 30.0));
    let b = make_zone("/L/zone-b", "/L", rect(120.0, 80.0, 50.0, 30.0));

    let (_, captured) = capture_warns(|| {
        reg.register_scope(a);
        reg.register_scope(b);
    });

    assert_eq!(
        overlap_events(&captured).len(),
        0,
        "different (x, y) must not warn; got {captured:?}"
    );
}

/// Two zones whose subpixel coordinates round to *different* integer
/// pixels (e.g. `100.6` rounds to `101`, `100.0` rounds to `100`) do
/// not warn. The rounding is the cutoff — same-rounded-(x, y) is the
/// structural overlap; different-rounded-(x, y) is structurally
/// distinct.
#[test]
fn two_zones_subpixel_difference_does_not_warn() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", "main"));

    let a = make_zone("/L/zone-a", "/L", rect(100.0, 80.0, 50.0, 30.0));
    // 100.6 rounds to 101 (banker's rounding via f64::round() for
    // positive values matches "round half away from zero" — 100.6
    // unambiguously rounds up).
    let b = make_zone("/L/zone-b", "/L", rect(100.6, 80.0, 50.0, 30.0));

    let (_, captured) = capture_warns(|| {
        reg.register_scope(a);
        reg.register_scope(b);
    });

    assert_eq!(
        overlap_events(&captured).len(),
        0,
        "subpixel difference rounding to different ints must not warn; got {captured:?}"
    );
}

/// Two zones at the same `(x, y)` but in DIFFERENT layers are not an
/// overlap — different layers are different surfaces.
#[test]
fn two_zones_same_xy_different_layers_does_not_warn() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L1", "main"));
    reg.push_layer(make_layer("/L2", "main"));

    let a = make_zone("/L1/zone-a", "/L1", rect(100.0, 80.0, 50.0, 30.0));
    let b = make_zone("/L2/zone-b", "/L2", rect(100.0, 80.0, 50.0, 30.0));

    let (_, captured) = capture_warns(|| {
        reg.register_scope(a);
        reg.register_scope(b);
    });

    assert_eq!(
        overlap_events(&captured).len(),
        0,
        "different layers must not warn; got {captured:?}"
    );
}

// ---------------------------------------------------------------------------
// `update_rect` overlap detection and per-key suppression.
// ---------------------------------------------------------------------------

/// `update_rect` that moves an entry onto an existing same-kind entry
/// at `(x, y)` emits exactly one `WARN`.
#[test]
fn update_rect_creates_overlap_warns_once() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", "main"));

    let a = make_zone("/L/zone-a", "/L", rect(100.0, 80.0, 50.0, 30.0));
    let b = make_zone("/L/zone-b", "/L", rect(200.0, 80.0, 50.0, 30.0));

    reg.register_scope(a);
    reg.register_scope(b);

    let b_fq = fq("/L/zone-b");
    let (_, captured) = capture_warns(|| reg.update_rect(&b_fq, rect(100.0, 80.0, 50.0, 30.0)));

    let events = overlap_events(&captured);
    assert_eq!(
        events.len(),
        1,
        "update_rect creating an overlap must emit one WARN; got {captured:?}"
    );
    let e = events[0];
    assert_eq!(e.op(), Some("update_rect"));
    assert_eq!(e.field("new_fq"), Some("/L/zone-b"));
    assert_eq!(e.field("overlap_fq"), Some("/L/zone-a"));
}

/// `update_rect` called repeatedly with the same coordinates against
/// the same overlap partner does NOT re-emit. Per-key suppression
/// elides re-warnings while the overlap pair persists, so per-frame
/// scroll tracking does not flood the log.
#[test]
fn update_rect_repeated_with_same_overlap_does_not_re_warn() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", "main"));

    reg.register_scope(make_zone("/L/zone-a", "/L", rect(100.0, 80.0, 50.0, 30.0)));
    reg.register_scope(make_zone("/L/zone-b", "/L", rect(200.0, 80.0, 50.0, 30.0)));

    let b_fq = fq("/L/zone-b");

    let (_, captured) = capture_warns(|| {
        reg.update_rect(&b_fq, rect(100.0, 80.0, 50.0, 30.0));
        for _ in 0..10 {
            reg.update_rect(&b_fq, rect(100.0, 80.0, 50.0, 30.0));
        }
    });

    let events = overlap_events(&captured);
    assert_eq!(
        events.len(),
        1,
        "repeated update_rect with same overlap pair must emit only once; got {captured:?}"
    );
}

/// Overlap → move off → move back ON emits a fresh warning the second
/// time. Suppression releases when the overlap clears (or partner
/// changes).
#[test]
fn update_rect_clears_overlap_then_re_creates_warns_again() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", "main"));

    reg.register_scope(make_zone("/L/zone-a", "/L", rect(100.0, 80.0, 50.0, 30.0)));
    reg.register_scope(make_zone("/L/zone-b", "/L", rect(200.0, 80.0, 50.0, 30.0)));

    let b_fq = fq("/L/zone-b");

    let (_, captured) = capture_warns(|| {
        // First overlap.
        reg.update_rect(&b_fq, rect(100.0, 80.0, 50.0, 30.0));
        // Move B away — overlap clears, suppression should release.
        reg.update_rect(&b_fq, rect(300.0, 80.0, 50.0, 30.0));
        // Move B back onto A — fresh warning.
        reg.update_rect(&b_fq, rect(100.0, 80.0, 50.0, 30.0));
    });

    let events = overlap_events(&captured);
    assert_eq!(
        events.len(),
        2,
        "overlap → clear → overlap must emit two WARNs; got {captured:?}"
    );
    assert_eq!(events[0].op(), Some("update_rect"));
    assert_eq!(events[1].op(), Some("update_rect"));
}

/// `unregister_scope` clears any per-key suppression state for that
/// key, so re-registering the same FQM at the same overlapping
/// position emits a fresh warning.
#[test]
fn unregister_scope_resets_suppression_for_key() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", "main"));

    reg.register_scope(make_zone("/L/zone-a", "/L", rect(100.0, 80.0, 50.0, 30.0)));
    reg.register_scope(make_zone("/L/zone-b", "/L", rect(100.0, 80.0, 50.0, 30.0)));

    let b_fq = fq("/L/zone-b");

    let (_, captured) = capture_warns(|| {
        reg.unregister_scope(&b_fq);
        // Re-register at the same overlapping position. Suppression
        // was cleared by unregister, so this counts as a fresh
        // overlap and should emit one WARN.
        reg.register_scope(make_zone("/L/zone-b", "/L", rect(100.0, 80.0, 50.0, 30.0)));
    });

    let events = overlap_events(&captured);
    assert_eq!(
        events.len(),
        1,
        "unregister + re-register must release suppression and emit a fresh WARN; got {captured:?}"
    );
    assert_eq!(events[0].op(), Some("register_scope"));
}
