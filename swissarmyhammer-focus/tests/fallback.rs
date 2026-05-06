//! Integration tests for `SpatialState` zone-aware fallback resolution.
//!
//! Headless pattern matching `tests/focus_state.rs` — pure Rust, no Tauri
//! runtime, no jsdom. Every fallback runs through the public
//! [`SpatialState::handle_unregister`] surface (registry-aware variant)
//! and is asserted by inspecting the returned [`FallbackResolution`] plus
//! the emitted [`FocusChangedEvent`].
//!
//! These tests cover the dynamic-lifecycle card
//! (`01KNS0B3HYNXDFGV3ZMN6JCK1E`):
//!
//! - **Sibling-in-zone** — when the focused entry's parent_zone still
//!   has live siblings, fallback picks the nearest sibling in the same
//!   zone.
//! - **Parent-zone last-focused** — when the lost entry's zone empties,
//!   fallback walks up to the parent zone and uses its `last_focused`
//!   if still registered.
//! - **Parent-zone nearest** — when the parent zone's `last_focused` is
//!   stale, fallback picks the nearest entry in that zone.
//! - **Parent-layer fallback** — when the layer root has no remaining
//!   entries, fallback walks `layer.parent` and uses that layer's
//!   `last_focused`.
//! - **No-focus** — at a lone window root with no parent layer, the
//!   resolution is `NoFocus` and the event clears focus.
//! - **WindowLabel barrier** — fallback never returns an entry whose
//!   `window_label` differs from the lost entry's window.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    FallbackResolution, FocusLayer, FocusOverrides, FocusScope, FullyQualifiedMoniker,
    IndexedSnapshot, LayerName, LostFocusContext, NavSnapshot, Pixels, Rect, SegmentMoniker,
    SnapshotScope, SpatialRegistry, SpatialState, WindowLabel,
};

// ---------------------------------------------------------------------------
// Builders — small helpers that keep test setup readable.
// ---------------------------------------------------------------------------

/// Build a `Rect` from raw `f64` coordinates.
fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        x: Pixels::new(x),
        y: Pixels::new(y),
        width: Pixels::new(w),
        height: Pixels::new(h),
    }
}

/// FQM for a primitive registered directly under a layer's root.
fn fq_in_layer(layer_path: &str, segment: &str) -> FullyQualifiedMoniker {
    FullyQualifiedMoniker::from_string(format!("{layer_path}/{segment}"))
}

/// Build a [`FocusScope`] leaf with the given identity, rect, layer, and
/// optional parent zone.
fn leaf(
    fq: FullyQualifiedMoniker,
    segment: &str,
    layer: &str,
    parent_zone: Option<FullyQualifiedMoniker>,
    r: Rect,
) -> FocusScope {
    FocusScope {
        fq,
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone,
        overrides: HashMap::new(),
        last_focused: None,
    }
}

/// Build a [`FocusScope`] with optional `last_focused` and parent zone.
fn zone(
    fq: FullyQualifiedMoniker,
    segment: &str,
    layer: &str,
    parent_zone: Option<FullyQualifiedMoniker>,
    last_focused: Option<FullyQualifiedMoniker>,
    r: Rect,
) -> FocusScope {
    FocusScope {
        fq,
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone,
        last_focused,
        overrides: HashMap::new(),
    }
}

/// Build a [`FocusLayer`] tied to a window.
fn layer(
    fq_str: &str,
    segment: &str,
    window: &str,
    parent: Option<&str>,
    last_focused: Option<FullyQualifiedMoniker>,
) -> FocusLayer {
    FocusLayer {
        fq: FullyQualifiedMoniker::from_string(fq_str),
        segment: SegmentMoniker::from_string(segment),
        name: LayerName::from_string("window"),
        parent: parent.map(FullyQualifiedMoniker::from_string),
        window_label: WindowLabel::from_string(window),
        last_focused,
    }
}

/// Build a [`NavSnapshot`] of every live scope under `layer_fq` in
/// `registry`, excluding `omit_fq`. Mirrors what React would build at
/// the moment of `spatial_focus_lost`: the lost FQM has just been
/// removed from the layer registry, so the snapshot's `scopes` vector
/// only contains the survivors.
fn snapshot_excluding(
    registry: &SpatialRegistry,
    layer_fq: &FullyQualifiedMoniker,
    omit_fq: &FullyQualifiedMoniker,
) -> NavSnapshot {
    let scopes = registry
        .scopes_in_layer(layer_fq)
        .filter(|s| &s.fq != omit_fq)
        .map(|s| SnapshotScope {
            fq: s.fq.clone(),
            rect: s.rect,
            parent_zone: s.parent_zone.clone(),
            nav_override: FocusOverrides::new(),
        })
        .collect();
    NavSnapshot {
        layer_fq: layer_fq.clone(),
        scopes,
    }
}

/// Run `resolve_fallback_with_snapshot` against the registry path's
/// expected `(parent_zone, layer_fq, rect)` for `lost_fq`. Builds a
/// snapshot of the lost layer that excludes `lost_fq` (matching the
/// React-side "already removed" shape) and constructs the
/// [`LostFocusContext`] inline.
fn resolve_via_snapshot(
    state: &SpatialState,
    registry: &SpatialRegistry,
    lost_fq: &FullyQualifiedMoniker,
) -> FallbackResolution {
    let lost = registry
        .find_by_fq(lost_fq)
        .expect("lost FQM must still be registered for the snapshot test setup");
    let lost_layer_fq = lost.layer_fq.clone();
    let lost_parent_zone = lost.parent_zone.clone();
    let lost_rect = lost.rect;

    let snap = snapshot_excluding(registry, &lost_layer_fq, lost_fq);
    let idx = IndexedSnapshot::new(&snap);
    let ctx = LostFocusContext {
        view: &idx,
        lost_layer_fq,
        lost_parent_zone,
        lost_rect,
    };
    state.resolve_fallback_with_snapshot(registry, lost_fq, &ctx)
}

