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
//!   discriminator (`"focusable"` / `"zone"`) and `snake_case` rename to
//!   match the rest of the kernel's enums.
//! - **All fields newtyped** — `key`, `moniker`, `rect`, `layer_key`,
//!   `parent_zone`, `overrides` use the existing newtypes; no bare
//!   `String` or `f64` on the wire.
//! - **Atomic application** — `SpatialRegistry::apply_batch` registers
//!   N entries in one pass without splitting across multiple calls.
//! - **Idempotent on key** — a focusable registered twice with the same
//!   key keeps the focusable variant; subsequent rect overwrites the
//!   prior value.
//! - **Kind mismatch is an error** — registering a zone for a key that
//!   was previously a focusable (or vice versa) is rejected so a
//!   placeholder/real-mount swap can't silently change the variant.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    BatchRegisterError, FocusScope, FocusZone, Focusable, LayerKey, Moniker, Pixels, Rect,
    RegisterEntry, ScopeKind, SpatialKey, SpatialRegistry,
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

fn focusable_entry(
    key: &str,
    moniker: &str,
    layer: &str,
    parent_zone: Option<&str>,
    r: Rect,
) -> RegisterEntry {
    RegisterEntry::Focusable {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect: r,
        layer_key: LayerKey::from_string(layer),
        parent_zone: parent_zone.map(SpatialKey::from_string),
        overrides: HashMap::new(),
    }
}

fn zone_entry(
    key: &str,
    moniker: &str,
    layer: &str,
    parent_zone: Option<&str>,
    r: Rect,
) -> RegisterEntry {
    RegisterEntry::Zone {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect: r,
        layer_key: LayerKey::from_string(layer),
        parent_zone: parent_zone.map(SpatialKey::from_string),
        overrides: HashMap::new(),
    }
}

// ---------------------------------------------------------------------------
// Wire shape
// ---------------------------------------------------------------------------

/// `RegisterEntry::Focusable` round-trips through JSON with a `"kind"
/// = "focusable"` tag and snake-cased field names.
#[test]
fn register_entry_focusable_serializes_with_kind_tag() {
    let entry = focusable_entry("k", "ui:k", "L", Some("zone"), rect(1.0, 2.0, 3.0, 4.0));
    let json = serde_json::to_value(&entry).expect("serialize");
    assert_eq!(json["kind"], "focusable");
    assert_eq!(json["key"], "k");
    assert_eq!(json["moniker"], "ui:k");
    assert_eq!(json["layer_key"], "L");
    assert_eq!(json["parent_zone"], "zone");
}

/// `RegisterEntry::Zone` round-trips through JSON with a `"kind" =
/// "zone"` tag.
#[test]
fn register_entry_zone_serializes_with_kind_tag() {
    let entry = zone_entry("z", "ui:z", "L", None, rect(0.0, 0.0, 10.0, 10.0));
    let json = serde_json::to_value(&entry).expect("serialize");
    assert_eq!(json["kind"], "zone");
    assert_eq!(json["key"], "z");
    assert_eq!(json["parent_zone"], serde_json::Value::Null);
}

