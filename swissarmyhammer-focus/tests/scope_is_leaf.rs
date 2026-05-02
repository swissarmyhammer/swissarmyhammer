//! Pin the **scope-is-leaf** invariant for the spatial-nav kernel.
//!
//! The kernel exposes three peers: [`FocusLayer`] (modal boundary),
//! [`FocusZone`] (navigable container), and [`FocusScope`] (leaf). Zones can
//! contain other zones or scopes; scopes are leaves and must not contain
//! **anything** navigable — no [`FocusScope`], no [`FocusZone`], **and no
//! [`FocusLayer`]**. The previous registry happily accepted a child whose
//! `parent_zone` resolved to a [`FocusScope`] — the invariant was
//! documented but not enforced. Wrapping a non-leaf as a `<FocusScope>`
//! confuses the kernel's beam search (the scope's rect is the *whole*
//! sub-region but it is treated as a single leaf candidate) and breaks
//! "drill into the bar and remember the last-focused leaf" (the navbar
//! zone's last-focused leaf is the wrapper, not the actually-focused inner
//! control), which silently degrades keyboard nav in toolbars.
//!
//! Contract pinned by these tests:
//!
//! 1. **`Scope` registered under a `Scope` parent → exactly one
//!    `tracing::error!` carrying the `scope-not-leaf` grep token.**
//! 2. **`Zone` registered under a `Scope` parent → exactly one
//!    `tracing::error!` carrying the `scope-not-leaf` grep token.**
//! 3. **`Layer` mounted under a `Scope` (path-prefix) → exactly one
//!    `tracing::error!` carrying the `scope-not-leaf` grep token,
//!    tagged `kind = "layer"`.** A `<FocusScope>` cannot host a modal
//!    boundary either — the scope is a leaf and a Layer is a navigable
//!    primitive in its own right.
//! 4. **`Scope` (or `Zone`, or `Layer`) under a `Zone` parent → silent.**
//!    Legal layout.
//! 5. **Order independence.** A descendant that registers before its
//!    enclosing Scope is re-checked when the Scope eventually registers
//!    (forward + backward path-prefix scans). Either way, a structural
//!    offender produces exactly one error event per offender × ancestor
//!    pair, regardless of registration order.
//! 6. **Grep token.** Every emitted message contains the literal
//!    `scope-not-leaf` so a developer can `just logs | grep scope-not-leaf`
//!    and see only this class of violation.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use swissarmyhammer_focus::{
    FocusLayer, FocusScope, FocusZone, FullyQualifiedMoniker, LayerName, Pixels, Rect,
    SegmentMoniker, SpatialRegistry, WindowLabel,
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
    message: String,
    fields: HashMap<String, String>,
}

struct FieldVisitor<'a> {
    message: &'a mut String,
    fields: &'a mut HashMap<String, String>,
}

impl<'a> Visit for FieldVisitor<'a> {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            *self.message = value.to_string();
        } else {
            self.fields
                .insert(field.name().to_string(), value.to_string());
        }
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let formatted = format!("{value:?}");
        if field.name() == "message" {
            *self.message = formatted;
        } else {
            self.fields.insert(field.name().to_string(), formatted);
        }
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
        let mut visitor = FieldVisitor {
            message: &mut captured.message,
            fields: &mut captured.fields,
        };
        event.record(&mut visitor);
        self.events.lock().unwrap().push(captured);
    }

    fn on_new_span(&self, _attrs: &Attributes<'_>, _id: &Id, _ctx: Context<'_, S>) {}
}

/// Run `f` with a tracing subscriber that captures ERROR events. Returns
/// the closure's value plus the events captured during the closure body.
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

