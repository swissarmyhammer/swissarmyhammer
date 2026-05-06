//! Integration tests for `RegisterEntry` and the batch-register entry point.
//!
//! Headless pattern matching `tests/focus_registry.rs` — pure Rust, no
//! Tauri runtime. The `RegisterEntry` struct is the wire shape that the
//! Tauri `spatial_register_batch` command receives; these tests exercise
//! its serde round-trip and the registry-side application path that lets
//! a single lock register N entries atomically.
//!
//! Coverage:
//!
//! - **Wire shape** — `RegisterEntry` carries the same field set as
//!   [`FocusScope`] (minus `last_focused`, which is server-owned).
//! - **All fields newtyped** — `fq`, `segment`, `rect`, `layer_fq`,
//!   `parent_zone`, `overrides` use the existing newtypes; no bare
//!   `String` or `f64` on the wire.
//! - **Atomic application** — `SpatialRegistry::apply_batch` registers
//!   N entries in one pass without splitting across multiple calls.
//! - **Idempotent on FQM** — a scope registered twice with the same
//!   FQM keeps its place in the registry; subsequent rect overwrites
//!   the prior value.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    FullyQualifiedMoniker, Pixels, Rect, RegisterEntry, SegmentMoniker, SpatialRegistry,
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

fn entry(
    fq: &str,
    segment: &str,
    layer: &str,
    parent_zone: Option<&str>,
    r: Rect,
) -> RegisterEntry {
    RegisterEntry {
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

/// `RegisterEntry` round-trips through JSON with the kernel's standard
/// snake-cased field names. Because there is only one primitive type
/// now, no `kind` discriminator is carried on the wire.
#[test]
fn register_entry_serializes_with_flat_shape() {
    let entry = entry(
        "/L/k",
        "k",
        "/L",
        Some("/L/parent"),
        rect(1.0, 2.0, 3.0, 4.0),
    );
    let json = serde_json::to_value(&entry).expect("serialize");
    assert_eq!(json["fq"], "/L/k");
    assert_eq!(json["segment"], "k");
    assert_eq!(json["layer_fq"], "/L");
    assert_eq!(json["parent_zone"], "/L/parent");
    assert!(json.get("kind").is_none(), "no kind discriminator: {json}");
}

/// All [`Rect`] fields round-trip as bare numbers, mirroring the
/// `Pixels` newtype's `transparent` serde shape.
#[test]
fn register_entry_rect_serializes_numerically() {
    let entry = entry("/L/k", "k", "/L", None, rect(13.5, 0.0, 10.0, 10.0));
    let json = serde_json::to_value(&entry).expect("serialize");
    let r = &json["rect"];
    assert_eq!(r["x"], 13.5);
    assert_eq!(r["y"], 0.0);
    assert_eq!(r["width"], 10.0);
    assert_eq!(r["height"], 10.0);
}

/// `RegisterEntry` round-trips through `serde_json::to_string` and
/// `from_str` without losing any field.
#[test]
fn register_entry_round_trips() {
    let original = entry(
        "/L/k",
        "k",
        "/L",
        Some("/L/parent"),
        rect(1.0, 2.0, 3.0, 4.0),
    );
    let json = serde_json::to_string(&original).expect("serialize");
    let parsed: RegisterEntry = serde_json::from_str(&json).expect("parse");
    assert_eq!(original, parsed);
}

// ---------------------------------------------------------------------------
// apply_batch
// ---------------------------------------------------------------------------

/// `apply_batch` registers every entry under a single mutable borrow.
/// Subsequent registry queries return the entries.
#[test]
fn apply_batch_registers_each_entry() {
    let mut reg = SpatialRegistry::new();
    let entries = vec![
        entry("/L/a", "a", "/L", None, rect(0.0, 0.0, 10.0, 10.0)),
        entry("/L/b", "b", "/L", None, rect(20.0, 0.0, 10.0, 10.0)),
        entry("/L/c", "c", "/L", None, rect(40.0, 0.0, 10.0, 10.0)),
    ];
    reg.apply_batch(entries);

    for fq in ["/L/a", "/L/b", "/L/c"] {
        assert!(
            reg.find_by_fq(&FullyQualifiedMoniker::from_string(fq))
                .is_some(),
            "expected {fq} to be registered after apply_batch"
        );
    }
}

/// Registering the same FQM twice via `apply_batch` keeps the entry in
/// place; the second registration's rect is the one that survives
/// (placeholder→real-mount swap semantics).
#[test]
fn apply_batch_is_idempotent_on_fq_with_rect_refresh() {
    let mut reg = SpatialRegistry::new();
    reg.apply_batch(vec![entry(
        "/L/k",
        "k",
        "/L",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    )]);
    reg.apply_batch(vec![entry(
        "/L/k",
        "k",
        "/L",
        None,
        rect(100.0, 200.0, 50.0, 50.0),
    )]);

    let scope = reg
        .find_by_fq(&FullyQualifiedMoniker::from_string("/L/k"))
        .expect("scope is registered");
    assert_eq!(scope.rect.x, Pixels::new(100.0));
    assert_eq!(scope.rect.y, Pixels::new(200.0));
}
