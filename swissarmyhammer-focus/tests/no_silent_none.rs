//! Source-of-truth integration tests for the no-silent-dropout contract
//! on the spatial-nav kernel.
//!
//! Pins the contract documented on
//! [`swissarmyhammer_focus::navigate`]: every nav / drill API returns a
//! [`Moniker`] (never `Option<Moniker>`); when motion is not possible
//! the kernel echoes the focused moniker, and torn-state paths
//! (unknown key, orphan parent reference) additionally emit
//! `tracing::error!` so the issue is observable in logs.
//!
//! Each test below sets up a registry, captures `tracing::error!`
//! events while invoking the kernel, and asserts:
//!
//! 1. The returned [`Moniker`] matches the contract for the path
//!    being exercised (peer match, drill-out, focused-moniker echo).
//! 2. The event capture's count matches the contract: zero error
//!    events on the well-formed semantic-edge paths (override wall,
//!    layer root, leaf with no children, empty zone), exactly one
//!    error event on the torn-state paths (unknown key, orphan
//!    parent ref).
//!
//! The tracing capture is a custom [`tracing_subscriber`] layer that
//! counts events and stores their fields — we read fields back to
//! verify the kernel emitted the expected `op = "nav" | "drill_in" |
//! "drill_out"` discriminator and included the focused key / moniker
//! in the structured payload.
//!
//! [`Moniker`]: swissarmyhammer_focus::Moniker

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use swissarmyhammer_focus::{
    BeamNavStrategy, Direction, FocusLayer, FocusScope, FocusZone, LayerKey, LayerName, Moniker,
    NavStrategy, Pixels, Rect, SpatialKey, SpatialRegistry, WindowLabel,
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
///
/// The kernel emits events with at minimum an `op` field
/// (`"nav"` / `"drill_in"` / `"drill_out"`) and a message describing
/// the torn-state class. Optional `focused_key`, `focused_moniker`,
/// and `parent_zone_key` fields carry the keys / monikers involved.
#[derive(Debug, Default)]
struct CapturedEvent {
    /// Field values rendered to string. Keys are field names; values
    /// are the rendered fmt::Debug or fmt::Display output. The kernel
    /// uses the structured field shape `op = "nav"` / `focused_key =
    /// %key`, which renders here as a `String`.
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
/// the captured event. Implements both the `&dyn Debug` and `&dyn
/// Display` arms so structured fields like `focused_key = %key` (which
/// uses Display) and `op = "nav"` (which uses Debug for the literal)
/// both round-trip readably.
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
/// `Vec<CapturedEvent>`. Wrapping the `Vec` in `Arc<Mutex<…>>` lets
/// the test thread inspect captures after the closure runs.
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
        // Record the message field too so tests can assert on the
        // human-readable description if needed. tracing wraps the
        // free-form message under a synthetic `message` field.
        let mut visitor = FieldVisitor(&mut captured.fields);
        event.record(&mut visitor);
        self.events.lock().unwrap().push(captured);
    }

    // No-op span hooks — we capture events only, not span lifecycles.
    fn on_new_span(&self, _attrs: &Attributes<'_>, _id: &Id, _ctx: Context<'_, S>) {}
}

/// Run `f` with a tracing subscriber that captures `ERROR` events,
/// returning the captured events in arrival order.
///
/// Uses [`tracing::subscriber::with_default`] so the subscriber is
/// scoped to the closure — concurrent tests do not see each other's
/// captures, and the global default (which test runners may have
/// installed for log output) is restored afterward.
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

// `Clone` impl for `CapturedEvent` so we can drain the mutex into a
// returnable Vec.
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

fn leaf(
    key: &str,
    moniker: &str,
    layer: &str,
    parent_zone: Option<&str>,
    overrides: HashMap<Direction, Option<Moniker>>,
    r: Rect,
) -> FocusScope {
    FocusScope {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect: r,
        layer_key: LayerKey::from_string(layer),
        parent_zone: parent_zone.map(SpatialKey::from_string),
        overrides,
    }
}

fn zone(
    key: &str,
    moniker: &str,
    layer: &str,
    parent_zone: Option<&str>,
    last_focused: Option<&str>,
    r: Rect,
) -> FocusZone {
    FocusZone {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect: r,
        layer_key: LayerKey::from_string(layer),
        parent_zone: parent_zone.map(SpatialKey::from_string),
        last_focused: last_focused.map(SpatialKey::from_string),
        overrides: HashMap::new(),
    }
}

fn layer_node(key: &str, window: &str, parent: Option<&str>) -> FocusLayer {
    FocusLayer {
        key: LayerKey::from_string(key),
        name: LayerName::from_string("window"),
        parent: parent.map(LayerKey::from_string),
        window_label: WindowLabel::from_string(window),
        last_focused: None,
    }
}