/// Filter events down to the scope-is-leaf class only. Other registry
/// emissions (structural-mismatch warnings, etc.) flow through the same
/// capture but are out of scope for this test file.
fn scope_not_leaf_events(events: &[CapturedEvent]) -> Vec<&CapturedEvent> {
    events
        .iter()
        .filter(|e| e.message.contains("scope-not-leaf"))
        .collect()
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

fn make_scope(path: &str, segment: &str, layer: &str, parent_zone: Option<&str>) -> FocusScope {
    FocusScope {
        fq: fq(path),
        segment: SegmentMoniker::from_string(segment),
        rect: rect(0.0, 0.0, 10.0, 10.0),
        layer_fq: fq(layer),
        parent_zone: parent_zone.map(fq),
        overrides: HashMap::new(),
    }
}

fn make_zone(path: &str, segment: &str, layer: &str, parent_zone: Option<&str>) -> FocusZone {
    FocusZone {
        fq: fq(path),
        segment: SegmentMoniker::from_string(segment),
        rect: rect(0.0, 0.0, 10.0, 10.0),
        layer_fq: fq(layer),
        parent_zone: parent_zone.map(fq),
        last_focused: None,
        overrides: HashMap::new(),
    }
}

/// Build a [`FocusLayer`] for tests. `path` is the layer's full FQM,
/// `parent_layer_path` is its stacking parent (a Layer FQM, or `None` for
/// a window root), and `window` is the Tauri webview label this layer
/// belongs to. The relative segment is derived from the final path
/// component so callers don't have to thread it separately.
fn make_layer(path: &str, parent_layer_path: Option<&str>, window: &str) -> FocusLayer {
    let segment = path.rsplit('/').next().unwrap_or("");
    FocusLayer {
        fq: fq(path),
        segment: SegmentMoniker::from_string(segment),
        name: LayerName::from_string(segment),
        parent: parent_layer_path.map(fq),
        window_label: WindowLabel::from_string(window),
        last_focused: None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Test 1 — child Scope registered under a parent that is itself a Scope.
/// One `scope-not-leaf` error must fire; the offending child's FQM must
/// appear in the structured fields.
#[test]
fn scope_under_scope_logs_error() {
    let layer = "/window";
    // Legal: Zone → Scope (the "first scope" — a leaf, correctly under a zone).
    let zone_path = "/window/zone:a";
    let first_scope_path = "/window/zone:a/leaf:a";
    // Illegal: a second Scope whose parent_zone is the first Scope's FQM.
    let illegal_child_path = "/window/zone:a/leaf:a/leaf:b";

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        reg.register_zone(make_zone(zone_path, "zone:a", layer, None));
        reg.register_scope(make_scope(
            first_scope_path,
            "leaf:a",
            layer,
            Some(zone_path),
        ));
        // Now register a second Scope whose parent_zone is the first Scope.
        reg.register_scope(make_scope(
            illegal_child_path,
            "leaf:b",
            layer,
            Some(first_scope_path),
        ));
    });

    let offenders = scope_not_leaf_events(&events);
    assert_eq!(
        offenders.len(),
        1,
        "scope-under-scope must emit exactly one scope-not-leaf error; got: {events:?}"
    );
    let ev = offenders[0];
    assert_eq!(
        ev.fields.get("kind").map(String::as_str),
        Some("scope"),
        "child kind must be reported as 'scope': {:?}",
        ev.fields
    );
    assert_eq!(
        ev.fields.get("parent_kind").map(String::as_str),
        Some("scope"),
        "parent_kind must be reported as 'scope': {:?}",
        ev.fields
    );
    let fq = ev.fields.get("fq").cloned().unwrap_or_default();
    assert!(
        fq.contains(illegal_child_path),
        "fq field must reference the offending child '{illegal_child_path}'; got fq='{fq}'"
    );
}

/// Test 2 — child Zone registered under a parent Scope. A Scope cannot
/// contain anything navigable, so a Zone child is just as illegal as a
/// Scope child.
#[test]
fn zone_under_scope_logs_error() {
    let layer = "/window";
    let zone_path = "/window/zone:a";
    let leaf_path = "/window/zone:a/leaf:a";
    // Illegal: a Zone whose parent_zone resolves to a Scope.
    let illegal_zone_path = "/window/zone:a/leaf:a/zone:b";

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        reg.register_zone(make_zone(zone_path, "zone:a", layer, None));
        reg.register_scope(make_scope(leaf_path, "leaf:a", layer, Some(zone_path)));
        // Zone under a Scope — illegal.
        reg.register_zone(make_zone(
            illegal_zone_path,
            "zone:b",
            layer,
            Some(leaf_path),
        ));
    });

    let offenders = scope_not_leaf_events(&events);
    assert_eq!(
        offenders.len(),
        1,
        "zone-under-scope must emit exactly one scope-not-leaf error; got: {events:?}"
    );
    let ev = offenders[0];
    assert_eq!(
        ev.fields.get("kind").map(String::as_str),
        Some("zone"),
        "child kind must be reported as 'zone': {:?}",
        ev.fields
    );
    assert_eq!(
        ev.fields.get("parent_kind").map(String::as_str),
        Some("scope"),
        "parent_kind must be reported as 'scope': {:?}",
        ev.fields
    );
}