// ---------------------------------------------------------------------------
// Sibling in same zone (rule 1)
// ---------------------------------------------------------------------------

/// When the focused entry's parent_zone still has live siblings,
/// fallback picks the nearest sibling in the same zone. The sibling's
/// FQM and segment are returned in the
/// [`FallbackResolution::FallbackSiblingInZone`] variant.
#[test]
fn fallback_returns_sibling_in_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None, None));
    let zone_fq = fq_in_layer("/L", "ui:zone");
    reg.register_scope(zone(
        zone_fq.clone(),
        "ui:zone",
        "/L",
        None,
        None,
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    // Two sibling leaves in the same zone, plus the lost focused leaf.
    let lost_fq = FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:lost"));
    let sib_near_fq =
        FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:sib-near"));
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/L",
        Some(zone_fq.clone()),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        sib_near_fq.clone(),
        "ui:sib-near",
        "/L",
        Some(zone_fq.clone()),
        rect(20.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:sib-far")),
        "ui:sib-far",
        "/L",
        Some(zone_fq),
        rect(100.0, 100.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state.focus(&mut reg, lost_fq.clone()).expect("focus lost");

    let resolution = state.resolve_fallback(&reg, &lost_fq);
    match resolution {
        FallbackResolution::FallbackSiblingInZone(fq, segment) => {
            assert_eq!(fq, sib_near_fq);
            assert_eq!(segment, SegmentMoniker::from_string("ui:sib-near"));
        }
        other => panic!("expected FallbackSiblingInZone, got {other:?}"),
    }
}

/// Regression: rule 1 (sibling-in-zone) outranks rule 2 even when the
/// lost entry's *own* `parent_zone` carries a `last_focused` slot.
///
/// The cascade documents rule 1 as "nearest sibling in the lost entry's
/// zone" and rule 2 as "walk up to ancestor zones and prefer their
/// `last_focused`". Reading the lost entry's *own* zone's
/// `last_focused` would silently swap rule 2 priority into rule 1's
/// slot — the bug this test guards against.
#[test]
fn fallback_prefers_sibling_over_lost_zone_last_focused() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None, None));
    let zone_fq = fq_in_layer("/L", "ui:zone");
    let remembered_fq =
        FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:remembered"));
    // The lost entry's zone has a `last_focused` slot pointing at
    // "remembered" — but rule 1 should ignore it and pick the nearest
    // sibling instead.
    reg.register_scope(zone(
        zone_fq.clone(),
        "ui:zone",
        "/L",
        None,
        Some(remembered_fq.clone()),
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    let lost_fq = FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:lost"));
    let sib_near_fq =
        FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:sib-near"));
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/L",
        Some(zone_fq.clone()),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    // Closest sibling — should win even though "remembered" is the
    // zone's recorded last-focused slot.
    reg.register_scope(leaf(
        sib_near_fq.clone(),
        "ui:sib-near",
        "/L",
        Some(zone_fq.clone()),
        rect(20.0, 0.0, 10.0, 10.0),
    ));
    // The "remembered" leaf is registered but is geometrically farther
    // than `sib-near`. If rule 1 incorrectly consulted the zone's
    // `last_focused`, this test would surface
    // `FallbackParentZoneLastFocused` instead of the expected
    // `FallbackSiblingInZone`.
    reg.register_scope(leaf(
        remembered_fq,
        "ui:remembered",
        "/L",
        Some(zone_fq),
        rect(150.0, 150.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state.focus(&mut reg, lost_fq.clone()).expect("focus lost");

    let resolution = state.resolve_fallback(&reg, &lost_fq);
    match resolution {
        FallbackResolution::FallbackSiblingInZone(fq, segment) => {
            assert_eq!(fq, sib_near_fq);
            assert_eq!(segment, SegmentMoniker::from_string("ui:sib-near"));
        }
        other => {
            panic!("rule 1 must outrank the lost entry's own zone `last_focused`; got {other:?}",)
        }
    }
}

// ---------------------------------------------------------------------------
// Parent zone's last_focused (rule 2)
// ---------------------------------------------------------------------------

/// When the lost entry's zone empties, fallback walks up to the parent
/// zone and uses its `last_focused` if still registered.
///
/// Note: the test pre-populates `outer.last_focused = remembered_fq` to
/// describe the fallback-tree shape directly, but
/// [`SpatialState::focus`] (the kernel writer) would overwrite that
/// slot if invoked on `lost_fq` because focusing `lost` walks up
/// outer's ancestor chain. The test therefore drives the resolver
/// directly without staging focus through `state.focus`; the hand-
/// populated slot is the resolver-input under test.
#[test]
fn fallback_returns_parent_zone_last_focused() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None, None));
    let outer_fq = fq_in_layer("/L", "ui:outer");
    let remembered_fq =
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:remembered"));
    // Outer zone with last_focused pointing at "remembered".
    reg.register_scope(zone(
        outer_fq.clone(),
        "ui:outer",
        "/L",
        None,
        Some(remembered_fq.clone()),
        rect(0.0, 0.0, 500.0, 500.0),
    ));
    let inner_fq =
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:inner"));
    // Inner zone (about to empty when "lost" is unregistered).
    reg.register_scope(zone(
        inner_fq.clone(),
        "ui:inner",
        "/L",
        Some(outer_fq.clone()),
        None,
        rect(0.0, 0.0, 100.0, 100.0),
    ));
    let lost_fq =
        FullyQualifiedMoniker::compose(&inner_fq, &SegmentMoniker::from_string("ui:lost"));
    // Sole leaf in the inner zone.
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/L",
        Some(inner_fq.clone()),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    // The remembered scope sits in the outer zone (parent of inner).
    reg.register_scope(leaf(
        remembered_fq.clone(),
        "ui:remembered",
        "/L",
        Some(outer_fq.clone()),
        rect(200.0, 200.0, 10.0, 10.0),
    ));
    // Another sibling in outer for variety, but rule 2 should still pick
    // the remembered slot.
    reg.register_scope(leaf(
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:other")),
        "ui:other",
        "/L",
        Some(outer_fq),
        rect(300.0, 300.0, 10.0, 10.0),
    ));

    let state = SpatialState::new();

    // Resolve directly against the hand-populated registry; the
    // resolver is a pure registry query that does not depend on any
    // `focus_by_window` state.
    let resolution = state.resolve_fallback(&reg, &lost_fq);
    match resolution {
        FallbackResolution::FallbackParentZoneLastFocused(fq, segment) => {
            assert_eq!(fq, remembered_fq);
            assert_eq!(segment, SegmentMoniker::from_string("ui:remembered"));
        }
        other => panic!("expected FallbackParentZoneLastFocused, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Parent zone nearest entry (rule 2 fallback when last_focused is stale)
// ---------------------------------------------------------------------------

/// When the parent zone's `last_focused` is stale (no longer registered),
/// fallback picks the nearest entry in that zone.
///
/// Rule 2 does not apply variant preference, so the nearest live
/// candidate wins regardless of whether it's a leaf or a zone. The
/// candidates are positioned so the leaf nearest the lost rect wins
/// purely on distance — the inner zone (the lost entry's now-empty
/// container) sits far away so it can't ghost-block the resolution.
#[test]
fn fallback_returns_parent_zone_nearest_when_last_focused_stale() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None, None));
    let outer_fq = fq_in_layer("/L", "ui:outer");
    let ghost_fq = FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ghost"));
    reg.register_scope(zone(
        outer_fq.clone(),
        "ui:outer",
        "/L",
        None,
        Some(ghost_fq), // points at unregistered FQM
        rect(0.0, 0.0, 500.0, 500.0),
    ));
    let inner_fq =
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:inner"));
    // Inner zone positioned far from the lost rect so it does not
    // beat `near` on distance once rule 2's nearest scan runs.
    reg.register_scope(zone(
        inner_fq.clone(),
        "ui:inner",
        "/L",
        Some(outer_fq.clone()),
        None,
        rect(400.0, 400.0, 100.0, 100.0),
    ));
    let lost_fq =
        FullyQualifiedMoniker::compose(&inner_fq, &SegmentMoniker::from_string("ui:lost"));
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/L",
        Some(inner_fq),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    // Two leaves in the outer zone — the nearest by top-left wins.
    let near_fq =
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:near"));
    reg.register_scope(leaf(
        near_fq.clone(),
        "ui:near",
        "/L",
        Some(outer_fq.clone()),
        rect(20.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:far")),
        "ui:far",
        "/L",
        Some(outer_fq),
        rect(300.0, 300.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state.focus(&mut reg, lost_fq.clone()).expect("focus lost");

    let resolution = state.resolve_fallback(&reg, &lost_fq);
    match resolution {
        FallbackResolution::FallbackParentZoneNearest(fq, segment) => {
            assert_eq!(fq, near_fq);
            assert_eq!(segment, SegmentMoniker::from_string("ui:near"));
        }
        other => panic!("expected FallbackParentZoneNearest, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Parent layer fallback (rule 4)
// ---------------------------------------------------------------------------

/// When the layer root has no remaining entries, fallback walks
/// `layer.parent` and uses that layer's `last_focused` if still
/// registered.
///
/// Note: the test pre-populates `root.last_focused = root_leaf_fq` to
/// describe the fallback-tree shape directly, but
/// [`SpatialState::focus`] (the kernel writer) would overwrite that
/// slot if invoked on `lost_fq` because focusing `lost` walks up
/// root's layer-ancestor chain. The test therefore drives the resolver
/// directly without staging focus through `state.focus`; the hand-
/// populated slot is the resolver-input under test.
#[test]
fn fallback_returns_parent_layer_last_focused() {
    let mut reg = SpatialRegistry::new();
    let root_leaf_fq = fq_in_layer("/root", "ui:root-leaf");
    // Root layer of the window with a remembered slot inside.
    reg.push_layer(layer(
        "/root",
        "root",
        "main",
        None,
        Some(root_leaf_fq.clone()),
    ));
    // Child layer (e.g. inspector overlay). The lost focused entry
    // lives here; when it goes the layer has no entries left.
    reg.push_layer(layer("/root/child", "child", "main", Some("/root"), None));
    let lost_fq = fq_in_layer("/root/child", "ui:lost");
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/root/child",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    // Live entry in the parent layer.
    reg.register_scope(leaf(
        root_leaf_fq.clone(),
        "ui:root-leaf",
        "/root",
        None,
        rect(20.0, 20.0, 10.0, 10.0),
    ));

    let state = SpatialState::new();

    // Resolve directly against the hand-populated registry; the
    // resolver is a pure registry query that does not depend on any
    // `focus_by_window` state.
    let resolution = state.resolve_fallback(&reg, &lost_fq);
    match resolution {
        FallbackResolution::FallbackParentLayerLastFocused(fq, segment) => {
            assert_eq!(fq, root_leaf_fq);
            assert_eq!(segment, SegmentMoniker::from_string("ui:root-leaf"));
        }
        other => panic!("expected FallbackParentLayerLastFocused, got {other:?}"),
    }
}

/// When the ancestor layer's `last_focused` is stale or absent, rule 4's
/// fallback picks the nearest live scope **anywhere** in the ancestor
/// layer — including a leaf nested inside an ancestor zone, not just
/// scopes hanging directly under the layer root. The previous
/// implementation limited the candidate set to `parent_zone is None`,
/// silently skipping every leaf that lived inside one of the parent
/// layer's zones; this test guards against that regression by making
/// the *only* candidate a leaf nested inside an ancestor zone.
#[test]
fn fallback_returns_parent_layer_nearest_includes_zone_nested_leaves() {
    let mut reg = SpatialRegistry::new();
    // Root layer with no remembered slot — forces rule 4 to fall through
    // to the nearest-in-layer scan.
    reg.push_layer(layer("/root", "root", "main", None, None));
    // Child layer (e.g. inspector overlay) holds the lost entry alone.
    reg.push_layer(layer("/root/child", "child", "main", Some("/root"), None));
    let lost_fq = fq_in_layer("/root/child", "ui:lost");
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/root/child",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    // A nested leaf is the only candidate in the parent layer. Its
    // enclosing zone is positioned far away so the leaf wins on raw
    // distance regardless of variant — the assertion is that rule 4's
    // nearest-fallback can reach a zone-nested leaf at all (the bug
    // this test guards against was the candidate set being limited to
    // `parent_zone is None`).
    let root_zone_fq = fq_in_layer("/root", "ui:root-zone");
    reg.register_scope(zone(
        root_zone_fq.clone(),
        "ui:root-zone",
        "/root",
        None,
        None,
        rect(500.0, 500.0, 100.0, 100.0),
    ));
    let nested_fq =
        FullyQualifiedMoniker::compose(&root_zone_fq, &SegmentMoniker::from_string("ui:nested"));
    reg.register_scope(leaf(
        nested_fq.clone(),
        "ui:nested",
        "/root",
        Some(root_zone_fq),
        rect(20.0, 20.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state.focus(&mut reg, lost_fq.clone()).expect("focus lost");

    let resolution = state.resolve_fallback(&reg, &lost_fq);
    match resolution {
        FallbackResolution::FallbackParentLayerNearest(fq, segment) => {
            assert_eq!(fq, nested_fq);
            assert_eq!(segment, SegmentMoniker::from_string("ui:nested"));
        }
        other => panic!("expected FallbackParentLayerNearest, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// No-focus (lone window root)
// ---------------------------------------------------------------------------

/// At a lone window root with no parent layer and no other entries,
/// the resolution is `NoFocus`.
#[test]
fn fallback_returns_no_focus_at_lone_window_root() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/root", "root", "main", None, None));
    let lost_fq = fq_in_layer("/root", "ui:lost");
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/root",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state.focus(&mut reg, lost_fq.clone()).expect("focus lost");

    let resolution = state.resolve_fallback(&reg, &lost_fq);
    assert!(
        matches!(resolution, FallbackResolution::NoFocus),
        "expected NoFocus, got {resolution:?}",
    );
}

// ---------------------------------------------------------------------------
// WindowLabel barrier
// ---------------------------------------------------------------------------

/// Fallback never returns an entry whose `window_label` differs from
/// the lost entry's window — the layer.parent walk is bounded by the
/// layer forest within a single window.
///
/// Constructs a deliberately torn registry where layer A in window
/// "win-a" has a `parent` pointing at layer B in window "win-b". This
/// is an invariant violation (parent edges are supposed to stay inside
/// a window), but the resolver still has to defend against it: the
/// per-window barrier check at the start of each layer-walk iteration
/// must short-circuit rather than return a foreign-window candidate.
/// Without that guard, the resolver would happily walk into window B
/// and return its `last_focused` slot.
#[test]
fn fallback_never_crosses_window_boundary() {
    let mut reg = SpatialRegistry::new();
    // Window A: the lost entry lives here. The lost layer is *torn* —
    // its `parent` points into window B's layer, exercising the barrier
    // branch in `resolve_fallback`'s phase 2.
    reg.push_layer(layer("/La", "La", "win-a", Some("/Lb"), None));
    let lost_fq = fq_in_layer("/La", "ui:lost");
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/La",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    // Window B: fully populated. If the barrier were not enforced, the
    // resolver would land on `b-leaf` (via `Lb`'s `last_focused`).
    let b_leaf_fq = fq_in_layer("/Lb", "ui:b-leaf");
    reg.push_layer(layer("/Lb", "Lb", "win-b", None, Some(b_leaf_fq.clone())));
    reg.register_scope(leaf(
        b_leaf_fq,
        "ui:b-leaf",
        "/Lb",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state.focus(&mut reg, lost_fq.clone()).expect("focus lost");

    let resolution = state.resolve_fallback(&reg, &lost_fq);
    assert!(
        matches!(resolution, FallbackResolution::NoFocus),
        "fallback must stay inside window 'win-a' even when the layer-tree \
         points across windows — got {resolution:?}",
    );
}

// ---------------------------------------------------------------------------
// handle_unregister event shape
// ---------------------------------------------------------------------------

/// `handle_unregister` consults the registry to compute zone-aware
/// fallback and emits a [`FocusChangedEvent`] whose `next_fq` /
/// `next_segment` reflect the fallback target.
#[test]
fn handle_unregister_emits_event_with_fallback_target() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None, None));
    let zone_fq = fq_in_layer("/L", "ui:zone");
    reg.register_scope(zone(
        zone_fq.clone(),
        "ui:zone",
        "/L",
        None,
        None,
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    let lost_fq = FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:lost"));
    let sibling_fq =
        FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:sibling"));
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/L",
        Some(zone_fq.clone()),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        sibling_fq.clone(),
        "ui:sibling",
        "/L",
        Some(zone_fq),
        rect(20.0, 0.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state.focus(&mut reg, lost_fq.clone()).expect("focus lost");

    let event = state
        .handle_unregister(&mut reg, &lost_fq, None)
        .expect("handle_unregister emits an event when the focused FQM is unregistered");

    assert_eq!(event.window_label, WindowLabel::from_string("main"));
    assert_eq!(event.prev_fq, Some(lost_fq));
    assert_eq!(event.next_fq, Some(sibling_fq.clone()));
    assert_eq!(
        event.next_segment,
        Some(SegmentMoniker::from_string("ui:sibling"))
    );

    // The window's slot must now point at the fallback target.
    assert_eq!(
        state.focused_in(&WindowLabel::from_string("main")),
        Some(&sibling_fq),
    );
}

/// At a lone window root, `handle_unregister` clears the focus slot and
/// emits a `Some → None` event so the React claim registry can release
/// the focus visual.
#[test]
fn handle_unregister_clears_focus_when_no_fallback() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None, None));
    let lost_fq = fq_in_layer("/L", "ui:lost");
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/L",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state.focus(&mut reg, lost_fq.clone()).expect("focus lost");

    let event = state
        .handle_unregister(&mut reg, &lost_fq, None)
        .expect("handle_unregister emits a clear event when there is no fallback");

    assert_eq!(event.window_label, WindowLabel::from_string("main"));
    assert_eq!(event.prev_fq, Some(lost_fq));
    assert_eq!(event.next_fq, None);
    assert_eq!(event.next_segment, None);
    assert_eq!(state.focused_in(&WindowLabel::from_string("main")), None);
}

/// `handle_unregister` for an unfocused FQM is a no-op — no fallback
/// resolution runs and no event is emitted.
#[test]
fn handle_unregister_unfocused_fq_is_noop() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None, None));
    let focused_fq = fq_in_layer("/L", "ui:focused");
    let other_fq = fq_in_layer("/L", "ui:other");
    reg.register_scope(leaf(
        focused_fq.clone(),
        "ui:focused",
        "/L",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        other_fq.clone(),
        "ui:other",
        "/L",
        None,
        rect(20.0, 20.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state
        .focus(&mut reg, focused_fq.clone())
        .expect("focus focused");

    assert!(
        state.handle_unregister(&mut reg, &other_fq, None).is_none(),
        "unregistering an unfocused FQM emits nothing",
    );
    // Focus slot still points at "focused".
    assert_eq!(
        state.focused_in(&WindowLabel::from_string("main")),
        Some(&focused_fq),
    );
}

// ---------------------------------------------------------------------------
// Snapshot path — every fallback variant resolves identically when the
// in-layer walk reads from a NavSnapshot instead of the registry.
// ---------------------------------------------------------------------------

/// Snapshot variant of `fallback_returns_sibling_in_zone`.
#[test]
fn snapshot_fallback_returns_sibling_in_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None, None));
    let zone_fq = fq_in_layer("/L", "ui:zone");
    reg.register_scope(zone(
        zone_fq.clone(),
        "ui:zone",
        "/L",
        None,
        None,
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    let lost_fq = FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:lost"));
    let sib_near_fq =
        FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:sib-near"));
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/L",
        Some(zone_fq.clone()),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        sib_near_fq.clone(),
        "ui:sib-near",
        "/L",
        Some(zone_fq.clone()),
        rect(20.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:sib-far")),
        "ui:sib-far",
        "/L",
        Some(zone_fq),
        rect(100.0, 100.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state.focus(&mut reg, lost_fq.clone()).expect("focus lost");

    let registry_resolution = state.resolve_fallback(&reg, &lost_fq);
    let snapshot_resolution = resolve_via_snapshot(&state, &reg, &lost_fq);
    assert_eq!(snapshot_resolution, registry_resolution);
    match snapshot_resolution {
        FallbackResolution::FallbackSiblingInZone(fq, segment) => {
            assert_eq!(fq, sib_near_fq);
            assert_eq!(segment, SegmentMoniker::from_string("ui:sib-near"));
        }
        other => panic!("expected FallbackSiblingInZone, got {other:?}"),
    }
}

/// Snapshot variant of `fallback_returns_parent_zone_last_focused`.
#[test]
fn snapshot_fallback_returns_parent_zone_last_focused() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None, None));
    let outer_fq = fq_in_layer("/L", "ui:outer");
    let remembered_fq =
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:remembered"));
    reg.register_scope(zone(
        outer_fq.clone(),
        "ui:outer",
        "/L",
        None,
        Some(remembered_fq.clone()),
        rect(0.0, 0.0, 500.0, 500.0),
    ));
    let inner_fq =
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:inner"));
    reg.register_scope(zone(
        inner_fq.clone(),
        "ui:inner",
        "/L",
        Some(outer_fq.clone()),
        None,
        rect(0.0, 0.0, 100.0, 100.0),
    ));
    let lost_fq =
        FullyQualifiedMoniker::compose(&inner_fq, &SegmentMoniker::from_string("ui:lost"));
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/L",
        Some(inner_fq.clone()),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        remembered_fq.clone(),
        "ui:remembered",
        "/L",
        Some(outer_fq.clone()),
        rect(200.0, 200.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:other")),
        "ui:other",
        "/L",
        Some(outer_fq),
        rect(300.0, 300.0, 10.0, 10.0),
    ));

    let state = SpatialState::new();
    let registry_resolution = state.resolve_fallback(&reg, &lost_fq);
    let snapshot_resolution = resolve_via_snapshot(&state, &reg, &lost_fq);
    assert_eq!(snapshot_resolution, registry_resolution);
    match snapshot_resolution {
        FallbackResolution::FallbackParentZoneLastFocused(fq, segment) => {
            assert_eq!(fq, remembered_fq);
            assert_eq!(segment, SegmentMoniker::from_string("ui:remembered"));
        }
        other => panic!("expected FallbackParentZoneLastFocused, got {other:?}"),
    }
}

/// Snapshot variant of
/// `fallback_returns_parent_zone_nearest_when_last_focused_stale`.
#[test]
fn snapshot_fallback_returns_parent_zone_nearest_when_last_focused_stale() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None, None));
    let outer_fq = fq_in_layer("/L", "ui:outer");
    let ghost_fq = FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ghost"));
    reg.register_scope(zone(
        outer_fq.clone(),
        "ui:outer",
        "/L",
        None,
        Some(ghost_fq),
        rect(0.0, 0.0, 500.0, 500.0),
    ));
    let inner_fq =
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:inner"));
    reg.register_scope(zone(
        inner_fq.clone(),
        "ui:inner",
        "/L",
        Some(outer_fq.clone()),
        None,
        rect(400.0, 400.0, 100.0, 100.0),
    ));
    let lost_fq =
        FullyQualifiedMoniker::compose(&inner_fq, &SegmentMoniker::from_string("ui:lost"));
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/L",
        Some(inner_fq),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    let near_fq =
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:near"));
    reg.register_scope(leaf(
        near_fq.clone(),
        "ui:near",
        "/L",
        Some(outer_fq.clone()),
        rect(20.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:far")),
        "ui:far",
        "/L",
        Some(outer_fq),
        rect(300.0, 300.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state.focus(&mut reg, lost_fq.clone()).expect("focus lost");

    let registry_resolution = state.resolve_fallback(&reg, &lost_fq);
    let snapshot_resolution = resolve_via_snapshot(&state, &reg, &lost_fq);
    assert_eq!(snapshot_resolution, registry_resolution);
    match snapshot_resolution {
        FallbackResolution::FallbackParentZoneNearest(fq, segment) => {
            assert_eq!(fq, near_fq);
            assert_eq!(segment, SegmentMoniker::from_string("ui:near"));
        }
        other => panic!("expected FallbackParentZoneNearest, got {other:?}"),
    }
}

/// Snapshot variant of `fallback_returns_parent_layer_last_focused`.
#[test]
fn snapshot_fallback_returns_parent_layer_last_focused() {
    let mut reg = SpatialRegistry::new();
    let root_leaf_fq = fq_in_layer("/root", "ui:root-leaf");
    reg.push_layer(layer(
        "/root",
        "root",
        "main",
        None,
        Some(root_leaf_fq.clone()),
    ));
    reg.push_layer(layer("/root/child", "child", "main", Some("/root"), None));
    let lost_fq = fq_in_layer("/root/child", "ui:lost");
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/root/child",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        root_leaf_fq.clone(),
        "ui:root-leaf",
        "/root",
        None,
        rect(20.0, 20.0, 10.0, 10.0),
    ));

    let state = SpatialState::new();
    let registry_resolution = state.resolve_fallback(&reg, &lost_fq);
    let snapshot_resolution = resolve_via_snapshot(&state, &reg, &lost_fq);
    assert_eq!(snapshot_resolution, registry_resolution);
    match snapshot_resolution {
        FallbackResolution::FallbackParentLayerLastFocused(fq, segment) => {
            assert_eq!(fq, root_leaf_fq);
            assert_eq!(segment, SegmentMoniker::from_string("ui:root-leaf"));
        }
        other => panic!("expected FallbackParentLayerLastFocused, got {other:?}"),
    }
}

/// Snapshot variant of
/// `fallback_returns_parent_layer_nearest_includes_zone_nested_leaves`.
#[test]
fn snapshot_fallback_returns_parent_layer_nearest_includes_zone_nested_leaves() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/root", "root", "main", None, None));
    reg.push_layer(layer("/root/child", "child", "main", Some("/root"), None));
    let lost_fq = fq_in_layer("/root/child", "ui:lost");
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/root/child",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    let root_zone_fq = fq_in_layer("/root", "ui:root-zone");
    reg.register_scope(zone(
        root_zone_fq.clone(),
        "ui:root-zone",
        "/root",
        None,
        None,
        rect(500.0, 500.0, 100.0, 100.0),
    ));
    let nested_fq =
        FullyQualifiedMoniker::compose(&root_zone_fq, &SegmentMoniker::from_string("ui:nested"));
    reg.register_scope(leaf(
        nested_fq.clone(),
        "ui:nested",
        "/root",
        Some(root_zone_fq),
        rect(20.0, 20.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state.focus(&mut reg, lost_fq.clone()).expect("focus lost");

    let registry_resolution = state.resolve_fallback(&reg, &lost_fq);
    let snapshot_resolution = resolve_via_snapshot(&state, &reg, &lost_fq);
    assert_eq!(snapshot_resolution, registry_resolution);
    match snapshot_resolution {
        FallbackResolution::FallbackParentLayerNearest(fq, segment) => {
            assert_eq!(fq, nested_fq);
            assert_eq!(segment, SegmentMoniker::from_string("ui:nested"));
        }
        other => panic!("expected FallbackParentLayerNearest, got {other:?}"),
    }
}

/// Snapshot variant of `fallback_returns_no_focus_at_lone_window_root`.
#[test]
fn snapshot_fallback_returns_no_focus_at_lone_window_root() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/root", "root", "main", None, None));
    let lost_fq = fq_in_layer("/root", "ui:lost");
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/root",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state.focus(&mut reg, lost_fq.clone()).expect("focus lost");

    let registry_resolution = state.resolve_fallback(&reg, &lost_fq);
    let snapshot_resolution = resolve_via_snapshot(&state, &reg, &lost_fq);
    assert_eq!(snapshot_resolution, registry_resolution);
    assert!(
        matches!(snapshot_resolution, FallbackResolution::NoFocus),
        "expected NoFocus, got {snapshot_resolution:?}",
    );
}

/// Snapshot variant of `fallback_prefers_sibling_over_lost_zone_last_focused` —
/// the same rule-1-outranks-rule-2 invariant must hold under the snapshot path.
#[test]
fn snapshot_fallback_prefers_sibling_over_lost_zone_last_focused() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None, None));
    let zone_fq = fq_in_layer("/L", "ui:zone");
    let remembered_fq =
        FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:remembered"));
    reg.register_scope(zone(
        zone_fq.clone(),
        "ui:zone",
        "/L",
        None,
        Some(remembered_fq.clone()),
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    let lost_fq = FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:lost"));
    let sib_near_fq =
        FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:sib-near"));
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/L",
        Some(zone_fq.clone()),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        sib_near_fq.clone(),
        "ui:sib-near",
        "/L",
        Some(zone_fq.clone()),
        rect(20.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        remembered_fq,
        "ui:remembered",
        "/L",
        Some(zone_fq),
        rect(150.0, 150.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state.focus(&mut reg, lost_fq.clone()).expect("focus lost");

    let registry_resolution = state.resolve_fallback(&reg, &lost_fq);
    let snapshot_resolution = resolve_via_snapshot(&state, &reg, &lost_fq);
    assert_eq!(snapshot_resolution, registry_resolution);
    match snapshot_resolution {
        FallbackResolution::FallbackSiblingInZone(fq, segment) => {
            assert_eq!(fq, sib_near_fq);
            assert_eq!(segment, SegmentMoniker::from_string("ui:sib-near"));
        }
        other => panic!("expected FallbackSiblingInZone, got {other:?}"),
    }
}

/// Snapshot variant of `fallback_never_crosses_window_boundary`.
#[test]
fn snapshot_fallback_never_crosses_window_boundary() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/La", "La", "win-a", Some("/Lb"), None));
    let lost_fq = fq_in_layer("/La", "ui:lost");
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/La",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    let b_leaf_fq = fq_in_layer("/Lb", "ui:b-leaf");
    reg.push_layer(layer("/Lb", "Lb", "win-b", None, Some(b_leaf_fq.clone())));
    reg.register_scope(leaf(
        b_leaf_fq,
        "ui:b-leaf",
        "/Lb",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state.focus(&mut reg, lost_fq.clone()).expect("focus lost");

    let registry_resolution = state.resolve_fallback(&reg, &lost_fq);
    let snapshot_resolution = resolve_via_snapshot(&state, &reg, &lost_fq);
    assert_eq!(snapshot_resolution, registry_resolution);
    assert!(
        matches!(snapshot_resolution, FallbackResolution::NoFocus),
        "fallback must stay inside window 'win-a' even when the layer-tree \
         points across windows — got {snapshot_resolution:?}",
    );
}

// ---------------------------------------------------------------------------
// Snapshot edge cases — required by step 4 acceptance criteria.
// ---------------------------------------------------------------------------

/// When the lost FQM's `lost_layer_fq` is not present in `registry.layers`,
/// both the registry path and the snapshot path resolve to `NoFocus`.
///
/// The registry-side metadata lookup short-circuits in the registry path
/// when the layer is missing; the snapshot path receives the lost layer
/// FQM directly via [`LostFocusContext`] and the inner resolver also
/// short-circuits when the layer cannot be found in the registry's
/// `layers` map.
#[test]
fn snapshot_fallback_no_focus_when_layer_missing_from_registry() {
    // Reproduce the registry-path behavior first: an FQM whose layer does
    // not exist resolves to `NoFocus` even when the FQM has metadata,
    // because the resolver cannot read the owning window. Construct a
    // registry where the lost FQM IS registered but its layer is not in
    // the layer map.
    let mut reg = SpatialRegistry::new();
    // Push a layer so the layer map has some entry, but the lost FQM
    // points at a non-existent layer FQM.
    reg.push_layer(layer("/real-layer", "real", "main", None, None));
    let ghost_layer_fq = FullyQualifiedMoniker::from_string("/ghost-layer");
    let lost_fq = fq_in_layer("/ghost-layer", "ui:lost");
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/ghost-layer",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));

    let state = SpatialState::new();
    let registry_resolution = state.resolve_fallback(&reg, &lost_fq);
    assert!(
        matches!(registry_resolution, FallbackResolution::NoFocus),
        "registry path must produce NoFocus when the lost layer is missing; got {registry_resolution:?}",
    );

    // Snapshot path: pass the same ghost layer FQM in `LostFocusContext`.
    // The snapshot is empty (no live scopes in the missing layer).
    let snap = NavSnapshot {
        layer_fq: ghost_layer_fq.clone(),
        scopes: vec![],
    };
    let idx = IndexedSnapshot::new(&snap);
    let ctx = LostFocusContext {
        view: &idx,
        lost_layer_fq: ghost_layer_fq,
        lost_parent_zone: None,
        lost_rect: rect(0.0, 0.0, 10.0, 10.0),
    };
    let snapshot_resolution = state.resolve_fallback_with_snapshot(&reg, &lost_fq, &ctx);
    assert_eq!(snapshot_resolution, registry_resolution);
}

/// Lost FQM has no `parent_zone` (it was registered directly under the
/// layer root). The phase 1 walk goes straight to the layer's
/// `last_focused` slot — produced by the layer-tree walk in phase 2 in
/// this test setup, since the lost FQM was the only scope in its layer.
///
/// Both the registry and snapshot paths must agree on the resolution.
#[test]
fn snapshot_fallback_walks_to_layer_last_focused_when_no_parent_zone() {
    let mut reg = SpatialRegistry::new();
    let root_leaf_fq = fq_in_layer("/root", "ui:root-leaf");
    reg.push_layer(layer(
        "/root",
        "root",
        "main",
        None,
        Some(root_leaf_fq.clone()),
    ));
    reg.push_layer(layer("/root/child", "child", "main", Some("/root"), None));
    // Lost FQM is the sole scope in its layer; its `parent_zone` is None.
    let lost_fq = fq_in_layer("/root/child", "ui:lost");
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/root/child",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        root_leaf_fq.clone(),
        "ui:root-leaf",
        "/root",
        None,
        rect(20.0, 20.0, 10.0, 10.0),
    ));

    let state = SpatialState::new();
    let registry_resolution = state.resolve_fallback(&reg, &lost_fq);
    let snapshot_resolution = resolve_via_snapshot(&state, &reg, &lost_fq);
    assert_eq!(snapshot_resolution, registry_resolution);
    // The phase 1 walk runs once with `current_zone = None`, finds no
    // sibling under the now-empty child layer, then phase 2 picks up the
    // parent layer's `last_focused`.
    match snapshot_resolution {
        FallbackResolution::FallbackParentLayerLastFocused(fq, _) => {
            assert_eq!(fq, root_leaf_fq);
        }
        other => panic!("expected FallbackParentLayerLastFocused, got {other:?}"),
    }
}

/// `handle_unregister` with a `Some(LostFocusContext)` produces the same
/// `FocusChangedEvent` as the registry-only path when both can resolve.
#[test]
fn handle_unregister_with_snapshot_emits_same_event_as_registry_path() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None, None));
    let zone_fq = fq_in_layer("/L", "ui:zone");
    reg.register_scope(zone(
        zone_fq.clone(),
        "ui:zone",
        "/L",
        None,
        None,
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    let lost_fq = FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:lost"));
    let sibling_fq =
        FullyQualifiedMoniker::compose(&zone_fq, &SegmentMoniker::from_string("ui:sibling"));
    reg.register_scope(leaf(
        lost_fq.clone(),
        "ui:lost",
        "/L",
        Some(zone_fq.clone()),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    reg.register_scope(leaf(
        sibling_fq.clone(),
        "ui:sibling",
        "/L",
        Some(zone_fq.clone()),
        rect(20.0, 0.0, 10.0, 10.0),
    ));

    // Run the registry path on a clone so we can re-run the snapshot
    // path against the same pre-unregister registry state.
    let mut reg_a = reg.clone();
    let mut state_a = SpatialState::new();
    state_a
        .focus(&mut reg_a, lost_fq.clone())
        .expect("focus lost on registry-path state");
    let registry_event = state_a
        .handle_unregister(&mut reg_a, &lost_fq, None)
        .expect("registry path emits event");

    // Snapshot path on a fresh clone: same registry state, same focus.
    let mut reg_b = reg;
    let mut state_b = SpatialState::new();
    state_b
        .focus(&mut reg_b, lost_fq.clone())
        .expect("focus lost on snapshot-path state");

    // Build the snapshot of the lost layer with the lost FQM omitted —
    // mirrors the React-side "already removed" shape.
    let snap = snapshot_excluding(&reg_b, &FullyQualifiedMoniker::from_string("/L"), &lost_fq);
    let idx = IndexedSnapshot::new(&snap);
    let ctx = LostFocusContext {
        view: &idx,
        lost_layer_fq: FullyQualifiedMoniker::from_string("/L"),
        lost_parent_zone: Some(zone_fq),
        lost_rect: rect(0.0, 0.0, 10.0, 10.0),
    };
    let snapshot_event = state_b
        .handle_unregister(&mut reg_b, &lost_fq, Some(&ctx))
        .expect("snapshot path emits event");

    assert_eq!(snapshot_event, registry_event);
    assert_eq!(snapshot_event.next_fq, Some(sibling_fq));
}
