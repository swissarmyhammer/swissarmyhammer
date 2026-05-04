//! Integration tests for the spatial focus registry kernel.
//!
//! Headless pattern matching `tests/focus_state.rs` — pure Rust, no Tauri
//! runtime, no jsdom. Every registry operation runs through the public
//! surface and is asserted by inspecting return values plus subsequent
//! reads. Exercises the unified-primitive surface: `find_by_fq`,
//! `scopes_in_layer`, `children_of`, `has_children`, `ancestor_zones`,
//! `children_of_layer`, `root_for_window`, `ancestors_of_layer`.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    Direction, FocusLayer, FocusScope, FullyQualifiedMoniker, LayerName,
    Pixels, Rect, SegmentMoniker, SpatialRegistry, WindowLabel,
};

// ---------------------------------------------------------------------------
// Pixels arithmetic
// ---------------------------------------------------------------------------

/// `Pixels` supports `+` and `-` between `Pixels` values, returning `Pixels`
/// (not `f64`). Beam-search and rect math must stay in newtype-land so a
/// stray `.0` doesn't drop type-safety on the floor.
#[test]
fn pixels_arithmetic_is_type_preserving() {
    let a = Pixels::new(10.0);
    let b = Pixels::new(3.5);

    let sum: Pixels = a + b;
    let diff: Pixels = a - b;
    let scaled: Pixels = a * 2.0;
    let halved: Pixels = a / 2.0;

    assert_eq!(sum, Pixels::new(13.5));
    assert_eq!(diff, Pixels::new(6.5));
    assert_eq!(scaled, Pixels::new(20.0));
    assert_eq!(halved, Pixels::new(5.0));
}

/// `Pixels` serializes transparently as a bare number — the wire shape is
/// just `13.5`, not `{"0": 13.5}`. Mirrors the `define_id!` `transparent`
/// shape used by string-valued newtypes.
#[test]
fn pixels_serializes_as_bare_number() {
    let p = Pixels::new(13.5);
    assert_eq!(serde_json::to_string(&p).unwrap(), "13.5");

    let back: Pixels = serde_json::from_str("42.0").unwrap();
    assert_eq!(back, Pixels::new(42.0));
}

// ---------------------------------------------------------------------------
// Rect helpers
// ---------------------------------------------------------------------------

/// `Rect` exposes `top`, `left`, `bottom`, `right` accessors that derive
/// from the stored `x`, `y`, `width`, `height` without `.0` arithmetic at
/// the call site.
#[test]
fn rect_edges_compute_from_origin_and_size() {
    let r = Rect {
        x: Pixels::new(10.0),
        y: Pixels::new(20.0),
        width: Pixels::new(100.0),
        height: Pixels::new(50.0),
    };

    assert_eq!(r.left(), Pixels::new(10.0));
    assert_eq!(r.top(), Pixels::new(20.0));
    assert_eq!(r.right(), Pixels::new(110.0));
    assert_eq!(r.bottom(), Pixels::new(70.0));
}

// ---------------------------------------------------------------------------
// FocusScope / FocusScope round-trip
// ---------------------------------------------------------------------------

/// [`FocusScope`] and [`FocusScope`] each round-trip through serde without
/// the help of a wrapping enum. Three-peer model: there is no public
/// sum-type that conflates leaves and zones — each struct is its own
/// JSON shape.
#[test]
fn focus_scope_and_zone_round_trip_independently() {
    let leaf = FocusScope {
        fq: FullyQualifiedMoniker::from_string("/L1/leaf"),
        segment: SegmentMoniker::from_string("leaf"),
        rect: Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        },
        layer_fq: FullyQualifiedMoniker::from_string("/L1"),
        parent_zone: None,
        overrides: HashMap::new(),
        last_focused: None,
    };

    let zone = FocusScope {
        fq: FullyQualifiedMoniker::from_string("/L1/zone"),
        segment: SegmentMoniker::from_string("zone"),
        rect: Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        },
        layer_fq: FullyQualifiedMoniker::from_string("/L1"),
        parent_zone: None,
        last_focused: None,
        overrides: HashMap::new(),
    };

    let leaf_json = serde_json::to_value(&leaf).unwrap();
    let zone_json = serde_json::to_value(&zone).unwrap();

    let leaf_back: FocusScope = serde_json::from_value(leaf_json).unwrap();
    let zone_back: FocusScope = serde_json::from_value(zone_json).unwrap();

    assert_eq!(leaf_back.segment, leaf.segment);
    assert_eq!(zone_back.segment, zone.segment);
    assert_eq!(zone_back.last_focused, None);
}

