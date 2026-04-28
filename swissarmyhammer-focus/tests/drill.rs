//! Integration tests for `SpatialRegistry::drill_in` and
//! `SpatialRegistry::drill_out`.
//!
//! Drill-in and drill-out are the explicit zone-descent / zone-ascent
//! commands that complement the cardinal beam-search nav. The methods
//! are pure registry queries — they take a [`SpatialKey`] paired with
//! a focused [`Moniker`] and return a non-optional [`Moniker`] under
//! the no-silent-dropout contract documented in
//! [`swissarmyhammer_focus::navigate`]. The Tauri adapter layer
//! (`spatial_drill_in` / `spatial_drill_out`) decides what to do with
//! the returned moniker — typically `setFocus(result)` and a fall-
//! through to edit / dismiss when `result == focused_moniker`.
//!
//! Coverage matches the acceptance criteria on the drill-in/out card:
//!
//! - `drill_in` on a Zone with a registered `last_focused` returns
//!   that entry's [`Moniker`].
//! - `drill_in` on a Zone whose `last_focused` is stale (the stored
//!   key no longer resolves to a registered scope) falls back to the
//!   first child by rect top-left ordering.
//! - `drill_in` on a Zone with no `last_focused` returns the first
//!   child.
//! - `drill_in` on a Zone with no children returns `focused_moniker`.
//! - `drill_in` on a FocusScope returns `focused_moniker` (React
//!   handles inline edit).
//! - `drill_in` on an unknown key returns `focused_moniker` AND emits
//!   `tracing::error!` (the trace is asserted in
//!   `tests/no_silent_none.rs`).
//! - `drill_out` on a FocusScope returns its `parent_zone`'s
//!   [`Moniker`].
//! - `drill_out` on a Zone returns its `parent_zone`'s [`Moniker`].
//! - `drill_out` at the layer root (no `parent_zone`) returns
//!   `focused_moniker`.
//! - `drill_out` on an unknown key returns `focused_moniker` AND emits
//!   `tracing::error!`.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    FocusScope, FocusZone, LayerKey, Moniker, Pixels, Rect, SpatialKey, SpatialRegistry,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a [`Rect`] at `(x, y)` with a unit width/height.
///
/// Used by the drill-in fallback test to give children distinct
/// top-left coordinates so the deterministic ordering is observable.
fn rect_at(x: f64, y: f64) -> Rect {
    Rect {
        x: Pixels::new(x),
        y: Pixels::new(y),
        width: Pixels::new(10.0),
        height: Pixels::new(10.0),
    }
}

/// Construct a [`FocusScope`] leaf at the given rect with no overrides.
fn leaf(
    key: &str,
    moniker: &str,
    layer: &str,
    parent_zone: Option<&str>,
    rect: Rect,
) -> FocusScope {
    FocusScope {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect,
        layer_key: LayerKey::from_string(layer),
        parent_zone: parent_zone.map(SpatialKey::from_string),
        overrides: HashMap::new(),
    }
}

/// Construct a [`FocusZone`] with optional `last_focused` and parent.
fn zone(
    key: &str,
    moniker: &str,
    layer: &str,
    parent_zone: Option<&str>,
    last_focused: Option<&str>,
) -> FocusZone {
    FocusZone {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect: rect_at(0.0, 0.0),
        layer_key: LayerKey::from_string(layer),
        parent_zone: parent_zone.map(SpatialKey::from_string),
        last_focused: last_focused.map(SpatialKey::from_string),
        overrides: HashMap::new(),
    }
}

// ---------------------------------------------------------------------------
// drill_in — Zone with live last_focused
// ---------------------------------------------------------------------------

/// Drill-in on a zone whose `last_focused` still resolves to a registered
/// scope returns that scope's moniker — preserves the user's last position
/// inside the zone across drill-out / drill-in cycles.
#[test]
fn drill_in_zone_with_live_last_focused_returns_remembered_moniker() {
    let mut reg = SpatialRegistry::new();
    reg.register_zone(zone("z", "ui:zone", "L", None, Some("leaf-b")));
    reg.register_scope(leaf(
        "leaf-a",
        "ui:leaf-a",
        "L",
        Some("z"),
        rect_at(0.0, 0.0),
    ));
    reg.register_scope(leaf(
        "leaf-b",
        "ui:leaf-b",
        "L",
        Some("z"),
        rect_at(20.0, 20.0),
    ));

    let zone_moniker = Moniker::from_string("ui:zone");
    let target = reg.drill_in(SpatialKey::from_string("z"), &zone_moniker);
    assert_eq!(target, Moniker::from_string("ui:leaf-b"));
}

