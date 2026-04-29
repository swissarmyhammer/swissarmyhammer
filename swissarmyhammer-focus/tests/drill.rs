//! Integration tests for `SpatialRegistry::drill_in` and
//! `SpatialRegistry::drill_out`.
//!
//! Drill-in and drill-out are the explicit zone-descent / zone-ascent
//! commands that complement the cardinal beam-search nav. The methods
//! are pure registry queries — they take a [`FullyQualifiedMoniker`]
//! paired with a focused FQM and return a non-optional FQM under
//! the no-silent-dropout contract documented in
//! [`swissarmyhammer_focus::navigate`]. The Tauri adapter layer
//! (`spatial_drill_in` / `spatial_drill_out`) decides what to do with
//! the returned FQM — typically `setFocus(result)` and a fall-
//! through to edit / dismiss when `result == focused_fq`.
//!
//! Coverage matches the acceptance criteria on the drill-in/out card:
//!
//! - `drill_in` on a Zone with a registered `last_focused` returns
//!   that entry's FQM.
//! - `drill_in` on a Zone whose `last_focused` is stale (the stored
//!   FQM no longer resolves to a registered scope) falls back to the
//!   first child by rect top-left ordering.
//! - `drill_in` on a Zone with no `last_focused` returns the first
//!   child.
//! - `drill_in` on a Zone with no children returns `focused_fq`.
//! - `drill_in` on a FocusScope returns `focused_fq` (React
//!   handles inline edit).
//! - `drill_in` on an unknown FQM returns `focused_fq` AND emits
//!   `tracing::error!` (the trace is asserted in
//!   `tests/no_silent_none.rs`).
//! - `drill_out` on a FocusScope returns its `parent_zone`'s FQM.
//! - `drill_out` on a Zone returns its `parent_zone`'s FQM.
//! - `drill_out` at the layer root (no `parent_zone`) returns
//!   `focused_fq`.
//! - `drill_out` on an unknown FQM returns `focused_fq` AND emits
//!   `tracing::error!`.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    FocusScope, FocusZone, FullyQualifiedMoniker, Pixels, Rect, SegmentMoniker, SpatialRegistry,
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

/// Compose an FQM under the layer at `layer_path` with the given
/// segment as the leaf.
fn fq_in_layer(layer_path: &str, segment: &str) -> FullyQualifiedMoniker {
    FullyQualifiedMoniker::from_string(format!("{layer_path}/{segment}"))
}

/// Construct a [`FocusScope`] leaf at the given rect with no overrides.
fn leaf(
    fq: FullyQualifiedMoniker,
    segment: &str,
    layer: &str,
    parent_zone: Option<FullyQualifiedMoniker>,
    rect: Rect,
) -> FocusScope {
    FocusScope {
        fq,
        segment: SegmentMoniker::from_string(segment),
        rect,
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone,
        overrides: HashMap::new(),
    }
}

/// Construct a [`FocusZone`] with optional `last_focused` and parent.
fn zone(
    fq: FullyQualifiedMoniker,
    segment: &str,
    layer: &str,
    parent_zone: Option<FullyQualifiedMoniker>,
    last_focused: Option<FullyQualifiedMoniker>,
) -> FocusZone {
    FocusZone {
        fq,
        segment: SegmentMoniker::from_string(segment),
        rect: rect_at(0.0, 0.0),
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone,
        last_focused,
        overrides: HashMap::new(),
    }
}

// ---------------------------------------------------------------------------
// drill_in — Zone with live last_focused
// ---------------------------------------------------------------------------

