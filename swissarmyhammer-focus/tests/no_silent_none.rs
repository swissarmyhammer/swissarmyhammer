//! Source-of-truth integration tests for the no-silent-dropout contract
//! on the spatial-nav kernel.
//!
//! Pins the contract documented on
//! [`swissarmyhammer_focus::navigate`]: every nav / drill API returns a
//! [`FullyQualifiedMoniker`] (never `Option<FullyQualifiedMoniker>`); when motion is not possible
//! the kernel echoes the focused FQM, and torn-state paths
//! (unknown FQM, orphan parent reference) additionally emit
//! `tracing::error!` so the issue is observable in logs.
//!
//! Each test below sets up a registry, captures `tracing::error!`
//! events while invoking the kernel, and asserts:
//!
//! 1. The returned [`FullyQualifiedMoniker`] matches the contract for
//!    the path being exercised (peer match, drill-out, focused-FQM
//!    echo).
//! 2. The event capture's count matches the contract: zero error
//!    events on the well-formed semantic-edge paths (override wall,
//!    layer root, leaf with no children, empty zone), exactly one
//!    error event on the torn-state paths (unknown FQM, orphan
//!    parent ref).
//!
//! [`FullyQualifiedMoniker`]: swissarmyhammer_focus::FullyQualifiedMoniker

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use swissarmyhammer_focus::{
    BeamNavStrategy, Direction, FocusLayer, FocusScope, FocusZone, FullyQualifiedMoniker,
    LayerName, NavStrategy, Pixels, Rect, SegmentMoniker, SpatialRegistry, WindowLabel,
};
use tracing::{
    field::{Field, Visit},
    span::Attributes,
    Event, Id, Level, Subscriber,
};
use tracing_subscriber::{layer::Context, prelude::*, registry::LookupSpan, Layer};

// ---------------------------------------------------------------------------
// Tracing capture — custom Layer that records ERROR events emitted
// while a closure runs.
// ---------------------------------------------------------------------------

/// One captured `ERROR` event with the structured fields the kernel
/// emits on torn-state paths.
#[derive(Debug, Default)]
struct CapturedEvent {
    /// Field values rendered to string. Keys are field names; values
    /// are the rendered fmt::Debug or fmt::Display output.
    fields: HashMap<String, String>,
}

impl CapturedEvent {
    /// Read the `op` field. Returns `None` if the event did not
    /// include an `op` discriminator (which would be a bug — the
    /// kernel always emits `op` on torn-state errors).
    fn op(&self) -> Option<&str> {
        self.fields.get("op").map(String::as_str)
    }
}

/// Visitor that copies each field into a `HashMap<String, String>` on
/// the captured event.
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

/// Tracing [`Layer`] that records ERROR-level events into a shared
/// `Vec<CapturedEvent>`.
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

/// Run `f` with a tracing subscriber that captures `ERROR` events,
/// returning the captured events in arrival order.
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

impl Clone for CapturedEvent {
    fn clone(&self) -> Self {
        Self {
            fields: self.fields.clone(),
        }
    }
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

fn rect_zero() -> Rect {
    rect(0.0, 0.0, 10.0, 10.0)
}

fn fq_in_layer(layer_path: &str, segment: &str) -> FullyQualifiedMoniker {
    FullyQualifiedMoniker::from_string(format!("{layer_path}/{segment}"))
}

fn leaf(
    fq: FullyQualifiedMoniker,
    segment: &str,
    layer: &str,
    parent_zone: Option<FullyQualifiedMoniker>,
    overrides: HashMap<Direction, Option<FullyQualifiedMoniker>>,
    r: Rect,
) -> FocusScope {
    FocusScope {
        fq,
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone,
        overrides,
    }
}

fn zone(
    fq: FullyQualifiedMoniker,
    segment: &str,
    layer: &str,
    parent_zone: Option<FullyQualifiedMoniker>,
    last_focused: Option<FullyQualifiedMoniker>,
    r: Rect,
) -> FocusZone {
    FocusZone {
        fq,
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone,
        last_focused,
        overrides: HashMap::new(),
    }
}

fn layer_node(layer_fq: &str, segment: &str, window: &str, parent: Option<&str>) -> FocusLayer {
    FocusLayer {
        fq: FullyQualifiedMoniker::from_string(layer_fq),
        segment: SegmentMoniker::from_string(segment),
        name: LayerName::from_string("window"),
        parent: parent.map(FullyQualifiedMoniker::from_string),
        window_label: WindowLabel::from_string(window),
        last_focused: None,
    }
}

// ---------------------------------------------------------------------------
// Cardinal nav — semantic edges (no trace) and torn state (one trace).
// ---------------------------------------------------------------------------

/// A leaf at the layer root with no peers receives no parent zone to
/// drill out to. The cascade echoes the focused FQM, and tracing
/// emits zero `ERROR` events.
#[test]
fn nav_at_layer_root_returns_focused_fq_no_trace() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer_node("/L", "L", "main", None));
    let k_fq = fq_in_layer("/L", "ui:k");
    reg.register_scope(leaf(
        k_fq.clone(),
        "ui:k",
        "/L",
        None,
        HashMap::new(),
        rect_zero(),
    ));

    let strategy = BeamNavStrategy::new();
    let segment = SegmentMoniker::from_string("ui:k");
    let (result, captured) = capture_errors(|| strategy.next(&reg, &k_fq, &segment, Direction::Up));

    assert_eq!(result, k_fq, "layer-root nav echoes the focused FQM");
    assert_eq!(
        captured.len(),
        0,
        "layer-root edge is well-formed; no ERROR events expected, got {captured:?}"
    );
}