// ---------------------------------------------------------------------------
// drill_in — Zone with stale last_focused
// ---------------------------------------------------------------------------

/// When `last_focused` points at a scope that has since been unregistered
/// (e.g. a card was deleted while focus was elsewhere), drill-in falls back
/// to the first child ordered by rect top-left.
#[test]
fn drill_in_zone_with_stale_last_focused_falls_back_to_first_child() {
    let mut reg = SpatialRegistry::new();
    reg.register_zone(zone("z", "ui:zone", "L", None, Some("ghost")));
    // leaf-a is at (10, 0), leaf-b is at (0, 0). Top-left ordering ranks
    // by (top, left) — leaf-b (top=0, left=0) wins over leaf-a (top=0,
    // left=10).
    reg.register_scope(leaf(
        "leaf-a",
        "ui:leaf-a",
        "L",
        Some("z"),
        rect_at(10.0, 0.0),
    ));
    reg.register_scope(leaf(
        "leaf-b",
        "ui:leaf-b",
        "L",
        Some("z"),
        rect_at(0.0, 0.0),
    ));

    let zone_moniker = Moniker::from_string("ui:zone");
    let target = reg.drill_in(SpatialKey::from_string("z"), &zone_moniker);
    assert_eq!(target, Moniker::from_string("ui:leaf-b"));
}

// ---------------------------------------------------------------------------
// drill_in — Zone with no last_focused
// ---------------------------------------------------------------------------

/// A zone with no `last_focused` at all (cold-start, no prior visit) drills
/// into its first child by rect top-left.
#[test]
fn drill_in_zone_with_no_last_focused_uses_first_child_by_rect() {
    let mut reg = SpatialRegistry::new();
    reg.register_zone(zone("z", "ui:zone", "L", None, None));
    // Two children: one at (5, 5), one at (5, 0). Top-left ordering ranks
    // (top=0, left=5) over (top=5, left=5) since `top` is the primary key.
    reg.register_scope(leaf(
        "leaf-bottom",
        "ui:leaf-bottom",
        "L",
        Some("z"),
        rect_at(5.0, 5.0),
    ));
    reg.register_scope(leaf(
        "leaf-top",
        "ui:leaf-top",
        "L",
        Some("z"),
        rect_at(5.0, 0.0),
    ));

    let zone_moniker = Moniker::from_string("ui:zone");
    let target = reg.drill_in(SpatialKey::from_string("z"), &zone_moniker);
    assert_eq!(target, Moniker::from_string("ui:leaf-top"));
}

// ---------------------------------------------------------------------------
// drill_in — Zone with no children
// ---------------------------------------------------------------------------

/// An empty zone — no `last_focused`, no children registered yet — returns
/// the focused moniker. The frontend interprets the equality with the prior
/// focus as "stay where you are" and falls through to onEdit / no-op.
#[test]
fn drill_in_empty_zone_returns_focused_moniker() {
    let mut reg = SpatialRegistry::new();
    reg.register_zone(zone("z", "ui:zone", "L", None, None));

    let zone_moniker = Moniker::from_string("ui:zone");
    let target = reg.drill_in(SpatialKey::from_string("z"), &zone_moniker);
    assert_eq!(target, zone_moniker);
}

// ---------------------------------------------------------------------------
// drill_in — FocusScope
// ---------------------------------------------------------------------------

/// Drill-in on a leaf returns the focused moniker — leaves do not have
/// children. The React side detects the equality and falls through to
/// the leaf's inline-edit affordance (or no-op for non-editable leaves).
#[test]
fn drill_in_focusable_returns_focused_moniker() {
    let mut reg = SpatialRegistry::new();
    reg.register_scope(leaf("leaf", "ui:leaf", "L", None, rect_at(0.0, 0.0)));

    let leaf_moniker = Moniker::from_string("ui:leaf");
    let target = reg.drill_in(SpatialKey::from_string("leaf"), &leaf_moniker);
    assert_eq!(target, leaf_moniker);
}

// ---------------------------------------------------------------------------
// drill_in — Unknown key
// ---------------------------------------------------------------------------