// ---------------------------------------------------------------------------
// Registry — round-trip register/lookup
// ---------------------------------------------------------------------------

fn make_focus_scope(fq: &str, segment: &str, layer: &str, parent_zone: Option<&str>) -> FocusScope {
    FocusScope {
        fq: FullyQualifiedMoniker::from_string(fq),
        segment: SegmentMoniker::from_string(segment),
        rect: Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        },
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone: parent_zone.map(FullyQualifiedMoniker::from_string),
        overrides: HashMap::new(),
        last_focused: None,
    }
}

fn make_zone(fq: &str, segment: &str, layer: &str, parent_zone: Option<&str>) -> FocusScope {
    FocusScope {
        fq: FullyQualifiedMoniker::from_string(fq),
        segment: SegmentMoniker::from_string(segment),
        rect: Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(100.0),
            height: Pixels::new(100.0),
        },
        layer_fq: FullyQualifiedMoniker::from_string(layer),
        parent_zone: parent_zone.map(FullyQualifiedMoniker::from_string),
        last_focused: None,
        overrides: HashMap::new(),
    }
}

fn make_layer(fq: &str, name: &str, window: &str, parent: Option<&str>) -> FocusLayer {
    FocusLayer {
        fq: FullyQualifiedMoniker::from_string(fq),
        segment: SegmentMoniker::from_string(name),
        name: LayerName::from_string(name),
        parent: parent.map(FullyQualifiedMoniker::from_string),
        window_label: WindowLabel::from_string(window),
        last_focused: None,
    }
}

/// Registering a [`FocusScope`] and a [`FocusScope`] under different FQMs
/// stores both leaf and container scopes in the same FQM-indexed map.
/// `scope` and `find_by_fq` both look up by FQM and yield the same
/// `&FocusScope` regardless of whether the entry has children or not —
/// container vs leaf is decided at runtime by [`SpatialRegistry::has_children`].
#[test]
fn registry_returns_scope_for_each_registration() {
    let mut reg = SpatialRegistry::new();
    let leaf = make_focus_scope("/L1/leaf", "leaf", "/L1", None);
    let container = make_zone("/L1/container", "container", "/L1", None);
    let inner = make_focus_scope("/L1/container/inner", "inner", "/L1", Some("/L1/container"));
    reg.register_scope(leaf);
    reg.register_scope(container);
    reg.register_scope(inner);

    let leaf_fq = FullyQualifiedMoniker::from_string("/L1/leaf");
    let container_fq = FullyQualifiedMoniker::from_string("/L1/container");

    assert!(reg.scope(&leaf_fq).is_some());
    assert!(reg.find_by_fq(&leaf_fq).is_some());
    assert!(reg.scope(&container_fq).is_some());
    assert!(reg.find_by_fq(&container_fq).is_some());

    // `has_children` distinguishes the two roles structurally.
    assert!(!reg.has_children(&leaf_fq));
    assert!(reg.has_children(&container_fq));

    // Both FQMs are registered (presence check).
    assert!(reg.is_registered(&leaf_fq));
    assert!(reg.is_registered(&container_fq));
    assert!(!reg.is_registered(&FullyQualifiedMoniker::from_string("/ghost")));
}

/// `update_rect` mutates the stored rect of a registered scope without
/// changing its variant or other fields.
#[test]
fn update_rect_preserves_variant_and_other_fields() {
    let mut reg = SpatialRegistry::new();
    let leaf = make_focus_scope("/L1/k", "k", "/L1", Some("/L1/parent"));
    reg.register_scope(leaf);

    let new_rect = Rect {
        x: Pixels::new(5.0),
        y: Pixels::new(6.0),
        width: Pixels::new(7.0),
        height: Pixels::new(8.0),
    };
    let fq = FullyQualifiedMoniker::from_string("/L1/k");
    reg.update_rect(&fq, new_rect);

    let scope = reg.scope(&fq).unwrap();
    assert_eq!(scope.rect, new_rect);
    assert_eq!(
        scope.parent_zone,
        Some(FullyQualifiedMoniker::from_string("/L1/parent"))
    );
}