/// Test 3 — legal layout: Layer → Zone → Scope. No error.
#[test]
fn scope_under_zone_silent() {
    let layer = "/window";
    let zone_path = "/window/zone:a";
    let leaf_path = "/window/zone:a/leaf:a";

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        reg.register_zone(make_zone(zone_path, "zone:a", layer, None));
        reg.register_scope(make_scope(leaf_path, "leaf:a", layer, Some(zone_path)));
    });

    let offenders = scope_not_leaf_events(&events);
    assert!(
        offenders.is_empty(),
        "Layer→Zone→Scope is legal; must emit no scope-not-leaf errors; got: {events:?}"
    );
}

/// Test 4a — child registered before parent. The illegal-shape error
/// must fire exactly once when the parent eventually registers as a
/// Scope.
///
/// Note on what is being pinned: when the child registers, its declared
/// `parent_zone` (the *parent's* FQM) is not yet in the registry, so
/// `warn_if_parent_is_scope` (the **forward** check) exits early and
/// emits nothing — that early-return is exactly the "deferred to the
/// backward scan" branch documented on `register_scope`. The child's
/// outer enclosing zone exists only so the parent can later resolve its
/// own enclosing zone without tripping its own forward check. The
/// assertion below is therefore pinning the **backward** scan: when the
/// parent later registers as a Scope, the registry walks its children
/// once and emits exactly one `scope-not-leaf` error for the pre-existing
/// illegal child.
#[test]
fn parent_registered_after_child_as_scope_emits_error_once() {
    let layer = "/window";
    let outer_zone = "/window/zone:a";
    let parent_path = "/window/zone:a/parent";
    let child_path = "/window/zone:a/parent/child";

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        // Outer zone first so the parent can resolve its enclosing zone.
        reg.register_zone(make_zone(outer_zone, "zone:a", layer, None));
        // Register the child first — its parent is not yet present.
        reg.register_scope(make_scope(child_path, "child", layer, Some(parent_path)));
        // Now the parent shows up as a Scope. The child's pending check
        // must fire here.
        reg.register_scope(make_scope(parent_path, "parent", layer, Some(outer_zone)));
    });

    let offenders = scope_not_leaf_events(&events);
    assert_eq!(
        offenders.len(),
        1,
        "deferred scope-not-leaf check must fire exactly once when the \
         parent is registered as a Scope; got: {events:?}"
    );
    let ev = offenders[0];
    let fq = ev.fields.get("fq").cloned().unwrap_or_default();
    assert!(
        fq.contains(child_path),
        "fq field must reference the offending child '{child_path}'; got fq='{fq}'"
    );
}

/// Test 4b — same shape, but the parent eventually registers as a Zone.
/// Legal layout, no error.
#[test]
fn parent_registered_after_child_as_zone_silent() {
    let layer = "/window";
    let outer_zone = "/window/zone:a";
    let parent_path = "/window/zone:a/parent";
    let child_path = "/window/zone:a/parent/child";

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        reg.register_zone(make_zone(outer_zone, "zone:a", layer, None));
        // Child first, parent unresolved.
        reg.register_scope(make_scope(child_path, "child", layer, Some(parent_path)));
        // Parent registers as a Zone — legal.
        reg.register_zone(make_zone(parent_path, "parent", layer, Some(outer_zone)));
    });

    let offenders = scope_not_leaf_events(&events);
    assert!(
        offenders.is_empty(),
        "child-then-Zone-parent is legal; must not emit scope-not-leaf; got: {events:?}"
    );
}