/// `RegisterEntry` deserializes from the same wire shape — the React
/// virtualizer constructs a `Vec<RegisterEntry>` and ships it through
/// the Tauri IPC, where serde matches on the `kind` tag.
#[test]
fn register_entry_deserializes_focusable_via_kind_tag() {
    let json = serde_json::json!({
        "kind": "focusable",
        "key": "k",
        "moniker": "ui:k",
        "rect": { "x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0 },
        "layer_key": "L",
        "parent_zone": null,
        "overrides": {}
    });
    let entry: RegisterEntry = serde_json::from_value(json).expect("deserialize");
    match entry {
        RegisterEntry::Focusable { key, moniker, .. } => {
            assert_eq!(key, SpatialKey::from_string("k"));
            assert_eq!(moniker, Moniker::from_string("ui:k"));
        }
        other => panic!("expected Focusable, got {other:?}"),
    }
}

/// `RegisterEntry` deserializes a zone entry the same way.
#[test]
fn register_entry_deserializes_zone_via_kind_tag() {
    let json = serde_json::json!({
        "kind": "zone",
        "key": "z",
        "moniker": "ui:z",
        "rect": { "x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0 },
        "layer_key": "L",
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
/// the test exercises the contract by handing it a mixed
/// focusable/zone vector and verifying every key resolves afterward.
#[test]
fn apply_batch_registers_all_entries() {
    let mut reg = SpatialRegistry::new();
    let entries = vec![
        zone_entry("z1", "ui:z1", "L", None, rect(0.0, 0.0, 100.0, 100.0)),
        focusable_entry("f1", "ui:f1", "L", Some("z1"), rect(0.0, 0.0, 10.0, 10.0)),
        focusable_entry("f2", "ui:f2", "L", Some("z1"), rect(20.0, 0.0, 10.0, 10.0)),
    ];

    reg.apply_batch(entries).expect("batch apply succeeds");

    assert!(reg.scope(&SpatialKey::from_string("z1")).is_some());
    assert!(reg.scope(&SpatialKey::from_string("f1")).is_some());
    assert!(reg.scope(&SpatialKey::from_string("f2")).is_some());
}

// ---------------------------------------------------------------------------
// Idempotent on key
// ---------------------------------------------------------------------------

/// Registering the same focusable key twice with a different rect
/// overwrites the rect and keeps the `Focusable` variant.
#[test]
fn re_register_same_focusable_key_overwrites_rect_keeps_variant() {
    let mut reg = SpatialRegistry::new();
    reg.apply_batch(vec![focusable_entry(
        "k",
        "ui:k",
        "L",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    )])
    .expect("first batch");

    reg.apply_batch(vec![focusable_entry(
        "k",
        "ui:k",
        "L",
        None,
        rect(50.0, 50.0, 20.0, 20.0),
    )])
    .expect("second batch");

    let scope = reg
        .scope(&SpatialKey::from_string("k"))
        .expect("key still registered");
    assert!(matches!(scope, FocusScope::Focusable(_)));
    let r = scope.rect();
    assert_eq!(r.left(), Pixels::new(50.0));
    assert_eq!(r.top(), Pixels::new(50.0));
    assert_eq!(r.right(), Pixels::new(70.0));
    assert_eq!(r.bottom(), Pixels::new(70.0));
}

// ---------------------------------------------------------------------------
// Kind mismatch
// ---------------------------------------------------------------------------

/// Registering a zone for a key previously registered as a focusable
/// returns an error — the kind on a stable `SpatialKey` must not change
/// between the placeholder mount and the real-mount swap.
#[test]
fn batch_register_zone_for_existing_focusable_key_errors() {
    let mut reg = SpatialRegistry::new();
    reg.apply_batch(vec![focusable_entry(
        "k",
        "ui:k",
        "L",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    )])
    .expect("first batch");

    let result = reg.apply_batch(vec![zone_entry(
        "k",
        "ui:k",
        "L",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    )]);
    assert!(
        result.is_err(),
        "registering a zone for a focusable key must fail",
    );
}

// ---------------------------------------------------------------------------
// Single-entry idempotency (the placeholder/real-mount swap path)
// ---------------------------------------------------------------------------

/// `register_focusable` called with the same `SpatialKey` twice
/// overwrites the rect on the existing entry while keeping the
/// variant unchanged. This is the contract the placeholder/real-mount
/// swap depends on: the virtualizer registers a placeholder under
/// `SpatialKey("k")`, the row scrolls into view, the real
/// `<Focusable>` mounts and re-registers under the same key with the
/// freshly-measured rect — the registry overwrites, no churn.
#[test]
fn register_focusable_twice_overwrites_rect_keeps_variant() {
    let mut reg = SpatialRegistry::new();
    let key = SpatialKey::from_string("k");

    reg.register_focusable(Focusable {
        key: key.clone(),
        moniker: Moniker::from_string("ui:k"),
        rect: rect(0.0, 0.0, 10.0, 10.0),
        layer_key: LayerKey::from_string("L"),
        parent_zone: None,
        overrides: HashMap::new(),
    });

    reg.register_focusable(Focusable {
        key: key.clone(),
        moniker: Moniker::from_string("ui:k"),
        rect: rect(50.0, 50.0, 20.0, 20.0),
        layer_key: LayerKey::from_string("L"),
        parent_zone: None,
        overrides: HashMap::new(),
    });

    let scope = reg.scope(&key).expect("key still registered");
    assert!(matches!(scope, FocusScope::Focusable(_)));
    assert_eq!(scope.rect().left(), Pixels::new(50.0));
    assert_eq!(scope.rect().top(), Pixels::new(50.0));
}

/// `register_focusable` called for a key currently registered as a
/// `Zone` — the standalone `register_*` methods overwrite without
/// validating the kind. The kind-stability check lives on the
/// **batch** entry point (where the placeholder/real-mount swap goes
/// through), because that's the only path React uses to register
/// many entries at once and the place where a kind mismatch is most
/// likely a bug worth surfacing rather than silently coercing.
///
/// Documenting the asymmetry as a test so future contributors don't
/// reflexively add a `panic!` to `register_focusable` and break hot-
/// reload semantics.
#[test]
fn register_focusable_overwrites_existing_zone_silently() {
    let mut reg = SpatialRegistry::new();
    let key = SpatialKey::from_string("k");

    reg.register_zone(FocusZone {
        key: key.clone(),
        moniker: Moniker::from_string("ui:k"),
        rect: rect(0.0, 0.0, 10.0, 10.0),
        layer_key: LayerKey::from_string("L"),
        parent_zone: None,
        last_focused: None,
        overrides: HashMap::new(),
    });

    reg.register_focusable(Focusable {
        key: key.clone(),
        moniker: Moniker::from_string("ui:k"),
        rect: rect(50.0, 50.0, 10.0, 10.0),
        layer_key: LayerKey::from_string("L"),
        parent_zone: None,
        overrides: HashMap::new(),
    });

    let scope = reg.scope(&key).expect("key still registered");
    assert!(scope.is_focusable());
}

/// Symmetric: registering a focusable for a key previously registered
/// as a zone returns an error.
#[test]
fn batch_register_focusable_for_existing_zone_key_errors() {
    let mut reg = SpatialRegistry::new();
    reg.apply_batch(vec![zone_entry(
        "k",
        "ui:k",
        "L",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    )])
    .expect("first batch");

    let result = reg.apply_batch(vec![focusable_entry(
        "k",
        "ui:k",
        "L",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    )]);
    assert!(
        result.is_err(),
        "registering a focusable for a zone key must fail",
    );
}

// ---------------------------------------------------------------------------
// Atomic-rollback contract
// ---------------------------------------------------------------------------

/// `apply_batch` validates the entire input vector before mutating the
/// registry — when *any* entry fails the kind-stability check, **none**
/// of the entries in the batch are applied. This guards against a
/// half-applied registration where the early entries land but a later
/// kind mismatch leaves the registry in a torn state.
///
/// The batch placed before the bad entry would otherwise overwrite a
/// previously-registered focusable's rect (a benign update) — asserting
/// the rect didn't move is what proves the rollback was atomic.
#[test]
fn apply_batch_rolls_back_when_any_entry_fails_kind_check() {
    let mut reg = SpatialRegistry::new();
    let baseline_rect = rect(0.0, 0.0, 10.0, 10.0);
    // Pre-existing focusable that the bad zone entry will collide with.
    reg.apply_batch(vec![focusable_entry(
        "collide",
        "ui:collide",
        "L",
        None,
        baseline_rect,
    )])
    .expect("seed batch");

    // Batch order: a brand-new focusable (would succeed), the bad zone
    // entry that flips a focusable into a zone (must fail), then a
    // recovery focusable (would also succeed). Atomic semantics demand
    // that *neither* the leading nor the trailing good entry is applied
    // and that `collide`'s rect stays at the baseline.
    let bumped_rect = rect(99.0, 99.0, 5.0, 5.0);
    let result = reg.apply_batch(vec![
        focusable_entry("first-good", "ui:first-good", "L", None, bumped_rect),
        zone_entry("collide", "ui:collide", "L", None, bumped_rect),
        focusable_entry("trailing-good", "ui:trailing-good", "L", None, bumped_rect),
    ]);

    match result {
        Err(BatchRegisterError::KindMismatch {
            key,
            existing_kind,
            requested_kind,
        }) => {
            assert_eq!(key, SpatialKey::from_string("collide"));
            assert_eq!(existing_kind, ScopeKind::Focusable);
            assert_eq!(requested_kind, ScopeKind::Zone);
        }
        other => panic!("expected KindMismatch error, got {other:?}"),
    }

    // Neither of the would-be-good entries landed.
    assert!(
        reg.scope(&SpatialKey::from_string("first-good")).is_none(),
        "leading good entry must not be applied when a later entry fails",
    );
    assert!(
        reg.scope(&SpatialKey::from_string("trailing-good"))
            .is_none(),
        "trailing good entry must not be applied when an earlier entry fails",
    );

    // The pre-existing entry's rect is unchanged — no half-applied
    // overwrite leaked through the rollback.
    let collide = reg
        .scope(&SpatialKey::from_string("collide"))
        .expect("pre-existing scope still registered");
    assert!(matches!(collide, FocusScope::Focusable(_)));
    assert_eq!(collide.rect().left(), baseline_rect.left());
    assert_eq!(collide.rect().top(), baseline_rect.top());
    assert_eq!(collide.rect().right(), baseline_rect.right());
    assert_eq!(collide.rect().bottom(), baseline_rect.bottom());
}

// ---------------------------------------------------------------------------
// last_focused preservation across re-registration
// ---------------------------------------------------------------------------

/// `apply_batch` preserves an existing zone's `last_focused` slot when
/// the same `SpatialKey` is re-registered as a zone (the
/// placeholder→real-mount swap path). The wire shape of `RegisterEntry`
/// intentionally omits `last_focused` because that field is server-
/// owned drill-out memory; carrying it on the wire would let a sloppy
/// client clobber it on every virtualizer pass.
///
/// This test seeds a zone, mutates its `last_focused` directly through
/// `register_zone`, then re-applies a batch that re-registers the same
/// key as a zone (with no `last_focused` on the wire) and asserts the
/// memory survived the swap.
#[test]
fn apply_batch_preserves_zone_last_focused_across_re_registration() {
    let mut reg = SpatialRegistry::new();
    let zone_key = SpatialKey::from_string("z");
    let remembered = SpatialKey::from_string("remembered-leaf");

    // Seed: zone with an explicit `last_focused` slot (would normally be
    // populated by the navigator as focus moves through the zone).
    reg.register_zone(FocusZone {
        key: zone_key.clone(),
        moniker: Moniker::from_string("ui:z"),
        rect: rect(0.0, 0.0, 100.0, 100.0),
        layer_key: LayerKey::from_string("L"),
        parent_zone: None,
        last_focused: Some(remembered.clone()),
        overrides: HashMap::new(),
    });

    // Re-register through the batch path with a fresh rect (the rect
    // overwrite is the placeholder→real-mount swap's whole point).
    reg.apply_batch(vec![zone_entry(
        "z",
        "ui:z",
        "L",
        None,
        rect(50.0, 50.0, 80.0, 80.0),
    )])
    .expect("re-register batch succeeds");

    let scope = reg
        .scope(&zone_key)
        .expect("zone still registered after batch");
    let zone = scope.as_zone().expect("scope is still a zone");
    assert_eq!(
        zone.last_focused,
        Some(remembered),
        "drill-out memory must survive the placeholder→real-mount swap",
    );
    // Sanity: the rect moved to the new value, proving the re-register
    // actually replaced the entry rather than no-oping.
    assert_eq!(zone.rect.left(), Pixels::new(50.0));
    assert_eq!(zone.rect.top(), Pixels::new(50.0));
}
