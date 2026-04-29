//! Integration tests for `RegisterEntry` and the batch-register entry point.
//!
//! Headless pattern matching `tests/focus_registry.rs` — pure Rust, no
//! Tauri runtime. The `RegisterEntry` enum is the wire shape that the
//! Tauri `spatial_register_batch` command receives; these tests exercise
//! its serde round-trip and the registry-side application path that lets
//! a single lock register N entries atomically.
//!
//! Coverage:
//!
//! - **Wire shape** — `RegisterEntry` serializes with a `"kind"` tag
//!   discriminator (`"scope"` / `"zone"`) and `snake_case` rename to
//!   match the rest of the kernel's enums.
//! - **All fields newtyped** — `fq`, `segment`, `rect`, `layer_fq`,
//!   `parent_zone`, `overrides` use the existing newtypes; no bare
//!   `String` or `f64` on the wire.
//! - **Atomic application** — `SpatialRegistry::apply_batch` registers
//!   N entries in one pass without splitting across multiple calls.
//! - **Idempotent on FQM** — a leaf scope registered twice with the same
//!   FQM keeps the leaf variant; subsequent rect overwrites the prior
//!   value.
//! - **Kind mismatch is an error** — registering a zone for an FQM that
//!   was previously a leaf scope (or vice versa) is rejected so a
//!   placeholder/real-mount swap can't silently change the variant.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    BatchRegisterError, FocusScope, FocusZone, FullyQualifiedMoniker, Pixels, Rect, RegisterEntry,
    ScopeKind, SegmentMoniker, SpatialRegistry,
};

// ---------------------------------------------------------------------------
// Builders
// ---------------------------------------------------------------------------

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        x: Pixels::new(x),
        y: Pixels::new(y),
        width: Pixels::new(w),
        height: Pixels::new(h),
    }
}

fn scope_entry(
    fq: &str,
    segment: &str,
    layer: &str,
    parent_zone: Option<&str>,
    r: Rect,
) -> RegisterEntry {
    RegisterEntry::Scope {
        fq: FullyQualifiedMoniker::from_string(fq),
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone: parent_zone.map(FullyQualifiedMoniker::from_string),
        overrides: HashMap::new(),
    }
}

fn zone_entry(
    fq: &str,
    segment: &str,
    layer: &str,
    parent_zone: Option<&str>,
    r: Rect,
) -> RegisterEntry {
    RegisterEntry::Zone {
        fq: FullyQualifiedMoniker::from_string(fq),
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone: parent_zone.map(FullyQualifiedMoniker::from_string),
        overrides: HashMap::new(),
    }
}

// ---------------------------------------------------------------------------
// Wire shape
// ---------------------------------------------------------------------------

/// `RegisterEntry::Scope` round-trips through JSON with a `"kind"
/// = "scope"` tag and snake-cased field names.
#[test]
fn register_entry_scope_serializes_with_kind_tag() {
    let entry = scope_entry("/L/k", "k", "/L", Some("/L/zone"), rect(1.0, 2.0, 3.0, 4.0));
    let json = serde_json::to_value(&entry).expect("serialize");
    assert_eq!(json["kind"], "scope");
    assert_eq!(json["fq"], "/L/k");
    assert_eq!(json["segment"], "k");
    assert_eq!(json["layer_fq"], "/L");
    assert_eq!(json["parent_zone"], "/L/zone");
}

/// `RegisterEntry::Zone` round-trips through JSON with a `"kind" =
/// "zone"` tag.
#[test]
fn register_entry_zone_serializes_with_kind_tag() {
    let entry = zone_entry("/L/z", "z", "/L", None, rect(0.0, 0.0, 10.0, 10.0));
    let json = serde_json::to_value(&entry).expect("serialize");
    assert_eq!(json["kind"], "zone");
    assert_eq!(json["fq"], "/L/z");
    assert_eq!(json["parent_zone"], serde_json::Value::Null);
}