/// Drill-in on a zone whose `last_focused` still resolves to a registered
/// scope returns that scope's FQM — preserves the user's last position
/// inside the zone across drill-out / drill-in cycles.
#[test]
fn drill_in_zone_with_live_last_focused_returns_remembered_fq() {
    let mut reg = SpatialRegistry::new();
    let zone_fq = fq_in_layer("/L", "ui:zone");
    let leaf_a_fq =
        FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:leaf-a"));
    let leaf_b_fq =
        FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:leaf-b"));
    reg.register_zone(zone(
        zone_fq.clone(),
        "ui:zone",
        "/L",
        None,
        Some(leaf_b_fq.clone()),
    ));
    reg.register_scope(leaf(
        leaf_a_fq,
        "ui:leaf-a",
        "/L",
        Some(zone_fq.clone()),
        rect_at(0.0, 0.0),
    ));
    reg.register_scope(leaf(
        leaf_b_fq.clone(),
        "ui:leaf-b",
        "/L",
        Some(zone_fq.clone()),
        rect_at(20.0, 20.0),
    ));

    let target = reg.drill_in(zone_fq.clone(), &zone_fq);
    assert_eq!(target, leaf_b_fq);
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
    let zone_fq = fq_in_layer("/L", "ui:zone");
    let leaf_a_fq =
        FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:leaf-a"));
    let leaf_b_fq =
        FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:leaf-b"));
    let ghost_fq = FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ghost"));
    reg.register_zone(zone(zone_fq.clone(), "ui:zone", "/L", None, Some(ghost_fq)));
    // leaf-a is at (10, 0), leaf-b is at (0, 0). Top-left ordering ranks
    // by (top, left) — leaf-b (top=0, left=0) wins over leaf-a (top=0,
    // left=10).
    reg.register_scope(leaf(
        leaf_a_fq,
        "ui:leaf-a",
        "/L",
        Some(zone_fq.clone()),
        rect_at(10.0, 0.0),
    ));
    reg.register_scope(leaf(
        leaf_b_fq.clone(),
        "ui:leaf-b",
        "/L",
        Some(zone_fq.clone()),
        rect_at(0.0, 0.0),
    ));

    let target = reg.drill_in(zone_fq.clone(), &zone_fq);
    assert_eq!(target, leaf_b_fq);
}

// ---------------------------------------------------------------------------
// drill_in — Zone with no last_focused
// ---------------------------------------------------------------------------

/// A zone with no `last_focused` at all (cold-start, no prior visit) drills
/// into its first child by rect top-left.
#[test]
fn drill_in_zone_with_no_last_focused_uses_first_child_by_rect() {
    let mut reg = SpatialRegistry::new();
    let zone_fq = fq_in_layer("/L", "ui:zone");
    let leaf_top_fq =
        FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:leaf-top"));
    let leaf_bottom_fq =
        FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:leaf-bottom"));
    reg.register_zone(zone(zone_fq.clone(), "ui:zone", "/L", None, None));
    // Two children: one at (5, 5), one at (5, 0). Top-left ordering ranks
    // (top=0, left=5) over (top=5, left=5) since `top` is the primary key.
    reg.register_scope(leaf(
        leaf_bottom_fq,
        "ui:leaf-bottom",
        "/L",
        Some(zone_fq.clone()),
        rect_at(5.0, 5.0),
    ));
    reg.register_scope(leaf(
        leaf_top_fq.clone(),
        "ui:leaf-top",
        "/L",
        Some(zone_fq.clone()),
        rect_at(5.0, 0.0),
    ));

    let target = reg.drill_in(zone_fq.clone(), &zone_fq);
    assert_eq!(target, leaf_top_fq);
}

// ---------------------------------------------------------------------------
// drill_in — Zone with no children
// ---------------------------------------------------------------------------

/// An empty zone — no `last_focused`, no children registered yet — returns
/// the focused FQM. The frontend interprets the equality with the prior
/// focus as "stay where you are" and falls through to onEdit / no-op.
#[test]
fn drill_in_empty_zone_returns_focused_fq() {
    let mut reg = SpatialRegistry::new();
    let zone_fq = fq_in_layer("/L", "ui:zone");
    reg.register_zone(zone(zone_fq.clone(), "ui:zone", "/L", None, None));

    let target = reg.drill_in(zone_fq.clone(), &zone_fq);
    assert_eq!(target, zone_fq);
}

// ---------------------------------------------------------------------------
// drill_in — FocusScope
// ---------------------------------------------------------------------------

/// Drill-in on a leaf returns the focused FQM — leaves do not have
/// children. The React side detects the equality and falls through to
/// the leaf's inline-edit affordance (or no-op for non-editable leaves).
#[test]
fn drill_in_focusable_returns_focused_fq() {
    let mut reg = SpatialRegistry::new();
    let leaf_fq = fq_in_layer("/L", "ui:leaf");
    reg.register_scope(leaf(
        leaf_fq.clone(),
        "ui:leaf",
        "/L",
        None,
        rect_at(0.0, 0.0),
    ));

    let target = reg.drill_in(leaf_fq.clone(), &leaf_fq);
    assert_eq!(target, leaf_fq);
}

// ---------------------------------------------------------------------------
// drill_in — Unknown FQM
// ---------------------------------------------------------------------------

/// Drill-in for an unknown FQM echoes the input focused FQM. The
/// kernel emits `tracing::error!` (verified in
/// `tests/no_silent_none.rs`); the React side's user-visible behavior
/// matches the no-children case (focus stays put).
#[test]
fn drill_in_unknown_fq_echoes_focused_fq() {
    let reg = SpatialRegistry::new();
    let focused_fq = FullyQualifiedMoniker::from_string("/L/ui:focused");
    let ghost_fq = FullyQualifiedMoniker::from_string("/L/ghost");
    let target = reg.drill_in(ghost_fq, &focused_fq);
    assert_eq!(target, focused_fq);
}