/// Drill-in for an unknown key echoes the input focused moniker. The
/// kernel emits `tracing::error!` (verified in
/// `tests/no_silent_none.rs`); the React side's user-visible behavior
/// matches the no-children case (focus stays put).
#[test]
fn drill_in_unknown_key_echoes_focused_moniker() {
    let reg = SpatialRegistry::new();
    let focused_moniker = Moniker::from_string("ui:focused");
    let target = reg.drill_in(SpatialKey::from_string("ghost"), &focused_moniker);
    assert_eq!(target, focused_moniker);
}

// ---------------------------------------------------------------------------
// drill_out — FocusScope to its parent zone
// ---------------------------------------------------------------------------

/// Drill-out on a leaf returns the moniker of its enclosing zone.
#[test]
fn drill_out_focusable_returns_parent_zone_moniker() {
    let mut reg = SpatialRegistry::new();
    reg.register_zone(zone("z", "ui:zone", "L", None, None));
    reg.register_scope(leaf("leaf", "ui:leaf", "L", Some("z"), rect_at(0.0, 0.0)));

    let leaf_moniker = Moniker::from_string("ui:leaf");
    let target = reg.drill_out(SpatialKey::from_string("leaf"), &leaf_moniker);
    assert_eq!(target, Moniker::from_string("ui:zone"));
}

// ---------------------------------------------------------------------------
// drill_out — Zone to its parent zone
// ---------------------------------------------------------------------------

/// Drill-out on a zone returns its enclosing zone's moniker — zones nest,
/// so `drill_out` on an inner zone moves focus to the outer one.
#[test]
fn drill_out_zone_returns_parent_zone_moniker() {
    let mut reg = SpatialRegistry::new();
    reg.register_zone(zone("outer", "ui:outer", "L", None, None));
    reg.register_zone(zone("inner", "ui:inner", "L", Some("outer"), None));

    let inner_moniker = Moniker::from_string("ui:inner");
    let target = reg.drill_out(SpatialKey::from_string("inner"), &inner_moniker);
    assert_eq!(target, Moniker::from_string("ui:outer"));
}

// ---------------------------------------------------------------------------
// drill_out — Layer-root scope (no parent_zone)
// ---------------------------------------------------------------------------

/// Drill-out on a scope that has no `parent_zone` (sits directly under the
/// layer root) returns the focused moniker (semantic "stay put"). The
/// frontend detects the equality and falls through to the next command in
/// the Escape chain (typically `app.dismiss`).
#[test]
fn drill_out_at_layer_root_returns_focused_moniker() {
    let mut reg = SpatialRegistry::new();
    reg.register_scope(leaf("leaf", "ui:leaf", "L", None, rect_at(0.0, 0.0)));

    let leaf_moniker = Moniker::from_string("ui:leaf");
    let target = reg.drill_out(SpatialKey::from_string("leaf"), &leaf_moniker);
    assert_eq!(target, leaf_moniker);
}

// ---------------------------------------------------------------------------
// drill_out — Unknown key
// ---------------------------------------------------------------------------

/// Drill-out for an unknown key echoes the input focused moniker. The
/// kernel emits `tracing::error!` (verified in
/// `tests/no_silent_none.rs`); same fall-through semantics as the
/// well-formed layer-root case from the React side's perspective.
#[test]
fn drill_out_unknown_key_echoes_focused_moniker() {
    let reg = SpatialRegistry::new();
    let focused_moniker = Moniker::from_string("ui:focused");
    let target = reg.drill_out(SpatialKey::from_string("ghost"), &focused_moniker);
    assert_eq!(target, focused_moniker);
}

// ---------------------------------------------------------------------------
// Round-trip — drill_out → drill_in returns to last_focused
// ---------------------------------------------------------------------------

/// When the navigator updates a zone's `last_focused` as focus moves
/// inside, a subsequent `drill_in` on that zone returns the remembered
/// position. This test wires `last_focused` directly (the navigator is
/// covered by its own tests); the contract being asserted here is that
/// drill-in honors the stored slot.
#[test]
fn drill_in_after_remembered_position_returns_remembered_moniker() {
    let mut reg = SpatialRegistry::new();
    // Pre-populate `last_focused` so the test does not depend on the
    // navigator's update path.
    reg.register_zone(zone("z", "ui:zone", "L", None, Some("leaf-a")));
    reg.register_scope(leaf(
        "leaf-a",
        "ui:leaf-a",
        "L",
        Some("z"),
        rect_at(0.0, 0.0),
    ));
    reg.register_scope(leaf(
        "leaf-b",
        "ui:leaf-b",
        "L",
        Some("z"),
        rect_at(20.0, 20.0),
    ));

    let zone_moniker = Moniker::from_string("ui:zone");
    let target = reg.drill_in(SpatialKey::from_string("z"), &zone_moniker);
    assert_eq!(
        target,
        Moniker::from_string("ui:leaf-a"),
        "drill-in honors the remembered slot, even when other children exist",
    );
}

