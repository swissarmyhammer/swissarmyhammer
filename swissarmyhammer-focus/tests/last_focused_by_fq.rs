//! Integration tests pinning the top-level `last_focused_by_fq` map
//! and the dual-write semantics on `SpatialRegistry::record_focus`.
//!
//! Three properties are pinned:
//!
//! 1. After every focus mutation, the per-scope `FocusScope::last_focused`
//!    mirror and the top-level `last_focused_by_fq` map agree on the
//!    recorded descendant for every walked ancestor.
//! 2. Walking via a `NavSnapshot` (`record_focus(fq, Some(&snapshot))`)
//!    produces the same `last_focused_by_fq` writes as walking via the
//!    registry (`record_focus(fq, None)`).
//! 3. `resolve_fallback`'s `FallbackParentZoneLastFocused` arm consults
//!    `last_focused_by_fq` first and falls back to the per-scope mirror
//!    when the map has no entry.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    FallbackResolution, FocusLayer, FocusScope, FullyQualifiedMoniker, IndexedSnapshot, LayerName,
    NavSnapshot, Pixels, Rect, SegmentMoniker, SnapshotScope, SpatialRegistry, SpatialState,
    WindowLabel,
};

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        x: Pixels::new(x),
        y: Pixels::new(y),
        width: Pixels::new(w),
        height: Pixels::new(h),
    }
}

fn fq(parent: &str, segment: &str) -> FullyQualifiedMoniker {
    FullyQualifiedMoniker::from_string(format!("{parent}/{segment}"))
}

fn scope(
    fq_str: &str,
    segment: &str,
    layer: &str,
    parent_zone: Option<FullyQualifiedMoniker>,
    r: Rect,
) -> FocusScope {
    FocusScope {
        fq: FullyQualifiedMoniker::from_string(fq_str),
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone,
        last_focused: None,
        overrides: HashMap::new(),
    }
}

fn layer_node(fq_str: &str, segment: &str, window: &str, parent: Option<&str>) -> FocusLayer {
    FocusLayer {
        fq: FullyQualifiedMoniker::from_string(fq_str),
        segment: SegmentMoniker::from_string(segment),
        name: LayerName::from_string("window"),
        parent: parent.map(FullyQualifiedMoniker::from_string),
        window_label: WindowLabel::from_string(window),
        last_focused: None,
    }
}

/// Build a three-level registry: `/L` → `outer` → `inner` → `lost`
/// plus a sibling leaf `remembered` of `inner` directly under `outer`.
/// Returns `(reg, outer_fq, inner_fq, remembered_fq, lost_fq)`.
fn build_three_level_registry() -> (
    SpatialRegistry,
    FullyQualifiedMoniker,
    FullyQualifiedMoniker,
    FullyQualifiedMoniker,
    FullyQualifiedMoniker,
) {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer_node("/L", "L", "main", None));

    let outer_fq = fq("/L", "ui:outer");
    reg.register_scope(scope(
        outer_fq.as_ref(),
        "ui:outer",
        "/L",
        None,
        rect(0.0, 0.0, 500.0, 500.0),
    ));

    let inner_fq =
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:inner"));
    reg.register_scope(scope(
        inner_fq.as_ref(),
        "ui:inner",
        "/L",
        Some(outer_fq.clone()),
        rect(0.0, 0.0, 100.0, 100.0),
    ));

    let remembered_fq =
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:remembered"));
    reg.register_scope(scope(
        remembered_fq.as_ref(),
        "ui:remembered",
        "/L",
        Some(outer_fq.clone()),
        rect(400.0, 400.0, 10.0, 10.0),
    ));

    let lost_fq =
        FullyQualifiedMoniker::compose(&inner_fq, &SegmentMoniker::from_string("ui:lost"));
    reg.register_scope(scope(
        lost_fq.as_ref(),
        "ui:lost",
        "/L",
        Some(inner_fq.clone()),
        rect(0.0, 0.0, 10.0, 10.0),
    ));

    (reg, outer_fq, inner_fq, remembered_fq, lost_fq)
}