/// Test 5 — every emitted scope-not-leaf message must include the literal
/// grep token `scope-not-leaf` so `just logs | grep scope-not-leaf`
/// works.
#[test]
fn error_message_contains_grep_token() {
    let layer = "/window";
    let outer_zone = "/window/zone:a";
    let parent_scope = "/window/zone:a/leaf:a";
    let child_scope = "/window/zone:a/leaf:a/leaf:b";

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        reg.register_zone(make_zone(outer_zone, "zone:a", layer, None));
        reg.register_scope(make_scope(parent_scope, "leaf:a", layer, Some(outer_zone)));
        reg.register_scope(make_scope(child_scope, "leaf:b", layer, Some(parent_scope)));
    });

    let offenders = scope_not_leaf_events(&events);
    assert!(
        !offenders.is_empty(),
        "expected at least one error event; got none. all events: {events:?}"
    );
    for ev in &offenders {
        assert!(
            ev.message.contains("scope-not-leaf"),
            "every scope-not-leaf event must carry the grep token in its message; \
             got message='{}'",
            ev.message
        );
    }
}

/// Test 6a — **path-prefix branch only**: a `<FocusZone>` rendered inside
/// a misused `<FocusScope>` whose `parent_zone` points to a *legal* Zone
/// (e.g. the enclosing column zone, because `<FocusScope>` does not push
/// `FocusZoneContext.Provider` and `useParentZoneFq()` walks the
/// `FocusZoneContext` chain) must still emit one `scope-not-leaf` error
/// — detected via the **path-prefix** relation.
///
/// This is the production shape that the original `parent_zone`-only
/// enforcement could not catch: the entity card's `<FocusScope>` is at
/// `/L/col/card`, the column's `<FocusZone>` is at `/L/col`, and a Field
/// `<FocusZone>` rendered inside the card has FQM `/L/col/card/field`
/// with `parent_zone = /L/col` (the column, a Zone). The card is a
/// path-FQM ancestor of the field but not its `parent_zone`.
#[test]
fn path_prefix_zone_under_scope_logs_error() {
    let layer = "/L";
    let column_zone_path = "/L/col";
    let card_scope_path = "/L/col/card";
    // Field's parent_zone points at the *column*, not the card —
    // exactly what useParentZoneFq() yields when the card is a Scope.
    let field_zone_path = "/L/col/card/field";

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        reg.register_zone(make_zone(column_zone_path, "col", layer, None));
        reg.register_scope(make_scope(
            card_scope_path,
            "card",
            layer,
            Some(column_zone_path),
        ));
        // Field zone: parent_zone is the column (legal Zone), but the
        // FQM path puts it inside the card (illegal Scope).
        reg.register_zone(make_zone(
            field_zone_path,
            "field",
            layer,
            Some(column_zone_path),
        ));
    });

    let offenders = scope_not_leaf_events(&events);
    assert_eq!(
        offenders.len(),
        1,
        "path-prefix branch must catch a Zone whose FQM sits under a Scope, \
         even when the Zone's parent_zone points at a legal Zone elsewhere; \
         got: {events:?}"
    );
    let ev = offenders[0];
    assert_eq!(
        ev.fields.get("relation").map(String::as_str),
        Some("path-prefix"),
        "the offender's parent_zone is a legal Zone — only the path-prefix \
         relation should fire: {:?}",
        ev.fields
    );
    assert_eq!(
        ev.fields.get("kind").map(String::as_str),
        Some("zone"),
        "child kind must be reported as 'zone' (the Field is a Zone): {:?}",
        ev.fields
    );
    let fq = ev.fields.get("fq").cloned().unwrap_or_default();
    assert!(
        fq.contains(field_zone_path),
        "fq field must reference the offending Zone '{field_zone_path}'; got fq='{fq}'"
    );
    let parent_zone = ev.fields.get("parent_zone").cloned().unwrap_or_default();
    assert!(
        parent_zone.contains(card_scope_path),
        "parent_zone field on the event must point at the offending Scope \
         (the path-prefix ancestor) '{card_scope_path}'; got parent_zone='{parent_zone}'"
    );
}