// ---------------------------------------------------------------------------
// drill_out — FocusScope to its parent zone
// ---------------------------------------------------------------------------

/// Drill-out on a leaf returns the FQM of its enclosing zone.
#[test]
fn drill_out_focusable_returns_parent_zone_fq() {
    let mut reg = SpatialRegistry::new();
    let zone_fq = fq_in_layer("/L", "ui:zone");
    let leaf_fq = FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:leaf"));
    reg.register_zone(zone(zone_fq.clone(), "ui:zone", "/L", None, None));
    reg.register_scope(leaf(
        leaf_fq.clone(),
        "ui:leaf",
        "/L",
        Some(zone_fq.clone()),
        rect_at(0.0, 0.0),
    ));

    let target = reg.drill_out(leaf_fq.clone(), &leaf_fq);
    assert_eq!(target, zone_fq);
}

// ---------------------------------------------------------------------------
// drill_out — Zone to its parent zone
// ---------------------------------------------------------------------------

/// Drill-out on a zone returns its enclosing zone's FQM — zones nest,
/// so `drill_out` on an inner zone moves focus to the outer one.
#[test]
fn drill_out_zone_returns_parent_zone_fq() {
    let mut reg = SpatialRegistry::new();
    let outer_fq = fq_in_layer("/L", "ui:outer");
    let inner_fq =
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:inner"));
    reg.register_zone(zone(outer_fq.clone(), "ui:outer", "/L", None, None));
    reg.register_zone(zone(
        inner_fq.clone(),
        "ui:inner",
        "/L",
        Some(outer_fq.clone()),
        None,
    ));

    let target = reg.drill_out(inner_fq.clone(), &inner_fq);
    assert_eq!(target, outer_fq);
}

// ---------------------------------------------------------------------------
// drill_out — Layer-root scope (no parent_zone)
// ---------------------------------------------------------------------------

/// Drill-out on a scope that has no `parent_zone` (sits directly under the
/// layer root) returns the focused FQM (semantic "stay put"). The
/// frontend detects the equality and falls through to the next command in
/// the Escape chain (typically `app.dismiss`).
#[test]
fn drill_out_at_layer_root_returns_focused_fq() {
    let mut reg = SpatialRegistry::new();
    let leaf_fq = fq_in_layer("/L", "ui:leaf");
    reg.register_scope(leaf(
        leaf_fq.clone(),
        "ui:leaf",
        "/L",
        None,
        rect_at(0.0, 0.0),
    ));

    let target = reg.drill_out(leaf_fq.clone(), &leaf_fq);
    assert_eq!(target, leaf_fq);
}

// ---------------------------------------------------------------------------
// drill_out — Unknown FQM
// ---------------------------------------------------------------------------