/// Walk every registered scope and assert the per-scope mirror and
/// the top-level map agree on what they remember (or both omit).
fn assert_stores_in_sync(reg: &SpatialRegistry) {
    for scope in reg.scopes_iter() {
        let map_value = reg.last_focused_by_fq.get(&scope.fq);
        match (&scope.last_focused, map_value) {
            (Some(per_scope), Some(top_level)) => {
                assert_eq!(
                    per_scope, top_level,
                    "stores diverge for ancestor {}: per_scope={:?} map={:?}",
                    scope.fq, per_scope, top_level,
                );
            }
            (None, None) => {}
            (per_scope, top_level) => {
                panic!(
                    "stores diverge for ancestor {}: per_scope={:?} map={:?}",
                    scope.fq, per_scope, top_level
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Property 1: dual-write keeps both stores synchronized
// ---------------------------------------------------------------------------

/// Focusing a leaf populates both the per-scope mirror and the
/// top-level map with the same `last_focused` value on every walked
/// ancestor scope.
#[test]
fn dual_write_synchronizes_on_focus() {
    let (mut reg, outer_fq, inner_fq, _remembered_fq, lost_fq) = build_three_level_registry();
    let mut state = SpatialState::new();

    state
        .focus(&mut reg, lost_fq.clone())
        .expect("focus emits an event");

    assert_eq!(
        reg.scope(&inner_fq).and_then(|s| s.last_focused.clone()),
        Some(lost_fq.clone()),
        "inner.last_focused (per-scope mirror) should be lost"
    );
    assert_eq!(
        reg.last_focused_by_fq.get(&inner_fq),
        Some(&lost_fq),
        "last_focused_by_fq[inner] should be lost"
    );
    assert_eq!(
        reg.scope(&outer_fq).and_then(|s| s.last_focused.clone()),
        Some(lost_fq.clone()),
        "outer.last_focused (per-scope mirror) should be lost"
    );
    assert_eq!(
        reg.last_focused_by_fq.get(&outer_fq),
        Some(&lost_fq),
        "last_focused_by_fq[outer] should be lost"
    );

    assert_stores_in_sync(&reg);
}

/// A second focus event overwrites both stores in lock-step on every
/// ancestor walked by the new focus path.
#[test]
fn dual_write_overwrites_on_subsequent_focus() {
    let (mut reg, outer_fq, _inner_fq, remembered_fq, lost_fq) = build_three_level_registry();
    let mut state = SpatialState::new();

    state
        .focus(&mut reg, lost_fq.clone())
        .expect("first focus emits");
    state
        .focus(&mut reg, remembered_fq.clone())
        .expect("second focus emits");

    // outer is an ancestor of both lost and remembered, so the second
    // focus should overwrite both stores.
    assert_eq!(
        reg.scope(&outer_fq).and_then(|s| s.last_focused.clone()),
        Some(remembered_fq.clone()),
    );
    assert_eq!(reg.last_focused_by_fq.get(&outer_fq), Some(&remembered_fq),);
    assert_stores_in_sync(&reg);
}

/// Empty registry has both stores empty.
#[test]
fn empty_registry_has_empty_stores() {
    let reg = SpatialRegistry::new();
    assert!(reg.last_focused_by_fq.is_empty());
    assert_stores_in_sync(&reg);
}

// ---------------------------------------------------------------------------
// Property 2: snapshot-walk parity with registry-walk
// ---------------------------------------------------------------------------

/// Build a snapshot whose `parent_zone` chain matches the registry
/// produced by `build_three_level_registry`.
fn snapshot_matching_three_level_registry(
    outer_fq: &FullyQualifiedMoniker,
    inner_fq: &FullyQualifiedMoniker,
    remembered_fq: &FullyQualifiedMoniker,
    lost_fq: &FullyQualifiedMoniker,
) -> NavSnapshot {
    NavSnapshot {
        layer_fq: FullyQualifiedMoniker::from_string("/L"),
        scopes: vec![
            SnapshotScope {
                fq: outer_fq.clone(),
                rect: rect(0.0, 0.0, 500.0, 500.0),
                parent_zone: None,
                nav_override: HashMap::new(),
            },
            SnapshotScope {
                fq: inner_fq.clone(),
                rect: rect(0.0, 0.0, 100.0, 100.0),
                parent_zone: Some(outer_fq.clone()),
                nav_override: HashMap::new(),
            },
            SnapshotScope {
                fq: remembered_fq.clone(),
                rect: rect(400.0, 400.0, 10.0, 10.0),
                parent_zone: Some(outer_fq.clone()),
                nav_override: HashMap::new(),
            },
            SnapshotScope {
                fq: lost_fq.clone(),
                rect: rect(0.0, 0.0, 10.0, 10.0),
                parent_zone: Some(inner_fq.clone()),
                nav_override: HashMap::new(),
            },
        ],
    }
}

/// Snapshot-walk and registry-walk produce identical
/// `last_focused_by_fq` maps when the snapshot mirrors the registry's
/// scope structure.
#[test]
fn snapshot_walk_parity_with_registry_walk() {
    let (mut reg_a, outer_fq, inner_fq, remembered_fq, lost_fq) = build_three_level_registry();
    let (mut reg_b, _, _, _, _) = build_three_level_registry();

    // Registry-walk variant.
    reg_a.record_focus(&lost_fq, None);

    // Snapshot-walk variant on a sibling registry.
    let snap =
        snapshot_matching_three_level_registry(&outer_fq, &inner_fq, &remembered_fq, &lost_fq);
    let idx = IndexedSnapshot::new(&snap);
    reg_b.record_focus(&lost_fq, Some(&idx));

    assert_eq!(
        reg_a.last_focused_by_fq, reg_b.last_focused_by_fq,
        "snapshot-walk and registry-walk should produce identical last_focused_by_fq",
    );
    // Both should have written outer + inner.
    assert_eq!(reg_a.last_focused_by_fq.get(&outer_fq), Some(&lost_fq),);
    assert_eq!(reg_a.last_focused_by_fq.get(&inner_fq), Some(&lost_fq),);
    assert_stores_in_sync(&reg_a);
    assert_stores_in_sync(&reg_b);
}

/// Snapshot-walk with an ancestor that is NOT registered in `scopes`
/// still writes to `last_focused_by_fq` (the map is the authoritative
/// store; the per-scope mirror gracefully no-ops).
#[test]
fn snapshot_walk_writes_map_when_ancestor_absent_from_registry() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer_node("/L", "L", "main", None));
    // Only `lost` is registered in the registry — its ancestors live in
    // the snapshot but not in `reg.scopes`.
    let outer_fq = fq("/L", "ui:outer");
    let inner_fq =
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:inner"));
    let lost_fq =
        FullyQualifiedMoniker::compose(&inner_fq, &SegmentMoniker::from_string("ui:lost"));
    reg.register_scope(scope(
        lost_fq.as_ref(),
        "ui:lost",
        "/L",
        Some(inner_fq.clone()),
        rect(0.0, 0.0, 10.0, 10.0),
    ));

    let snap = NavSnapshot {
        layer_fq: FullyQualifiedMoniker::from_string("/L"),
        scopes: vec![
            SnapshotScope {
                fq: outer_fq.clone(),
                rect: rect(0.0, 0.0, 500.0, 500.0),
                parent_zone: None,
                nav_override: HashMap::new(),
            },
            SnapshotScope {
                fq: inner_fq.clone(),
                rect: rect(0.0, 0.0, 100.0, 100.0),
                parent_zone: Some(outer_fq.clone()),
                nav_override: HashMap::new(),
            },
            SnapshotScope {
                fq: lost_fq.clone(),
                rect: rect(0.0, 0.0, 10.0, 10.0),
                parent_zone: Some(inner_fq.clone()),
                nav_override: HashMap::new(),
            },
        ],
    };
    let idx = IndexedSnapshot::new(&snap);
    reg.record_focus(&lost_fq, Some(&idx));

    assert_eq!(
        reg.last_focused_by_fq.get(&outer_fq),
        Some(&lost_fq),
        "snapshot-walk should populate the map even when ancestor is not in scopes",
    );
    assert_eq!(reg.last_focused_by_fq.get(&inner_fq), Some(&lost_fq),);
    // The per-scope mirror could not be written for outer/inner (they
    // are not in `reg.scopes`), but lost itself is unaffected (the
    // focused FQM never writes to its own slot anyway).
    assert!(reg.scope(&outer_fq).is_none());
    assert!(reg.scope(&inner_fq).is_none());
}

// ---------------------------------------------------------------------------
// Property 3: resolve_fallback read precedence — map first, mirror second
// ---------------------------------------------------------------------------
//
// To prove read precedence we deliberately stake the two stores at
// *different* values for the same ancestor. The dual-write keeps them
// in sync in production; these tests violate the invariant on purpose
// to observe which slot the resolver consults.

/// Two distinct candidate FQMs registered as siblings of `inner` under
/// `outer`. Used so the precedence tests can stake `last_focused_by_fq`
/// at one and the per-scope mirror at the other and read out the
/// resolver's choice unambiguously.
fn build_two_candidate_registry() -> (
    SpatialRegistry,
    FullyQualifiedMoniker, // outer
    FullyQualifiedMoniker, // map_target  (sibling A)
    FullyQualifiedMoniker, // mirror_target (sibling B)
    FullyQualifiedMoniker, // lost
) {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer_node("/L", "L", "main", None));

    let outer_fq = fq("/L", "ui:outer");
    reg.register_scope(scope(
        outer_fq.as_ref(),
        "ui:outer",
        "/L",
        None,
        rect(0.0, 0.0, 500.0, 500.0),
    ));

    let map_target =
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:map-target"));
    reg.register_scope(scope(
        map_target.as_ref(),
        "ui:map-target",
        "/L",
        Some(outer_fq.clone()),
        rect(400.0, 400.0, 10.0, 10.0),
    ));

    let mirror_target =
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:mirror-target"));
    reg.register_scope(scope(
        mirror_target.as_ref(),
        "ui:mirror-target",
        "/L",
        Some(outer_fq.clone()),
        rect(450.0, 450.0, 10.0, 10.0),
    ));

    let inner_fq =
        FullyQualifiedMoniker::compose(&outer_fq, &SegmentMoniker::from_string("ui:inner"));
    reg.register_scope(scope(
        inner_fq.as_ref(),
        "ui:inner",
        "/L",
        Some(outer_fq.clone()),
        rect(0.0, 0.0, 100.0, 100.0),
    ));

    let lost_fq =
        FullyQualifiedMoniker::compose(&inner_fq, &SegmentMoniker::from_string("ui:lost"));
    reg.register_scope(scope(
        lost_fq.as_ref(),
        "ui:lost",
        "/L",
        Some(inner_fq.clone()),
        rect(0.0, 0.0, 10.0, 10.0),
    ));

    (reg, outer_fq, map_target, mirror_target, lost_fq)
}

/// When the map and per-scope mirror disagree, the resolver picks the
/// map's value — establishing read precedence.
#[test]
fn resolve_fallback_prefers_map_over_per_scope_mirror() {
    let (mut reg, outer_fq, map_target, mirror_target, lost_fq) = build_two_candidate_registry();
    let mut state = SpatialState::new();

    // Drive focus to lost so the resolver has a window to work with.
    // record_focus will populate both stores at `lost`'s ancestors;
    // we stake the divergence after.
    state.focus(&mut reg, lost_fq.clone()).unwrap();

    // Stake the per-scope mirror at `mirror_target` by re-registering
    // outer with that field preset (same-shape re-register preserves
    // the user-supplied last_focused).
    let mut outer_replacement = scope(
        outer_fq.as_ref(),
        "ui:outer",
        "/L",
        None,
        rect(0.0, 0.0, 500.0, 500.0),
    );
    outer_replacement.last_focused = Some(mirror_target.clone());
    reg.register_scope(outer_replacement);

    // Stake the map at `map_target`.
    reg.last_focused_by_fq
        .insert(outer_fq.clone(), map_target.clone());

    // Sanity: stores genuinely disagree.
    assert_eq!(
        reg.scope(&outer_fq).unwrap().last_focused,
        Some(mirror_target.clone()),
    );
    assert_eq!(reg.last_focused_by_fq.get(&outer_fq), Some(&map_target));

    let resolution = state.resolve_fallback(&reg, &lost_fq);
    match resolution {
        FallbackResolution::FallbackParentZoneLastFocused(target_fq, _) => {
            assert_eq!(
                target_fq, map_target,
                "resolver should pick the map's value over the per-scope mirror",
            );
        }
        other => panic!("expected FallbackParentZoneLastFocused via map, got {other:?}",),
    }
}

/// When the map has no entry but the per-scope mirror is populated,
/// the resolver falls back to the mirror — confirming the second tier
/// of the precedence chain.
#[test]
fn resolve_fallback_falls_back_to_per_scope_mirror_when_map_empty() {
    let (mut reg, outer_fq, _map_target, mirror_target, lost_fq) = build_two_candidate_registry();
    let mut state = SpatialState::new();

    state.focus(&mut reg, lost_fq.clone()).unwrap();

    // Stake the per-scope mirror at `mirror_target`.
    let mut outer_replacement = scope(
        outer_fq.as_ref(),
        "ui:outer",
        "/L",
        None,
        rect(0.0, 0.0, 500.0, 500.0),
    );
    outer_replacement.last_focused = Some(mirror_target.clone());
    reg.register_scope(outer_replacement);

    // Clear the map entry the focus call wrote.
    reg.last_focused_by_fq.remove(&outer_fq);

    // Sanity.
    assert!(!reg.last_focused_by_fq.contains_key(&outer_fq));
    assert_eq!(
        reg.scope(&outer_fq).unwrap().last_focused,
        Some(mirror_target.clone()),
    );

    let resolution = state.resolve_fallback(&reg, &lost_fq);
    match resolution {
        FallbackResolution::FallbackParentZoneLastFocused(target_fq, _) => {
            assert_eq!(
                target_fq, mirror_target,
                "resolver should fall back to per-scope mirror when map has no entry",
            );
        }
        other => {
            panic!("expected FallbackParentZoneLastFocused via per-scope mirror, got {other:?}",)
        }
    }
}