/// Test 6b — **path-prefix backward branch**: when the inner Zone
/// registers BEFORE the wrapping Scope, the violation must still fire
/// when the Scope eventually registers, via the path-prefix backward scan.
#[test]
fn path_prefix_backward_scan_fires_when_scope_registers_late() {
    let layer = "/L";
    let column_zone_path = "/L/col";
    let card_scope_path = "/L/col/card";
    let field_zone_path = "/L/col/card/field";

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        reg.register_zone(make_zone(column_zone_path, "col", layer, None));
        // Field zone registers FIRST — its parent_zone is the column.
        reg.register_zone(make_zone(
            field_zone_path,
            "field",
            layer,
            Some(column_zone_path),
        ));
        // Card scope registers AFTER — backward scan must fire here.
        reg.register_scope(make_scope(
            card_scope_path,
            "card",
            layer,
            Some(column_zone_path),
        ));
    });

    let offenders = scope_not_leaf_events(&events);
    assert_eq!(
        offenders.len(),
        1,
        "deferred path-prefix check must fire exactly once when the Scope \
         registers after a path-descendant; got: {events:?}"
    );
    let ev = offenders[0];
    assert_eq!(
        ev.fields.get("relation").map(String::as_str),
        Some("path-prefix"),
        "backward scan must tag the relation as path-prefix: {:?}",
        ev.fields
    );
}

/// Test 6c — when the offender's `parent_zone` matches the ancestor Scope
/// AND its FQM is a path-descendant of that Scope (the synthetic case
/// from tests 1, 2, 4a, 5), the helper must emit ONE event tagged
/// `relation = "both"` — never two events for the same logical
/// violation.
#[test]
fn parent_zone_and_path_prefix_collapse_to_single_event() {
    let layer = "/L";
    let outer_zone = "/L/zone";
    let parent_scope = "/L/zone/p";
    let child_scope = "/L/zone/p/c";

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        reg.register_zone(make_zone(outer_zone, "zone", layer, None));
        reg.register_scope(make_scope(parent_scope, "p", layer, Some(outer_zone)));
        reg.register_scope(make_scope(child_scope, "c", layer, Some(parent_scope)));
    });

    let offenders = scope_not_leaf_events(&events);
    assert_eq!(
        offenders.len(),
        1,
        "both relations apply to the same offender × ancestor pair — emit \
         exactly one event, not one per relation; got: {events:?}"
    );
    assert_eq!(
        offenders[0].fields.get("relation").map(String::as_str),
        Some("both"),
        "collapsed event must be tagged relation = 'both': {:?}",
        offenders[0].fields
    );
}

/// Test 6 — same-shape re-registration must not re-fire `scope-not-leaf`.
///
/// The hot path under StrictMode double-mount, ResizeObserver rect refresh,
/// and the virtualizer placeholder→real-mount swap re-registers existing
/// primitives with the same `(kind, segment, layer_fq, parent_zone,
/// overrides)` tuple repeatedly. The registry's existing
/// `warn_on_structural_mismatch` deliberately silences that case; the
/// scope-is-leaf checks must follow the same precedent so an
/// already-reported illegal edge is not re-fired on every render.
///
/// Pin: register an illegal Scope-under-Scope edge once, capture the one
/// expected error, then re-register both parent and child several times
/// with identical shapes. Total `scope-not-leaf` events must remain 1
/// (the originally-novel edge, not duplicated by the re-registers).
#[test]
fn same_shape_reregistration_is_silent() {
    let layer = "/window";
    let outer_zone = "/window/zone:a";
    let parent_scope_path = "/window/zone:a/leaf:a";
    let child_scope_path = "/window/zone:a/leaf:a/leaf:b";

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        reg.register_zone(make_zone(outer_zone, "zone:a", layer, None));
        // Establish the illegal edge: child Scope under parent Scope.
        reg.register_scope(make_scope(
            parent_scope_path,
            "leaf:a",
            layer,
            Some(outer_zone),
        ));
        reg.register_scope(make_scope(
            child_scope_path,
            "leaf:b",
            layer,
            Some(parent_scope_path),
        ));
        // Now hammer the registry with same-shape re-registrations of
        // both ends of the illegal edge — exactly what StrictMode /
        // ResizeObserver / virtualizer churn does on the hot path.
        for _ in 0..3 {
            reg.register_scope(make_scope(
                parent_scope_path,
                "leaf:a",
                layer,
                Some(outer_zone),
            ));
            reg.register_scope(make_scope(
                child_scope_path,
                "leaf:b",
                layer,
                Some(parent_scope_path),
            ));
        }
    });

    let offenders = scope_not_leaf_events(&events);
    assert_eq!(
        offenders.len(),
        1,
        "same-shape re-registration must not re-emit scope-not-leaf; \
         exactly one error is expected for the originally-novel edge. \
         got: {events:?}"
    );
}