// ---------------------------------------------------------------------------
// drill_in — Field zone with pill children
//
// Pin the inspector-specific contract from card
// `01KQ9ZJHRXCY8Z5YT6RF4SG6EK` (bug 3): a field zone whose pill scopes
// are laid out left-to-right (e.g. a `tags` field rendering badge
// pills) drills in to the leftmost pill. The pills' monikers follow
// the entity convention `tag:<id>` rather than the synthetic `ui:`
// monikers above, but the geometric ordering rule is the same — the
// first child by rect (top, left) wins when no `last_focused` is set.
// ---------------------------------------------------------------------------

/// A field zone whose pill children are laid out horizontally returns the
/// leftmost pill's moniker on drill-in. Pins the inspector contract that
/// "Enter on a focused pill field drills into the first pill" maps to a
/// kernel response that the React side can dispatch `setFocus` against.
#[test]
fn drill_in_field_zone_with_pill_children_returns_first_pill_moniker() {
    let mut reg = SpatialRegistry::new();

    // The field zone (`field:task:T1.tags`) sits inside the inspector
    // panel zone in production; here we anchor it directly in the
    // layer for simplicity. The contract under test is the in-zone
    // ordering, which doesn't depend on the parent chain.
    reg.register_zone(zone(
        "field-tags-key",
        "field:task:T1.tags",
        "L",
        None,
        None,
    ));

    // Three pill scopes, horizontally progressing on the same row.
    // Top-left ordering ranks (top=0, left=0) before (top=0, left=10)
    // before (top=0, left=20) — `tag:tag-bug` wins.
    reg.register_scope(leaf(
        "pill-bug-key",
        "tag:tag-bug",
        "L",
        Some("field-tags-key"),
        rect_at(0.0, 0.0),
    ));
    reg.register_scope(leaf(
        "pill-ui-key",
        "tag:tag-ui",
        "L",
        Some("field-tags-key"),
        rect_at(10.0, 0.0),
    ));
    reg.register_scope(leaf(
        "pill-docs-key",
        "tag:tag-docs",
        "L",
        Some("field-tags-key"),
        rect_at(20.0, 0.0),
    ));

    let field_moniker = Moniker::from_string("field:task:T1.tags");
    let target = reg.drill_in(SpatialKey::from_string("field-tags-key"), &field_moniker);
    assert_eq!(
        target,
        Moniker::from_string("tag:tag-bug"),
        "drill-in on a tags field zone returns the leftmost pill's moniker",
    );
}

// ---------------------------------------------------------------------------
// drill_in — Field zone with no children
//
// Pin the "Enter falls through to edit mode" contract from card
// `01KQ9ZJHRXCY8Z5YT6RF4SG6EK` (bug 3): when a field zone has no pill
// children (e.g. an empty tags field, or an editable scalar like
// `name`), drill-in returns the field's own moniker. The React side
// detects the equality and falls through to `onEdit?.()` — opening the
// editor for editable fields, no-op for read-only ones.
// ---------------------------------------------------------------------------

/// A field zone with no spatial children — the canonical "scalar leaf
/// without a click-into structure" case — returns the focused moniker
/// on drill-in. React detects the equality (result == focused_moniker)
/// and falls through to `onEdit?.()`.
#[test]
fn drill_in_field_zone_with_no_children_returns_focused_moniker() {
    let mut reg = SpatialRegistry::new();

    // Register the field zone but no pill children — mirrors an
    // editable scalar field (e.g. `name`) or an empty pill field.
    reg.register_zone(zone(
        "field-name-key",
        "field:task:T1.name",
        "L",
        None,
        None,
    ));

    let field_moniker = Moniker::from_string("field:task:T1.name");
    let target = reg.drill_in(SpatialKey::from_string("field-name-key"), &field_moniker);
    assert_eq!(
        target, field_moniker,
        "drill-in on a childless field zone echoes the focused moniker — \
         React detects equality and falls through to onEdit",
    );
}