// ---------------------------------------------------------------------------
// Cardinal nav — semantic edges (no trace) and torn state (one trace).
// ---------------------------------------------------------------------------

/// A leaf at the layer root with no peers receives no parent zone to
/// drill out to. The cascade echoes the focused moniker, and tracing
/// emits zero `ERROR` events — this is a well-formed semantic edge.
#[test]
fn nav_at_layer_root_returns_focused_moniker_no_trace() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer_node("L", "main", None));
    reg.register_scope(leaf("k", "ui:k", "L", None, HashMap::new(), rect_zero()));

    let strategy = BeamNavStrategy::new();
    let focused_moniker = Moniker::from_string("ui:k");
    let (result, captured) = capture_errors(|| {
        strategy.next(
            &reg,
            &SpatialKey::from_string("k"),
            &focused_moniker,
            Direction::Up,
        )
    });

    assert_eq!(
        result, focused_moniker,
        "layer-root nav echoes the focused moniker"
    );
    assert_eq!(
        captured.len(),
        0,
        "layer-root edge is well-formed; no ERROR events expected, got {captured:?}"
    );
}

/// A leaf carrying an explicit `Right => None` override (the override
/// wall) returns the focused moniker. Wall is a semantic "stay put",
/// not a kernel error — zero `ERROR` events.
#[test]
fn nav_with_wall_override_returns_focused_moniker_no_trace() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer_node("L", "main", None));
    let mut overrides = HashMap::new();
    overrides.insert(Direction::Right, None);
    reg.register_scope(leaf(
        "src",
        "ui:src",
        "L",
        None,
        overrides,
        rect(0.0, 0.0, 50.0, 50.0),
    ));
    // A would-be beam-search target to the right — must NOT be picked
    // because the override wall fires first.
    reg.register_scope(leaf(
        "dst",
        "ui:dst",
        "L",
        None,
        HashMap::new(),
        rect(100.0, 0.0, 50.0, 50.0),
    ));

    let strategy = BeamNavStrategy::new();
    let focused_moniker = Moniker::from_string("ui:src");
    let (result, captured) = capture_errors(|| {
        strategy.next(
            &reg,
            &SpatialKey::from_string("src"),
            &focused_moniker,
            Direction::Right,
        )
    });

    assert_eq!(
        result, focused_moniker,
        "wall override echoes the focused moniker"
    );
    assert_eq!(
        captured.len(),
        0,
        "override wall is well-formed; no ERROR events expected, got {captured:?}"
    );
}

/// A scope whose `parent_zone` references an unregistered key is in
/// torn state. The cascade echoes the focused moniker AND emits
/// exactly one `ERROR` event with `op = "nav"`.
#[test]
fn nav_with_torn_parent_returns_focused_moniker_and_traces_error() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer_node("L", "main", None));
    // `src` claims `parent_zone = orphan-zone`, but no zone is
    // registered under that key.
    reg.register_scope(leaf(
        "src",
        "ui:src",
        "L",
        Some("orphan-zone"),
        HashMap::new(),
        rect_zero(),
    ));

    let strategy = BeamNavStrategy::new();
    let focused_moniker = Moniker::from_string("ui:src");
    let (result, captured) = capture_errors(|| {
        strategy.next(
            &reg,
            &SpatialKey::from_string("src"),
            &focused_moniker,
            Direction::Right,
        )
    });

    assert_eq!(
        result, focused_moniker,
        "torn parent ref echoes the focused moniker"
    );
    assert_eq!(
        captured.len(),
        1,
        "torn parent ref must emit exactly one ERROR event, got {captured:?}"
    );
    assert_eq!(captured[0].op(), Some("nav"));
}

/// An unknown focused key is torn state — the cascade can't even read
/// the entry. The kernel emits one `ERROR` event with `op = "nav"`
/// and echoes the input moniker.
#[test]
fn nav_with_unknown_key_returns_focused_moniker_and_traces_error() {
    let reg = SpatialRegistry::new();
    let strategy = BeamNavStrategy::new();
    let focused_moniker = Moniker::from_string("ui:ghost");

    let (result, captured) = capture_errors(|| {
        strategy.next(
            &reg,
            &SpatialKey::from_string("ghost"),
            &focused_moniker,
            Direction::Down,
        )
    });

    assert_eq!(
        result, focused_moniker,
        "unknown key echoes the focused moniker"
    );
    assert_eq!(
        captured.len(),
        1,
        "unknown key must emit exactly one ERROR event, got {captured:?}"
    );
    assert_eq!(captured[0].op(), Some("nav"));
}

// ---------------------------------------------------------------------------
// Drill-in — semantic edges (no trace) and torn state (one trace).
// ---------------------------------------------------------------------------