/// Drill-out for an unknown FQM echoes the input focused FQM. The
/// kernel emits `tracing::error!` (verified in
/// `tests/no_silent_none.rs`); same fall-through semantics as the
/// well-formed layer-root case from the React side's perspective.
#[test]
fn drill_out_unknown_fq_echoes_focused_fq() {
    let reg = SpatialRegistry::new();
    let focused_fq = FullyQualifiedMoniker::from_string("/L/ui:focused");
    let ghost_fq = FullyQualifiedMoniker::from_string("/L/ghost");
    let target = reg.drill_out(ghost_fq, &focused_fq);
    assert_eq!(target, focused_fq);
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
fn drill_in_after_remembered_position_returns_remembered_fq() {
    let mut reg = SpatialRegistry::new();
    let zone_fq = fq_in_layer("/L", "ui:zone");
    let leaf_a_fq =
        FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:leaf-a"));
    let leaf_b_fq =
        FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:leaf-b"));
    // Pre-populate `last_focused` so the test does not depend on the
    // navigator's update path.
    reg.register_zone(zone(
        zone_fq.clone(),
        "ui:zone",
        "/L",
        None,
        Some(leaf_a_fq.clone()),
    ));
    reg.register_scope(leaf(
        leaf_a_fq.clone(),
        "ui:leaf-a",
        "/L",
        Some(zone_fq.clone()),
        rect_at(0.0, 0.0),
    ));
    reg.register_scope(leaf(
        leaf_b_fq,
        "ui:leaf-b",
        "/L",
        Some(zone_fq.clone()),
        rect_at(20.0, 20.0),
    ));

    let target = reg.drill_in(zone_fq.clone(), &zone_fq);
    assert_eq!(
        target, leaf_a_fq,
        "drill-in honors the remembered slot, even when other children exist",
    );
}

// ---------------------------------------------------------------------------
// drill_in — Field zone with pill children
//
// Pin the inspector-specific contract from card
// `01KQ9ZJHRXCY8Z5YT6RF4SG6EK` (bug 3): a field zone whose pill scopes
// are laid out left-to-right (e.g. a `tags` field rendering badge
// pills) drills in to the leftmost pill. The pills' segments follow
// the entity convention `tag:<id>` rather than the synthetic `ui:`
// segments above, but the geometric ordering rule is the same — the
// first child by rect (top, left) wins when no `last_focused` is set.
// ---------------------------------------------------------------------------

/// A field zone whose pill children are laid out horizontally returns the
/// leftmost pill's FQM on drill-in. Pins the inspector contract that
/// "Enter on a focused pill field drills into the first pill" maps to a
/// kernel response that the React side can dispatch `setFocus` against.
#[test]
fn drill_in_field_zone_with_pill_children_returns_first_pill_fq() {
    let mut reg = SpatialRegistry::new();

    // The field zone (`field:task:T1.tags`) sits inside the inspector
    // panel zone in production; here we anchor it directly in the
    // layer for simplicity. The contract under test is the in-zone
    // ordering, which doesn't depend on the parent chain.
    let field_fq = fq_in_layer("/L", "field:task:T1.tags");
    reg.register_zone(zone(
        field_fq.clone(),
        "field:task:T1.tags",
        "/L",
        None,
        None,
    ));

    let pill_bug_fq =
        FullyQualifiedMoniker::compose(&field_fq, &SegmentMoniker::from_string("tag:tag-bug"));
    let pill_ui_fq =
        FullyQualifiedMoniker::compose(&field_fq, &SegmentMoniker::from_string("tag:tag-ui"));
    let pill_docs_fq =
        FullyQualifiedMoniker::compose(&field_fq, &SegmentMoniker::from_string("tag:tag-docs"));

    // Three pill scopes, horizontally progressing on the same row.
    // Top-left ordering ranks (top=0, left=0) before (top=0, left=10)
    // before (top=0, left=20) — `tag:tag-bug` wins.
    reg.register_scope(leaf(
        pill_bug_fq.clone(),
        "tag:tag-bug",
        "/L",
        Some(field_fq.clone()),
        rect_at(0.0, 0.0),
    ));
    reg.register_scope(leaf(
        pill_ui_fq,
        "tag:tag-ui",
        "/L",
        Some(field_fq.clone()),
        rect_at(10.0, 0.0),
    ));
    reg.register_scope(leaf(
        pill_docs_fq,
        "tag:tag-docs",
        "/L",
        Some(field_fq.clone()),
        rect_at(20.0, 0.0),
    ));

    let target = reg.drill_in(field_fq.clone(), &field_fq);
    assert_eq!(
        target, pill_bug_fq,
        "drill-in on a tags field zone returns the leftmost pill's FQM",
    );
}

// ---------------------------------------------------------------------------
// drill_in — Field zone with no children
//
// Pin the "Enter falls through to edit mode" contract from card
// `01KQ9ZJHRXCY8Z5YT6RF4SG6EK` (bug 3): when a field zone has no pill
// children (e.g. an empty tags field, or an editable scalar like
// `name`), drill-in returns the field's own FQM. The React side
// detects the equality and falls through to `onEdit?.()` — opening the
// editor for editable fields, no-op for read-only ones.
// ---------------------------------------------------------------------------

/// A field zone with no spatial children — the canonical "scalar leaf
/// without a click-into structure" case — returns the focused FQM
/// on drill-in. React detects the equality (result == focused_fq)
/// and falls through to `onEdit?.()`.
#[test]
fn drill_in_field_zone_with_no_children_returns_focused_fq() {
    let mut reg = SpatialRegistry::new();

    // Register the field zone but no pill children — mirrors an
    // editable scalar field (e.g. `name`) or an empty pill field.
    let field_fq = fq_in_layer("/L", "field:task:T1.name");
    reg.register_zone(zone(
        field_fq.clone(),
        "field:task:T1.name",
        "/L",
        None,
        None,
    ));

    let target = reg.drill_in(field_fq.clone(), &field_fq);
    assert_eq!(
        target, field_fq,
        "drill-in on a childless field zone echoes the focused FQM — \
         React detects equality and falls through to onEdit",
    );
}