/// `RegisterEntry` deserializes from the same wire shape — the React
/// virtualizer constructs a `Vec<RegisterEntry>` and ships it through
/// the Tauri IPC, where serde matches on the `kind` tag.
#[test]
fn register_entry_deserializes_scope_via_kind_tag() {
    let json = serde_json::json!({
        "kind": "scope",
        "fq": "/L/k",
        "segment": "k",
        "rect": { "x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0 },
        "layer_fq": "/L",
        "parent_zone": null,
        "overrides": {}
    });
    let entry: RegisterEntry = serde_json::from_value(json).expect("deserialize");
    match entry {
        RegisterEntry::Scope { fq, segment, .. } => {
            assert_eq!(fq, FullyQualifiedMoniker::from_string("/L/k"));
            assert_eq!(segment, SegmentMoniker::from_string("k"));
        }
        other => panic!("expected Scope, got {other:?}"),
    }
}

/// `RegisterEntry` deserializes a zone entry the same way.
#[test]
fn register_entry_deserializes_zone_via_kind_tag() {
    let json = serde_json::json!({
        "kind": "zone",
        "fq": "/L/z",
        "segment": "z",
        "rect": { "x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0 },
        "layer_fq": "/L",
        "parent_zone": null,
        "overrides": {}
    });
    let entry: RegisterEntry = serde_json::from_value(json).expect("deserialize");
    assert!(matches!(entry, RegisterEntry::Zone { .. }));
}

// ---------------------------------------------------------------------------
// Atomic application
// ---------------------------------------------------------------------------

/// `apply_batch` registers N entries under the same registry call —
/// the test exercises the contract by handing it a mixed scope/zone
/// vector and verifying every FQM resolves afterward.
#[test]
fn apply_batch_registers_all_entries() {
    let mut reg = SpatialRegistry::new();
    let entries = vec![
        zone_entry("/L/z1", "z1", "/L", None, rect(0.0, 0.0, 100.0, 100.0)),
        scope_entry(
            "/L/z1/f1",
            "f1",
            "/L",
            Some("/L/z1"),
            rect(0.0, 0.0, 10.0, 10.0),
        ),
        scope_entry(
            "/L/z1/f2",
            "f2",
            "/L",
            Some("/L/z1"),
            rect(20.0, 0.0, 10.0, 10.0),
        ),
    ];

    reg.apply_batch(entries).expect("batch apply succeeds");

    assert!(reg.is_registered(&FullyQualifiedMoniker::from_string("/L/z1")));
    assert!(reg.is_registered(&FullyQualifiedMoniker::from_string("/L/z1/f1")));
    assert!(reg.is_registered(&FullyQualifiedMoniker::from_string("/L/z1/f2")));
}

// ---------------------------------------------------------------------------
// Idempotent on FQM
// ---------------------------------------------------------------------------

/// Registering the same leaf scope FQM twice with a different rect
/// overwrites the rect and keeps the leaf variant.
#[test]
fn re_register_same_scope_fq_overwrites_rect_keeps_variant() {
    let mut reg = SpatialRegistry::new();
    reg.apply_batch(vec![scope_entry(
        "/L/k",
        "k",
        "/L",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    )])
    .expect("first batch");

    reg.apply_batch(vec![scope_entry(
        "/L/k",
        "k",
        "/L",
        None,
        rect(50.0, 50.0, 20.0, 20.0),
    )])
    .expect("second batch");

    let scope = reg
        .scope(&FullyQualifiedMoniker::from_string("/L/k"))
        .expect("FQM still registered as a leaf scope");
    let r = scope.rect;
    assert_eq!(r.left(), Pixels::new(50.0));
    assert_eq!(r.top(), Pixels::new(50.0));
    assert_eq!(r.right(), Pixels::new(70.0));
    assert_eq!(r.bottom(), Pixels::new(70.0));
}

// ---------------------------------------------------------------------------
// Kind mismatch
// ---------------------------------------------------------------------------

/// Registering a zone for an FQM previously registered as a leaf scope
/// returns an error — the kind on a stable FQM must not change between
/// the placeholder mount and the real-mount swap.
#[test]
fn batch_register_zone_for_existing_scope_fq_errors() {
    let mut reg = SpatialRegistry::new();
    reg.apply_batch(vec![scope_entry(
        "/L/k",
        "k",
        "/L",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    )])
    .expect("first batch");

    let result = reg.apply_batch(vec![zone_entry(
        "/L/k",
        "k",
        "/L",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    )]);
    assert!(
        result.is_err(),
        "registering a zone for a leaf scope FQM must fail",
    );
}

// ---------------------------------------------------------------------------
// Single-entry idempotency (the placeholder/real-mount swap path)
// ---------------------------------------------------------------------------

/// `register_scope` called with the same FQM twice overwrites the rect
/// on the existing entry while keeping the variant unchanged.
#[test]
fn register_scope_twice_overwrites_rect_keeps_variant() {
    let mut reg = SpatialRegistry::new();
    let fq = FullyQualifiedMoniker::from_string("/L/k");

    reg.register_scope(FocusScope {
        fq: fq.clone(),
        segment: SegmentMoniker::from_string("k"),
        rect: rect(0.0, 0.0, 10.0, 10.0),
        layer_fq: FullyQualifiedMoniker::from_string("/L"),
        parent_zone: None,
        overrides: HashMap::new(),
    });

    reg.register_scope(FocusScope {
        fq: fq.clone(),
        segment: SegmentMoniker::from_string("k"),
        rect: rect(50.0, 50.0, 20.0, 20.0),
        layer_fq: FullyQualifiedMoniker::from_string("/L"),
        parent_zone: None,
        overrides: HashMap::new(),
    });

    let scope = reg.scope(&fq).expect("FQM still registered as a leaf");
    assert_eq!(scope.rect.left(), Pixels::new(50.0));
    assert_eq!(scope.rect.top(), Pixels::new(50.0));
}

/// `register_scope` called for an FQM currently registered as a `Zone`
/// — the standalone `register_*` methods overwrite without validating
/// the kind. The kind-stability check lives on the **batch** entry
/// point.
#[test]
fn register_scope_overwrites_existing_zone_silently() {
    let mut reg = SpatialRegistry::new();
    let fq = FullyQualifiedMoniker::from_string("/L/k");

    reg.register_zone(FocusZone {
        fq: fq.clone(),
        segment: SegmentMoniker::from_string("k"),
        rect: rect(0.0, 0.0, 10.0, 10.0),
        layer_fq: FullyQualifiedMoniker::from_string("/L"),
        parent_zone: None,
        last_focused: None,
        overrides: HashMap::new(),
    });

    reg.register_scope(FocusScope {
        fq: fq.clone(),
        segment: SegmentMoniker::from_string("k"),
        rect: rect(50.0, 50.0, 10.0, 10.0),
        layer_fq: FullyQualifiedMoniker::from_string("/L"),
        parent_zone: None,
        overrides: HashMap::new(),
    });

    assert!(reg.scope(&fq).is_some());
    assert!(reg.zone(&fq).is_none());
}

/// Symmetric: registering a leaf scope for an FQM previously registered
/// as a zone returns an error through the batch path.
#[test]
fn batch_register_scope_for_existing_zone_fq_errors() {
    let mut reg = SpatialRegistry::new();
    reg.apply_batch(vec![zone_entry(
        "/L/k",
        "k",
        "/L",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    )])
    .expect("first batch");

    let result = reg.apply_batch(vec![scope_entry(
        "/L/k",
        "k",
        "/L",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    )]);
    assert!(
        result.is_err(),
        "registering a leaf scope for a zone FQM must fail",
    );
}

// ---------------------------------------------------------------------------
// Atomic-rollback contract
// ---------------------------------------------------------------------------

/// `apply_batch` validates the entire input vector before mutating the
/// registry — when *any* entry fails the kind-stability check, **none**
/// of the entries in the batch are applied.
#[test]
fn apply_batch_rolls_back_when_any_entry_fails_kind_check() {
    let mut reg = SpatialRegistry::new();
    let baseline_rect = rect(0.0, 0.0, 10.0, 10.0);
    reg.apply_batch(vec![scope_entry(
        "/L/collide",
        "collide",
        "/L",
        None,
        baseline_rect,
    )])
    .expect("seed batch");

    let bumped_rect = rect(99.0, 99.0, 5.0, 5.0);
    let result = reg.apply_batch(vec![
        scope_entry("/L/first-good", "first-good", "/L", None, bumped_rect),
        zone_entry("/L/collide", "collide", "/L", None, bumped_rect),
        scope_entry("/L/trailing-good", "trailing-good", "/L", None, bumped_rect),
    ]);

    match result {
        Err(BatchRegisterError::KindMismatch {
            fq,
            existing_kind,
            requested_kind,
        }) => {
            assert_eq!(fq, FullyQualifiedMoniker::from_string("/L/collide"));
            assert_eq!(existing_kind, ScopeKind::Scope);
            assert_eq!(requested_kind, ScopeKind::Zone);
        }
        other => panic!("expected KindMismatch error, got {other:?}"),
    }

    assert!(!reg.is_registered(&FullyQualifiedMoniker::from_string("/L/first-good")),);
    assert!(!reg.is_registered(&FullyQualifiedMoniker::from_string("/L/trailing-good")),);

    let collide = reg
        .scope(&FullyQualifiedMoniker::from_string("/L/collide"))
        .expect("pre-existing leaf still registered");
    assert_eq!(collide.rect.left(), baseline_rect.left());
    assert_eq!(collide.rect.top(), baseline_rect.top());
}

// ---------------------------------------------------------------------------
// last_focused preservation across re-registration
// ---------------------------------------------------------------------------

/// `apply_batch` preserves an existing zone's `last_focused` slot when
/// the same FQM is re-registered as a zone.
#[test]
fn apply_batch_preserves_zone_last_focused_across_re_registration() {
    let mut reg = SpatialRegistry::new();
    let zone_fq = FullyQualifiedMoniker::from_string("/L/z");
    let remembered = FullyQualifiedMoniker::from_string("/L/z/remembered");

    reg.register_zone(FocusZone {
        fq: zone_fq.clone(),
        segment: SegmentMoniker::from_string("z"),
        rect: rect(0.0, 0.0, 100.0, 100.0),
        layer_fq: FullyQualifiedMoniker::from_string("/L"),
        parent_zone: None,
        last_focused: Some(remembered.clone()),
        overrides: HashMap::new(),
    });

    reg.apply_batch(vec![zone_entry(
        "/L/z",
        "z",
        "/L",
        None,
        rect(50.0, 50.0, 80.0, 80.0),
    )])
    .expect("re-register batch succeeds");

    let zone = reg
        .zone(&zone_fq)
        .expect("zone still registered after batch");
    assert_eq!(
        zone.last_focused,
        Some(remembered),
        "drill-out memory must survive the placeholder→real-mount swap",
    );
    assert_eq!(zone.rect.left(), Pixels::new(50.0));
    assert_eq!(zone.rect.top(), Pixels::new(50.0));
}