/// `unregister_scope` removes the scope from the map.
#[test]
fn unregister_removes_scope() {
    let mut reg = SpatialRegistry::new();
    let leaf = make_focus_scope("/L1/k", "k", "/L1", None);
    reg.register_scope(leaf);
    let fq = FullyQualifiedMoniker::from_string("/L1/k");
    assert!(reg.is_registered(&fq));

    reg.unregister_scope(&fq);
    assert!(!reg.is_registered(&fq));
}

/// `find_by_fq` resolves a [`FullyQualifiedMoniker`] to its registered
/// entry. The kernel's path-as-key contract means the FQM IS the
/// registry key; lookups are exact-match.
#[test]
fn find_by_fq_resolves_to_registered_entry() {
    let mut reg = SpatialRegistry::new();
    reg.register_scope(make_focus_scope(
        "/L1/task:01ABC",
        "task:01ABC",
        "/L1",
        None,
    ));
    reg.register_scope(make_zone("/L1/column:doing", "column:doing", "/L1", None));

    let leaf_entry = reg
        .find_by_fq(&FullyQualifiedMoniker::from_string("/L1/task:01ABC"))
        .expect("leaf FQM resolves to its registered entry");
    assert_eq!(
        leaf_entry.segment,
        SegmentMoniker::from_string("task:01ABC")
    );

    let zone_entry = reg
        .find_by_fq(&FullyQualifiedMoniker::from_string("/L1/column:doing"))
        .expect("zone FQM resolves to its registered entry");
    assert_eq!(
        zone_entry.segment,
        SegmentMoniker::from_string("column:doing")
    );
}

/// `find_by_fq` returns `None` for an unregistered FQM. Higher-level
/// callers translate this to `tracing::error!` per the
/// no-silent-dropout contract.
#[test]
fn find_by_fq_unknown_returns_none() {
    let mut reg = SpatialRegistry::new();
    reg.register_scope(make_focus_scope("/L1/task:01", "task:01", "/L1", None));

    assert!(reg
        .find_by_fq(&FullyQualifiedMoniker::from_string(
            "/L1/task:does-not-exist"
        ))
        .is_none());
}

/// Registering the same FQM twice with the same structural shape (only
/// the `rect` differs) replaces the prior entry silently. This is the
/// placeholder→real-mount swap path, which is part of the normal
/// virtualizer + spatial-nav lifecycle: the column placeholder hook
/// registers a rect estimate, and the `<EntityCard>` `<FocusScope>`
/// later registers its real `getBoundingClientRect()` at the same FQM.
/// Same-shape re-registers must NOT trip a programmer-mistake error —
/// see `register_scope` docstring for the full rationale.
#[test]
fn duplicate_fq_registration_replaces_prior_entry() {
    let mut reg = SpatialRegistry::new();
    let path = "/L1/task:duplicate";

    let mut first = make_focus_scope(path, "task:duplicate", "/L1", None);
    first.rect = Rect {
        x: Pixels::new(1.0),
        y: Pixels::new(1.0),
        width: Pixels::new(10.0),
        height: Pixels::new(10.0),
    };
    reg.register_scope(first);

    let mut second = make_focus_scope(path, "task:duplicate", "/L1", None);
    second.rect = Rect {
        x: Pixels::new(99.0),
        y: Pixels::new(99.0),
        width: Pixels::new(10.0),
        height: Pixels::new(10.0),
    };
    reg.register_scope(second);

    let resolved = reg
        .scope(&FullyQualifiedMoniker::from_string(path))
        .expect("duplicate FQM still resolves to one registered entry");
    assert_eq!(resolved.rect.x, Pixels::new(99.0));
}

// ---------------------------------------------------------------------------
// Zone tree ops
// ---------------------------------------------------------------------------