/// Test 7a — **forward layer-under-scope (path-prefix)**: a [`FocusLayer`]
/// pushed at an FQM that is a strict path-descendant of an already-
/// registered Scope must emit exactly one `scope-not-leaf` error tagged
/// `kind = "layer"` and `relation = "path-prefix"`.
///
/// A Scope is the leafmost primitive — it cannot host a Zone OR a Layer.
/// Layers do not have a `parent_zone` field (their `parent` field always
/// names another Layer FQM), so the parent-zone branch never applies to
/// Layers; only the path-prefix branch can fire.
#[test]
fn layer_under_scope_logs_error() {
    let outer_layer = "/window";
    let outer_zone = "/window/zone:a";
    let card_scope = "/window/zone:a/leaf:a";
    // Illegal: a Layer whose FQM sits inside the card scope.
    let illegal_layer = "/window/zone:a/leaf:a/dialog";

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        // Outer window-root layer so the scope's `layer_fq` resolves.
        reg.push_layer(make_layer(outer_layer, None, "main"));
        reg.register_zone(make_zone(outer_zone, "zone:a", outer_layer, None));
        reg.register_scope(make_scope(
            card_scope,
            "leaf:a",
            outer_layer,
            Some(outer_zone),
        ));
        // Illegal: layer pushed at a path-descendant of the Scope.
        reg.push_layer(make_layer(illegal_layer, Some(outer_layer), "main"));
    });

    let offenders = scope_not_leaf_events(&events);
    assert_eq!(
        offenders.len(),
        1,
        "layer-under-scope must emit exactly one scope-not-leaf error; got: {events:?}"
    );
    let ev = offenders[0];
    assert_eq!(
        ev.fields.get("kind").map(String::as_str),
        Some("layer"),
        "child kind must be reported as 'layer': {:?}",
        ev.fields
    );
    assert_eq!(
        ev.fields.get("parent_kind").map(String::as_str),
        Some("scope"),
        "parent_kind must be reported as 'scope': {:?}",
        ev.fields
    );
    assert_eq!(
        ev.fields.get("relation").map(String::as_str),
        Some("path-prefix"),
        "Layers only ever match the path-prefix relation: {:?}",
        ev.fields
    );
    let fq = ev.fields.get("fq").cloned().unwrap_or_default();
    assert!(
        fq.contains(illegal_layer),
        "fq field must reference the offending Layer '{illegal_layer}'; got fq='{fq}'"
    );
    let parent_zone = ev.fields.get("parent_zone").cloned().unwrap_or_default();
    assert!(
        parent_zone.contains(card_scope),
        "parent_zone field on the event must point at the offending Scope \
         (the path-prefix ancestor) '{card_scope}'; got parent_zone='{parent_zone}'"
    );
}