/// A leaf carrying an explicit `Right => None` override (the override
/// wall) returns the focused FQM. Wall is a semantic "stay put",
/// not a kernel error — zero `ERROR` events.
#[test]
fn nav_with_wall_override_returns_focused_fq_no_trace() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer_node("/L", "L", "main", None));
    let mut overrides = HashMap::new();
    overrides.insert(Direction::Right, None);
    let src_fq = fq_in_layer("/L", "ui:src");
    reg.register_scope(leaf(
        src_fq.clone(),
        "ui:src",
        "/L",
        None,
        overrides,
        rect(0.0, 0.0, 50.0, 50.0),
    ));
    // A would-be beam-search target to the right — must NOT be picked.
    reg.register_scope(leaf(
        fq_in_layer("/L", "ui:dst"),
        "ui:dst",
        "/L",
        None,
        HashMap::new(),
        rect(100.0, 0.0, 50.0, 50.0),
    ));

    let strategy = BeamNavStrategy::new();
    let segment = SegmentMoniker::from_string("ui:src");
    let (result, captured) =
        capture_errors(|| strategy.next(&reg, &src_fq, &segment, Direction::Right));

    assert_eq!(result, src_fq, "wall override echoes the focused FQM");
    assert_eq!(
        captured.len(),
        0,
        "override wall is well-formed; no ERROR events expected, got {captured:?}"
    );
}

/// A scope whose `parent_zone` references an unregistered FQM is in
/// torn state. The cascade echoes the focused FQM AND emits
/// exactly one `ERROR` event with `op = "nav"`.
#[test]
fn nav_with_torn_parent_returns_focused_fq_and_traces_error() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer_node("/L", "L", "main", None));
    // `src` claims `parent_zone = orphan-zone`, but no zone is
    // registered under that FQM.
    let src_fq = fq_in_layer("/L", "ui:src");
    let orphan_fq = fq_in_layer("/L", "orphan-zone");
    reg.register_scope(leaf(
        src_fq.clone(),
        "ui:src",
        "/L",
        Some(orphan_fq),
        HashMap::new(),
        rect_zero(),
    ));

    let strategy = BeamNavStrategy::new();
    let segment = SegmentMoniker::from_string("ui:src");
    let (result, captured) =
        capture_errors(|| strategy.next(&reg, &src_fq, &segment, Direction::Right));

    assert_eq!(result, src_fq, "torn parent ref echoes the focused FQM");
    assert_eq!(
        captured.len(),
        1,
        "torn parent ref must emit exactly one ERROR event, got {captured:?}"
    );
    assert_eq!(captured[0].op(), Some("nav"));
}

/// An unknown focused FQM is torn state — the cascade can't even read
/// the entry. The kernel emits one `ERROR` event with `op = "nav"`
/// and echoes the input FQM.
#[test]
fn nav_with_unknown_fq_returns_focused_fq_and_traces_error() {
    let reg = SpatialRegistry::new();
    let strategy = BeamNavStrategy::new();
    let ghost_fq = fq_in_layer("/L", "ui:ghost");
    let segment = SegmentMoniker::from_string("ui:ghost");

    let (result, captured) =
        capture_errors(|| strategy.next(&reg, &ghost_fq, &segment, Direction::Down));

    assert_eq!(result, ghost_fq, "unknown FQM echoes the focused FQM");
    assert_eq!(
        captured.len(),
        1,
        "unknown FQM must emit exactly one ERROR event, got {captured:?}"
    );
    assert_eq!(captured[0].op(), Some("nav"));
}

// ---------------------------------------------------------------------------
// Drill-in — semantic edges (no trace) and torn state (one trace).
// ---------------------------------------------------------------------------

/// A registered zone with no children returns the focused FQM
/// without tracing.
#[test]
fn drill_in_zone_with_no_children_returns_zone_fq_no_trace() {
    let mut reg = SpatialRegistry::new();
    let zone_fq = fq_in_layer("/L", "ui:zone");
    reg.register_zone(zone(
        zone_fq.clone(),
        "ui:zone",
        "/L",
        None,
        None,
        rect_zero(),
    ));

    let (result, captured) = capture_errors(|| reg.drill_in(zone_fq.clone(), &zone_fq));

    assert_eq!(result, zone_fq, "empty zone echoes the focused FQM");
    assert_eq!(
        captured.len(),
        0,
        "empty zone is well-formed; no ERROR events expected, got {captured:?}"
    );
}