/// A registered zone with no children returns the focused moniker
/// without tracing — the React side detects equality and falls
/// through to inline edit / no-op.
#[test]
fn drill_in_zone_with_no_children_returns_zone_moniker_no_trace() {
    let mut reg = SpatialRegistry::new();
    reg.register_zone(zone("z", "ui:zone", "L", None, None, rect_zero()));

    let zone_moniker = Moniker::from_string("ui:zone");
    let (result, captured) =
        capture_errors(|| reg.drill_in(SpatialKey::from_string("z"), &zone_moniker));

    assert_eq!(
        result, zone_moniker,
        "empty zone echoes the focused moniker"
    );
    assert_eq!(
        captured.len(),
        0,
        "empty zone is well-formed; no ERROR events expected, got {captured:?}"
    );
}

/// A leaf has no children to drill into — semantic "stay put", no
/// tracing.
#[test]
fn drill_in_leaf_returns_leaf_moniker_no_trace() {
    let mut reg = SpatialRegistry::new();
    reg.register_scope(leaf(
        "leaf",
        "ui:leaf",
        "L",
        None,
        HashMap::new(),
        rect_zero(),
    ));

    let leaf_moniker = Moniker::from_string("ui:leaf");
    let (result, captured) =
        capture_errors(|| reg.drill_in(SpatialKey::from_string("leaf"), &leaf_moniker));

    assert_eq!(result, leaf_moniker, "leaf echoes the focused moniker");
    assert_eq!(
        captured.len(),
        0,
        "leaf drill-in is well-formed; no ERROR events expected, got {captured:?}"
    );
}

/// An unknown key on `drill_in` is torn state — exactly one `ERROR`
/// event with `op = "drill_in"`.
#[test]
fn drill_in_unknown_key_returns_focused_moniker_and_traces_error() {
    let reg = SpatialRegistry::new();
    let focused_moniker = Moniker::from_string("ui:focused");
    let (result, captured) =
        capture_errors(|| reg.drill_in(SpatialKey::from_string("ghost"), &focused_moniker));

    assert_eq!(
        result, focused_moniker,
        "unknown drill-in echoes the focused moniker"
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

/// A scope at the layer root (no parent_zone) drills out to itself —
/// the React adapter detects equality and falls through to
/// `app.dismiss`. Well-formed edge, no tracing.
#[test]
fn drill_out_layer_root_returns_focused_moniker_no_trace() {
    let mut reg = SpatialRegistry::new();
    reg.register_scope(leaf(
        "leaf",
        "ui:leaf",
        "L",
        None,
        HashMap::new(),
        rect_zero(),
    ));

    let leaf_moniker = Moniker::from_string("ui:leaf");
    let (result, captured) =
        capture_errors(|| reg.drill_out(SpatialKey::from_string("leaf"), &leaf_moniker));

    assert_eq!(
        result, leaf_moniker,
        "layer-root drill-out echoes the focused moniker"
    );
    assert_eq!(
        captured.len(),
        0,
        "layer-root drill-out is well-formed; no ERROR events expected, got {captured:?}"
    );
}

/// A scope whose `parent_zone` references an unregistered key is in
/// torn state. Drill-out emits one `ERROR` event with
/// `op = "drill_out"` and echoes the focused moniker.
#[test]
fn drill_out_torn_parent_returns_focused_moniker_and_traces_error() {
    let mut reg = SpatialRegistry::new();
    reg.register_scope(leaf(
        "leaf",
        "ui:leaf",
        "L",
        Some("orphan-zone"),
        HashMap::new(),
        rect_zero(),
    ));

    let leaf_moniker = Moniker::from_string("ui:leaf");
    let (result, captured) =
        capture_errors(|| reg.drill_out(SpatialKey::from_string("leaf"), &leaf_moniker));

    assert_eq!(
        result, leaf_moniker,
        "torn-parent drill-out echoes the focused moniker"
    );
    assert_eq!(
        captured.len(),
        1,
        "torn-parent drill-out must emit exactly one ERROR event, got {captured:?}"
    );
    assert_eq!(captured[0].op(), Some("drill_out"));
}

/// Drill-out for an unknown key is torn state — exactly one `ERROR`
/// event with `op = "drill_out"`.
#[test]
fn drill_out_unknown_key_returns_focused_moniker_and_traces_error() {
    let reg = SpatialRegistry::new();
    let focused_moniker = Moniker::from_string("ui:focused");
    let (result, captured) =
        capture_errors(|| reg.drill_out(SpatialKey::from_string("ghost"), &focused_moniker));

    assert_eq!(
        result, focused_moniker,
        "unknown drill-out echoes the focused moniker"
    );
    assert_eq!(
        captured.len(),
        1,
        "unknown drill-out must emit exactly one ERROR event, got {captured:?}"
    );
    assert_eq!(captured[0].op(), Some("drill_out"));
}