/// `children_of_zone` returns direct children only — a grandchild whose
/// `parent_zone` points at a different zone must NOT show up. The
/// returned [`ChildScope`] view distinguishes leaf vs container children
/// without exposing the registry's internal enum.
#[test]
fn children_of_zone_returns_direct_children_only() {
    let mut reg = SpatialRegistry::new();
    // outer zone → inner zone → leaf
    reg.register_scope(make_zone("/L1/outer", "outer", "/L1", None));
    reg.register_scope(make_zone(
        "/L1/outer/inner",
        "inner",
        "/L1",
        Some("/L1/outer"),
    ));
    reg.register_scope(make_focus_scope(
        "/L1/outer/leaf-outer",
        "leaf-outer",
        "/L1",
        Some("/L1/outer"),
    ));
    reg.register_scope(make_focus_scope(
        "/L1/outer/inner/leaf-inner",
        "leaf-inner",
        "/L1",
        Some("/L1/outer/inner"),
    ));

    let outer_kids: Vec<&FullyQualifiedMoniker> = reg
        .children_of(&FullyQualifiedMoniker::from_string("/L1/outer"))
        .map(|c| &c.fq)
        .collect();
    assert_eq!(
        outer_kids.len(),
        2,
        "outer has 2 direct children: inner zone + leaf-outer"
    );
    assert!(outer_kids.contains(&&FullyQualifiedMoniker::from_string("/L1/outer/inner")));
    assert!(outer_kids.contains(&&FullyQualifiedMoniker::from_string("/L1/outer/leaf-outer")));
    assert!(
        !outer_kids.contains(&&FullyQualifiedMoniker::from_string(
            "/L1/outer/inner/leaf-inner"
        )),
        "leaf-inner is a grandchild — must not show up"
    );
}

/// `ancestor_zones` walks `parent_zone` from the focused leaf up through
/// each ancestor zone, in order from nearest to farthest.
#[test]
fn ancestor_zones_walks_parent_chain_innermost_first() {
    let mut reg = SpatialRegistry::new();
    reg.register_scope(make_zone("/L1/outer", "outer", "/L1", None));
    reg.register_scope(make_zone(
        "/L1/outer/middle",
        "middle",
        "/L1",
        Some("/L1/outer"),
    ));
    reg.register_scope(make_zone(
        "/L1/outer/middle/inner",
        "inner",
        "/L1",
        Some("/L1/outer/middle"),
    ));
    reg.register_scope(make_focus_scope(
        "/L1/outer/middle/inner/leaf",
        "leaf",
        "/L1",
        Some("/L1/outer/middle/inner"),
    ));

    let chain: Vec<&FullyQualifiedMoniker> = reg
        .ancestor_zones(&FullyQualifiedMoniker::from_string(
            "/L1/outer/middle/inner/leaf",
        ))
        .into_iter()
        .map(|z| &z.fq)
        .collect();

    assert_eq!(
        chain,
        vec![
            &FullyQualifiedMoniker::from_string("/L1/outer/middle/inner"),
            &FullyQualifiedMoniker::from_string("/L1/outer/middle"),
            &FullyQualifiedMoniker::from_string("/L1/outer"),
        ]
    );
}

/// `ancestor_zones` for a leaf with no `parent_zone` returns an empty
/// vector — the leaf itself is at the layer root.
#[test]
fn ancestor_zones_at_layer_root_is_empty() {
    let mut reg = SpatialRegistry::new();
    reg.register_scope(make_focus_scope("/L1/leaf", "leaf", "/L1", None));

    assert!(reg
        .ancestor_zones(&FullyQualifiedMoniker::from_string("/L1/leaf"))
        .is_empty());
}

// ---------------------------------------------------------------------------
// Layer forest ops
// ---------------------------------------------------------------------------