/// A leaf has no children to drill into — semantic "stay put", no
/// tracing.
#[test]
fn drill_in_leaf_returns_leaf_fq_no_trace() {
    let mut reg = SpatialRegistry::new();
    let leaf_fq = fq_in_layer("/L", "ui:leaf");
    reg.register_scope(leaf(
        leaf_fq.clone(),
        "ui:leaf",
        "/L",
        None,
        HashMap::new(),
        rect_zero(),
    ));

    let (result, captured) = capture_errors(|| reg.drill_in(leaf_fq.clone(), &leaf_fq));

    assert_eq!(result, leaf_fq, "leaf echoes the focused FQM");
    assert_eq!(
        captured.len(),
        0,
        "leaf drill-in is well-formed; no ERROR events expected, got {captured:?}"
    );
}

/// An unknown FQM on `drill_in` is torn state — exactly one `ERROR`
/// event with `op = "drill_in"`.
#[test]
fn drill_in_unknown_fq_returns_focused_fq_and_traces_error() {
    let reg = SpatialRegistry::new();
    let focused_fq = fq_in_layer("/L", "ui:focused");
    let ghost_fq = fq_in_layer("/L", "ghost");
    let (result, captured) = capture_errors(|| reg.drill_in(ghost_fq, &focused_fq));

    assert_eq!(
        result, focused_fq,
        "unknown drill-in echoes the focused FQM"
    );
    assert_eq!(
        captured.len(),
        1,
        "unknown drill-in must emit exactly one ERROR event, got {captured:?}"
    );
    assert_eq!(captured[0].op(), Some("drill_in"));
}

// ---------------------------------------------------------------------------
// Drill-out — semantic edges (no trace) and torn state (one trace).
// ---------------------------------------------------------------------------

/// A scope at the layer root (no parent_zone) drills out to itself.
/// Well-formed edge, no tracing.
#[test]
fn drill_out_layer_root_returns_focused_fq_no_trace() {
    let mut reg = SpatialRegistry::new();
    let leaf_fq = fq_in_layer("/L", "ui:leaf");
    reg.register_scope(leaf(
        leaf_fq.clone(),
        "ui:leaf",
        "/L",
        None,
        HashMap::new(),
        rect_zero(),
    ));

    let (result, captured) = capture_errors(|| reg.drill_out(leaf_fq.clone(), &leaf_fq));

    assert_eq!(
        result, leaf_fq,
        "layer-root drill-out echoes the focused FQM"
    );
    assert_eq!(
        captured.len(),
        0,
        "layer-root drill-out is well-formed; no ERROR events expected, got {captured:?}"
    );
}

/// A scope whose `parent_zone` references an unregistered FQM is in
/// torn state. Drill-out emits one `ERROR` event with
/// `op = "drill_out"` and echoes the focused FQM.
#[test]
fn drill_out_torn_parent_returns_focused_fq_and_traces_error() {
    let mut reg = SpatialRegistry::new();
    let leaf_fq = fq_in_layer("/L", "ui:leaf");
    let orphan_fq = fq_in_layer("/L", "orphan-zone");
    reg.register_scope(leaf(
        leaf_fq.clone(),
        "ui:leaf",
        "/L",
        Some(orphan_fq),
        HashMap::new(),
        rect_zero(),
    ));

    let (result, captured) = capture_errors(|| reg.drill_out(leaf_fq.clone(), &leaf_fq));

    assert_eq!(
        result, leaf_fq,
        "torn-parent drill-out echoes the focused FQM"
    );
    assert_eq!(
        captured.len(),
        1,
        "torn-parent drill-out must emit exactly one ERROR event, got {captured:?}"
    );
    assert_eq!(captured[0].op(), Some("drill_out"));
}

/// Drill-out for an unknown FQM is torn state — exactly one `ERROR`
/// event with `op = "drill_out"`.
#[test]
fn drill_out_unknown_fq_returns_focused_fq_and_traces_error() {
    let reg = SpatialRegistry::new();
    let focused_fq = fq_in_layer("/L", "ui:focused");
    let ghost_fq = fq_in_layer("/L", "ghost");
    let (result, captured) = capture_errors(|| reg.drill_out(ghost_fq, &focused_fq));

    assert_eq!(
        result, focused_fq,
        "unknown drill-out echoes the focused FQM"
    );
    assert_eq!(
        captured.len(),
        1,
        "unknown drill-out must emit exactly one ERROR event, got {captured:?}"
    );
    assert_eq!(captured[0].op(), Some("drill_out"));
}