/// Test 7b — **backward layer-under-scope (path-prefix)**: when a Layer is
/// pushed BEFORE the wrapping Scope registers, the violation must still
/// fire when the Scope eventually registers, via the path-prefix backward
/// scan. Pins the order-independence contract for the layer arm of the
/// backward pass that [`SpatialRegistry::register_scope`] runs over the
/// layers map.
#[test]
fn layer_path_prefix_backward_scan_when_scope_registers_late() {
    let outer_layer = "/window";
    let outer_zone = "/window/zone:a";
    let card_scope = "/window/zone:a/leaf:a";
    let illegal_layer = "/window/zone:a/leaf:a/dialog";

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer(outer_layer, None, "main"));
        reg.register_zone(make_zone(outer_zone, "zone:a", outer_layer, None));
        // Layer pushed FIRST — its FQM happens to be a path-descendant
        // of the Scope that has not yet registered. The forward
        // path-prefix scan in `push_layer` finds no Scope ancestor and
        // is silent.
        reg.push_layer(make_layer(illegal_layer, Some(outer_layer), "main"));
        // Scope registers LATER — its backward scan walks the layers
        // map and finds the pre-existing illegal Layer descendant.
        reg.register_scope(make_scope(
            card_scope,
            "leaf:a",
            outer_layer,
            Some(outer_zone),
        ));
    });

    let offenders = scope_not_leaf_events(&events);
    assert_eq!(
        offenders.len(),
        1,
        "deferred layer path-prefix check must fire exactly once when the \
         Scope registers after a Layer descendant; got: {events:?}"
    );
    let ev = offenders[0];
    assert_eq!(
        ev.fields.get("kind").map(String::as_str),
        Some("layer"),
        "backward scan must tag the descendant kind as 'layer': {:?}",
        ev.fields
    );
    assert_eq!(
        ev.fields.get("relation").map(String::as_str),
        Some("path-prefix"),
        "backward scan over layers must tag the relation as path-prefix: {:?}",
        ev.fields
    );
    let fq = ev.fields.get("fq").cloned().unwrap_or_default();
    assert!(
        fq.contains(illegal_layer),
        "fq field must reference the offending Layer '{illegal_layer}'; got fq='{fq}'"
    );
}

/// Test 7c — **legal Layer-under-Zone**: a Layer whose FQM is a strict
/// path-descendant of a Zone (and not of any Scope) is the legal layout
/// (e.g. an inspector / palette / dialog layer mounted inside the window
/// root layout). No `scope-not-leaf` event must fire.
#[test]
fn layer_under_zone_silent() {
    let outer_layer = "/window";
    let outer_zone = "/window/zone:a";
    // Legal nested layer: parent is a Zone, not a Scope.
    let nested_layer = "/window/zone:a/inspector";

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer(outer_layer, None, "main"));
        reg.register_zone(make_zone(outer_zone, "zone:a", outer_layer, None));
        reg.push_layer(make_layer(nested_layer, Some(outer_layer), "main"));
    });

    let offenders = scope_not_leaf_events(&events);
    assert!(
        offenders.is_empty(),
        "Layer-inside-Zone is legal; must emit no scope-not-leaf errors; got: {events:?}"
    );
}

/// Test 7d — same-shape re-registration of an illegal Layer-under-Scope
/// edge must not re-fire `scope-not-leaf`. Mirrors
/// [`same_shape_reregistration_is_silent`] for the layer arm — the hot
/// paths under StrictMode double-mount and palette open/close cycles
/// re-push the same layer with identical `(segment, name, parent,
/// window_label)` repeatedly; the registry must treat that as a no-op
/// for invariant-checking purposes.
///
/// Pin: register an illegal Layer-under-Scope edge once, capture the
/// expected single error, then re-register both ends three times with
/// identical shapes. Total `scope-not-leaf` events stays at 1.
#[test]
fn layer_same_shape_reregistration_is_silent() {
    let outer_layer = "/window";
    let outer_zone = "/window/zone:a";
    let card_scope = "/window/zone:a/leaf:a";
    let illegal_layer = "/window/zone:a/leaf:a/dialog";

    let ((), events) = capture_errors(|| {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer(outer_layer, None, "main"));
        reg.register_zone(make_zone(outer_zone, "zone:a", outer_layer, None));
        // Establish the illegal edge: Scope wrapping a Layer.
        reg.register_scope(make_scope(
            card_scope,
            "leaf:a",
            outer_layer,
            Some(outer_zone),
        ));
        reg.push_layer(make_layer(illegal_layer, Some(outer_layer), "main"));
        // Hammer same-shape re-registrations of both ends — the
        // StrictMode / palette open-close hot path.
        for _ in 0..3 {
            reg.register_scope(make_scope(
                card_scope,
                "leaf:a",
                outer_layer,
                Some(outer_zone),
            ));
            reg.push_layer(make_layer(illegal_layer, Some(outer_layer), "main"));
        }
    });

    let offenders = scope_not_leaf_events(&events);
    assert_eq!(
        offenders.len(),
        1,
        "same-shape Layer-under-Scope re-registration must not re-emit \
         scope-not-leaf; exactly one error is expected for the \
         originally-novel edge. got: {events:?}"
    );
}