/// `children_of_layer` returns layers whose `parent` matches the queried
/// FQM. Layers belonging to other windows or other parents are filtered
/// out.
#[test]
fn children_of_layer_filters_by_parent_not_cross_window() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/win-a", "window", "win-a", None));
    reg.push_layer(make_layer("/win-b", "window", "win-b", None));
    reg.push_layer(make_layer(
        "/win-a/inspector",
        "inspector",
        "win-a",
        Some("/win-a"),
    ));
    reg.push_layer(make_layer(
        "/win-b/inspector",
        "inspector",
        "win-b",
        Some("/win-b"),
    ));
    reg.push_layer(make_layer(
        "/win-a/inspector/dialog",
        "dialog",
        "win-a",
        Some("/win-a/inspector"),
    ));

    let kids_a: Vec<&FullyQualifiedMoniker> = reg
        .children_of_layer(&FullyQualifiedMoniker::from_string("/win-a"))
        .into_iter()
        .map(|l| &l.fq)
        .collect();

    assert_eq!(
        kids_a.len(),
        1,
        "/win-a has exactly one child layer (the inspector)"
    );
    assert_eq!(
        kids_a[0],
        &FullyQualifiedMoniker::from_string("/win-a/inspector")
    );
}

/// `root_for_window` returns the layer with the matching `window_label`
/// and `parent == None`.
#[test]
fn root_for_window_returns_window_root_only() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/win-a", "window", "win-a", None));
    reg.push_layer(make_layer(
        "/win-a/inspector",
        "inspector",
        "win-a",
        Some("/win-a"),
    ));
    reg.push_layer(make_layer("/win-b", "window", "win-b", None));

    let root_a = reg
        .root_for_window(&WindowLabel::from_string("win-a"))
        .unwrap();
    assert_eq!(root_a.fq, FullyQualifiedMoniker::from_string("/win-a"));

    let root_b = reg
        .root_for_window(&WindowLabel::from_string("win-b"))
        .unwrap();
    assert_eq!(root_b.fq, FullyQualifiedMoniker::from_string("/win-b"));

    assert!(reg
        .root_for_window(&WindowLabel::from_string("ghost"))
        .is_none());
}

/// `ancestors_of_layer` walks `layer.parent` from the queried layer up to
/// the window root, innermost first.
#[test]
fn ancestors_of_layer_walks_parent_chain() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/win-a", "window", "win-a", None));
    reg.push_layer(make_layer(
        "/win-a/ins",
        "inspector",
        "win-a",
        Some("/win-a"),
    ));
    reg.push_layer(make_layer(
        "/win-a/ins/dlg",
        "dialog",
        "win-a",
        Some("/win-a/ins"),
    ));

    let chain: Vec<&FullyQualifiedMoniker> = reg
        .ancestors_of_layer(&FullyQualifiedMoniker::from_string("/win-a/ins/dlg"))
        .into_iter()
        .map(|l| &l.fq)
        .collect();

    assert_eq!(
        chain,
        vec![
            &FullyQualifiedMoniker::from_string("/win-a/ins"),
            &FullyQualifiedMoniker::from_string("/win-a"),
        ]
    );
}

/// `remove_layer` deletes the layer; subsequent `layer(fq)` returns
/// `None`.
#[test]
fn remove_layer_drops_the_entry() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", "window", "win-a", None));
    let fq = FullyQualifiedMoniker::from_string("/L");
    assert!(reg.layer(&fq).is_some());

    reg.remove_layer(&fq);
    assert!(reg.layer(&fq).is_none());
}

/// `scopes_in_layer` returns scopes filtered by `layer_fq`. Scopes in
/// other layers are excluded — beam search and ancestor walks rely on
/// this layer-bounded view.
#[test]
fn scopes_in_layer_filter_by_layer_fq() {
    let mut reg = SpatialRegistry::new();
    reg.register_scope(make_focus_scope("/L1/leaf-1", "leaf-1", "/L1", None));
    reg.register_scope(make_zone("/L1/zone-1", "zone-1", "/L1", None));
    reg.register_scope(make_focus_scope("/L2/leaf-2", "leaf-2", "/L2", None));

    let l1 = FullyQualifiedMoniker::from_string("/L1");
    let l2 = FullyQualifiedMoniker::from_string("/L2");

    let mut l1_fqs: Vec<&FullyQualifiedMoniker> =
        reg.scopes_in_layer(&l1).map(|s| &s.fq).collect();
    l1_fqs.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    assert_eq!(
        l1_fqs,
        vec![
            &FullyQualifiedMoniker::from_string("/L1/leaf-1"),
            &FullyQualifiedMoniker::from_string("/L1/zone-1"),
        ]
    );

    let l2_fqs: Vec<&FullyQualifiedMoniker> =
        reg.scopes_in_layer(&l2).map(|s| &s.fq).collect();
    assert_eq!(
        l2_fqs,
        vec![&FullyQualifiedMoniker::from_string("/L2/leaf-2")]
    );
}

// ---------------------------------------------------------------------------
// Forest scenario — 2 windows + 2 inspectors + 1 dialog = 5 layers, 2 roots
// ---------------------------------------------------------------------------

/// Build a realistic forest: two windows (each with a root layer and an
/// inspector layer), plus a dialog layer stacked on one of the inspectors.
#[test]
fn forest_with_two_windows_and_stacked_overlays() {
    let mut reg = SpatialRegistry::new();

    // Two windows, each with a root layer.
    reg.push_layer(make_layer("/win-a", "window", "win-a", None));
    reg.push_layer(make_layer("/win-b", "window", "win-b", None));

    // Each window has an inspector overlay.
    reg.push_layer(make_layer(
        "/win-a/ins",
        "inspector",
        "win-a",
        Some("/win-a"),
    ));
    reg.push_layer(make_layer(
        "/win-b/ins",
        "inspector",
        "win-b",
        Some("/win-b"),
    ));

    // Window A has a dialog stacked on top of its inspector.
    reg.push_layer(make_layer(
        "/win-a/ins/dlg",
        "dialog",
        "win-a",
        Some("/win-a/ins"),
    ));

    let root_a = reg
        .root_for_window(&WindowLabel::from_string("win-a"))
        .unwrap();
    let root_b = reg
        .root_for_window(&WindowLabel::from_string("win-b"))
        .unwrap();
    assert_eq!(root_a.fq, FullyQualifiedMoniker::from_string("/win-a"));
    assert_eq!(root_b.fq, FullyQualifiedMoniker::from_string("/win-b"));

    let dlg_ancestors: Vec<&FullyQualifiedMoniker> = reg
        .ancestors_of_layer(&FullyQualifiedMoniker::from_string("/win-a/ins/dlg"))
        .into_iter()
        .map(|l| &l.fq)
        .collect();
    assert_eq!(
        dlg_ancestors,
        vec![
            &FullyQualifiedMoniker::from_string("/win-a/ins"),
            &FullyQualifiedMoniker::from_string("/win-a"),
        ]
    );

    let root_a_kids: Vec<&FullyQualifiedMoniker> = reg
        .children_of_layer(&FullyQualifiedMoniker::from_string("/win-a"))
        .into_iter()
        .map(|l| &l.fq)
        .collect();
    assert_eq!(
        root_a_kids,
        vec![&FullyQualifiedMoniker::from_string("/win-a/ins")]
    );
}

// ---------------------------------------------------------------------------
// Overrides round-trip on FocusScope
// ---------------------------------------------------------------------------

/// `FocusScope.overrides` map round-trips through serde with `Direction` as
/// the JSON key. `None` value preserves an explicit "wall" override (block
/// nav in this direction); `Some(FullyQualifiedMoniker)` is an explicit
/// redirect.
#[test]
fn focus_scope_overrides_round_trip_with_direction_keys() {
    let mut overrides: HashMap<Direction, Option<FullyQualifiedMoniker>> = HashMap::new();
    overrides.insert(
        Direction::Up,
        Some(FullyQualifiedMoniker::from_string("/L/above")),
    );
    overrides.insert(Direction::Left, None);

    let f = FocusScope {
        fq: FullyQualifiedMoniker::from_string("/L/k"),
        segment: SegmentMoniker::from_string("k"),
        rect: Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(1.0),
            height: Pixels::new(1.0),
        },
        layer_fq: FullyQualifiedMoniker::from_string("/L"),
        parent_zone: None,
        overrides,
        last_focused: None,
    };

    let json = serde_json::to_value(&f).unwrap();
    let back: FocusScope = serde_json::from_value(json).unwrap();

    assert_eq!(back.overrides.len(), 2);
    assert_eq!(
        back.overrides.get(&Direction::Up),
        Some(&Some(FullyQualifiedMoniker::from_string("/L/above")))
    );
    assert_eq!(back.overrides.get(&Direction::Left), Some(&None));
}
