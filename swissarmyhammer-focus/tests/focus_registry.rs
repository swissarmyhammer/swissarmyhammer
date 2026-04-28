//! Integration tests for the spatial focus registry kernel.
//!
//! Headless pattern matching `tests/resolve_focused_column.rs` and
//! `tests/focus_state.rs` — pure Rust, no Tauri runtime, no jsdom. Every
//! registry operation runs through the public surface and is asserted by
//! inspecting return values plus subsequent reads.
//!
//! These tests cover the kernel-types card (`01KNQXW7HH...`):
//!
//! - `Pixels` arithmetic is type-preserving (no `.0` access required for
//!   `+`, `-`, `*`, `/`).
//! - `FocusScope` (leaves) and `FocusZone` (containers) JSON-round-trip
//!   independently. There is no public sum-type enum that conflates them
//!   — the registry stores the discriminator internally.
//! - `SpatialRegistry` stores both [`FocusScope`] leaves and [`FocusZone`]
//!   containers behind a single [`SpatialKey`] map; typed accessors
//!   (`scope`, `zone`) return only the matching variant.
//! - Zone tree ops (`children_of_zone`, `ancestor_zones`) walk the
//!   `parent_zone` chain inside a layer.
//! - Layer forest ops (`children_of_layer`, `root_for_window`,
//!   `ancestors_of_layer`) walk the `layer.parent` chain across windows.
//! - `leaves_in_layer` / `zones_in_layer` return typed structs filtered
//!   by `layer_key`.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    ChildScope, Direction, FocusLayer, FocusScope, FocusZone, LayerKey, LayerName, Moniker, Pixels,
    Rect, SpatialKey, SpatialRegistry, WindowLabel,
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
// FocusScope / FocusZone round-trip
// ---------------------------------------------------------------------------

/// [`FocusScope`] and [`FocusZone`] each round-trip through serde without
/// the help of a wrapping enum. Three-peer model: there is no public
/// sum-type that conflates leaves and zones — each struct is its own
/// JSON shape.
#[test]
fn focus_scope_and_zone_round_trip_independently() {
    let leaf = FocusScope {
        key: SpatialKey::from_string("k-leaf"),
        moniker: Moniker::from_string("ui:leaf"),
        rect: Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        },
        layer_key: LayerKey::from_string("layer-1"),
        parent_zone: None,
        overrides: HashMap::new(),
    };

    let zone = FocusZone {
        key: SpatialKey::from_string("k-zone"),
        moniker: Moniker::from_string("ui:zone"),
        rect: Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        },
        layer_key: LayerKey::from_string("layer-1"),
        parent_zone: None,
        last_focused: None,
        overrides: HashMap::new(),
    };

    let leaf_json = serde_json::to_value(&leaf).unwrap();
    let zone_json = serde_json::to_value(&zone).unwrap();

    let leaf_back: FocusScope = serde_json::from_value(leaf_json).unwrap();
    let zone_back: FocusZone = serde_json::from_value(zone_json).unwrap();

    assert_eq!(leaf_back.moniker, leaf.moniker);
    assert_eq!(zone_back.moniker, zone.moniker);
    assert_eq!(zone_back.last_focused, None);
}

// ---------------------------------------------------------------------------
// Registry — round-trip register/lookup
// ---------------------------------------------------------------------------

fn make_focus_scope(
    key: &str,
    moniker: &str,
    layer: &str,
    parent_zone: Option<&str>,
) -> FocusScope {
    FocusScope {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect: Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        },
        layer_key: LayerKey::from_string(layer),
        parent_zone: parent_zone.map(SpatialKey::from_string),
        overrides: HashMap::new(),
    }
}

fn make_zone(key: &str, moniker: &str, layer: &str, parent_zone: Option<&str>) -> FocusZone {
    FocusZone {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect: Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(100.0),
            height: Pixels::new(100.0),
        },
        layer_key: LayerKey::from_string(layer),
        parent_zone: parent_zone.map(SpatialKey::from_string),
        last_focused: None,
        overrides: HashMap::new(),
    }
}

fn make_layer(key: &str, name: &str, window: &str, parent: Option<&str>) -> FocusLayer {
    FocusLayer {
        key: LayerKey::from_string(key),
        name: LayerName::from_string(name),
        parent: parent.map(LayerKey::from_string),
        window_label: WindowLabel::from_string(window),
        last_focused: None,
    }
}

/// Registering a [`FocusScope`] and a [`FocusZone`] under different keys
/// stores them in the same `SpatialKey`-indexed map but exposes them
/// through the variant-typed accessors. The leaf accessor (`scope`)
/// returns `Some` only for leaves; the zone accessor (`zone`) returns
/// `Some` only for zones.
#[test]
fn registry_returns_typed_accessor_for_each_variant() {
    let mut reg = SpatialRegistry::new();
    let leaf = make_focus_scope("k-leaf", "ui:leaf", "L1", None);
    let zone = make_zone("k-zone", "ui:zone", "L1", None);
    reg.register_scope(leaf);
    reg.register_zone(zone);

    assert!(reg.scope(&SpatialKey::from_string("k-leaf")).is_some());
    assert!(reg.zone(&SpatialKey::from_string("k-leaf")).is_none());

    assert!(reg.zone(&SpatialKey::from_string("k-zone")).is_some());
    assert!(reg.scope(&SpatialKey::from_string("k-zone")).is_none());

    // Both keys are registered (variant-blind check).
    assert!(reg.is_registered(&SpatialKey::from_string("k-leaf")));
    assert!(reg.is_registered(&SpatialKey::from_string("k-zone")));
    assert!(!reg.is_registered(&SpatialKey::from_string("ghost")));
}

/// `update_rect` mutates the stored rect of a registered scope without
/// changing its variant or other fields.
#[test]
fn update_rect_preserves_variant_and_other_fields() {
    let mut reg = SpatialRegistry::new();
    let leaf = make_focus_scope("k", "ui:leaf", "L1", Some("parent"));
    reg.register_scope(leaf);

    let new_rect = Rect {
        x: Pixels::new(5.0),
        y: Pixels::new(6.0),
        width: Pixels::new(7.0),
        height: Pixels::new(8.0),
    };
    reg.update_rect(&SpatialKey::from_string("k"), new_rect);

    let scope = reg.scope(&SpatialKey::from_string("k")).unwrap();
    assert_eq!(scope.rect, new_rect);
    assert_eq!(scope.parent_zone, Some(SpatialKey::from_string("parent")));
}

/// `unregister_scope` removes the scope from the map.
#[test]
fn unregister_removes_scope() {
    let mut reg = SpatialRegistry::new();
    let leaf = make_focus_scope("k", "ui:leaf", "L1", None);
    reg.register_scope(leaf);
    assert!(reg.is_registered(&SpatialKey::from_string("k")));

    reg.unregister_scope(&SpatialKey::from_string("k"));
    assert!(!reg.is_registered(&SpatialKey::from_string("k")));
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
    reg.register_zone(make_zone("outer", "ui:outer", "L1", None));
    reg.register_zone(make_zone("inner", "ui:inner", "L1", Some("outer")));
    reg.register_scope(make_focus_scope(
        "leaf-outer",
        "ui:leaf-outer",
        "L1",
        Some("outer"),
    ));
    reg.register_scope(make_focus_scope(
        "leaf-inner",
        "ui:leaf-inner",
        "L1",
        Some("inner"),
    ));

    let outer_kids: Vec<&SpatialKey> = reg
        .children_of_zone(&SpatialKey::from_string("outer"))
        .map(|c| match c {
            ChildScope::Leaf(f) => &f.key,
            ChildScope::Zone(z) => &z.key,
        })
        .collect();
    assert_eq!(
        outer_kids.len(),
        2,
        "outer has 2 direct children: inner zone + leaf-outer"
    );
    assert!(outer_kids.contains(&&SpatialKey::from_string("inner")));
    assert!(outer_kids.contains(&&SpatialKey::from_string("leaf-outer")));
    assert!(
        !outer_kids.contains(&&SpatialKey::from_string("leaf-inner")),
        "leaf-inner is a grandchild — must not show up"
    );
}

/// `ancestor_zones` walks `parent_zone` from the focused leaf up through
/// each ancestor zone, in order from nearest to farthest.
#[test]
fn ancestor_zones_walks_parent_chain_innermost_first() {
    let mut reg = SpatialRegistry::new();
    reg.register_zone(make_zone("outer", "ui:outer", "L1", None));
    reg.register_zone(make_zone("middle", "ui:middle", "L1", Some("outer")));
    reg.register_zone(make_zone("inner", "ui:inner", "L1", Some("middle")));
    reg.register_scope(make_focus_scope("leaf", "ui:leaf", "L1", Some("inner")));

    let chain: Vec<&SpatialKey> = reg
        .ancestor_zones(&SpatialKey::from_string("leaf"))
        .into_iter()
        .map(|z| &z.key)
        .collect();

    assert_eq!(
        chain,
        vec![
            &SpatialKey::from_string("inner"),
            &SpatialKey::from_string("middle"),
            &SpatialKey::from_string("outer"),
        ]
    );
}

/// `ancestor_zones` for a leaf with no `parent_zone` returns an empty
/// vector — the leaf itself is at the layer root.
#[test]
fn ancestor_zones_at_layer_root_is_empty() {
    let mut reg = SpatialRegistry::new();
    reg.register_scope(make_focus_scope("leaf", "ui:leaf", "L1", None));

    assert!(reg
        .ancestor_zones(&SpatialKey::from_string("leaf"))
        .is_empty());
}

// ---------------------------------------------------------------------------
// Layer forest ops
// ---------------------------------------------------------------------------

/// `children_of_layer` returns layers whose `parent` matches the queried
/// key. Layers belonging to other windows or other parents are filtered
/// out.
#[test]
fn children_of_layer_filters_by_parent_not_cross_window() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("root-a", "window", "win-a", None));
    reg.push_layer(make_layer("root-b", "window", "win-b", None));
    reg.push_layer(make_layer("ins-a", "inspector", "win-a", Some("root-a")));
    reg.push_layer(make_layer("ins-b", "inspector", "win-b", Some("root-b")));
    reg.push_layer(make_layer("dlg-a", "dialog", "win-a", Some("ins-a")));

    let kids_a: Vec<&LayerKey> = reg
        .children_of_layer(&LayerKey::from_string("root-a"))
        .into_iter()
        .map(|l| &l.key)
        .collect();

    assert_eq!(
        kids_a.len(),
        1,
        "root-a has exactly one child layer (ins-a)"
    );
    assert_eq!(kids_a[0], &LayerKey::from_string("ins-a"));
}

/// `root_for_window` returns the layer with the matching `window_label`
/// and `parent == None`.
#[test]
fn root_for_window_returns_window_root_only() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("root-a", "window", "win-a", None));
    reg.push_layer(make_layer("ins-a", "inspector", "win-a", Some("root-a")));
    reg.push_layer(make_layer("root-b", "window", "win-b", None));

    let root_a = reg
        .root_for_window(&WindowLabel::from_string("win-a"))
        .unwrap();
    assert_eq!(root_a.key, LayerKey::from_string("root-a"));

    let root_b = reg
        .root_for_window(&WindowLabel::from_string("win-b"))
        .unwrap();
    assert_eq!(root_b.key, LayerKey::from_string("root-b"));

    assert!(reg
        .root_for_window(&WindowLabel::from_string("ghost"))
        .is_none());
}

/// `ancestors_of_layer` walks `layer.parent` from the queried layer up to
/// the window root, innermost first.
#[test]
fn ancestors_of_layer_walks_parent_chain() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("root", "window", "win-a", None));
    reg.push_layer(make_layer("ins", "inspector", "win-a", Some("root")));
    reg.push_layer(make_layer("dlg", "dialog", "win-a", Some("ins")));

    let chain: Vec<&LayerKey> = reg
        .ancestors_of_layer(&LayerKey::from_string("dlg"))
        .into_iter()
        .map(|l| &l.key)
        .collect();

    assert_eq!(
        chain,
        vec![
            &LayerKey::from_string("ins"),
            &LayerKey::from_string("root"),
        ]
    );
}

/// `remove_layer` deletes the layer; subsequent `layer(key)` returns
/// `None`.
#[test]
fn remove_layer_drops_the_entry() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("L", "window", "win-a", None));
    assert!(reg.layer(&LayerKey::from_string("L")).is_some());

    reg.remove_layer(&LayerKey::from_string("L"));
    assert!(reg.layer(&LayerKey::from_string("L")).is_none());
}

/// `leaves_in_layer` and `zones_in_layer` return typed structs filtered
/// by `layer_key`. Scopes in other layers are excluded.
#[test]
fn leaves_and_zones_in_layer_filter_by_layer_key() {
    let mut reg = SpatialRegistry::new();
    reg.register_scope(make_focus_scope("leaf-1", "ui:leaf-1", "L1", None));
    reg.register_zone(make_zone("zone-1", "ui:zone-1", "L1", None));
    reg.register_scope(make_focus_scope("leaf-2", "ui:leaf-2", "L2", None));

    let leaf_keys: Vec<&SpatialKey> = reg
        .leaves_in_layer(&LayerKey::from_string("L1"))
        .map(|f| &f.key)
        .collect();
    let zone_keys: Vec<&SpatialKey> = reg
        .zones_in_layer(&LayerKey::from_string("L1"))
        .map(|z| &z.key)
        .collect();

    assert_eq!(leaf_keys, vec![&SpatialKey::from_string("leaf-1")]);
    assert_eq!(zone_keys, vec![&SpatialKey::from_string("zone-1")]);

    // L2 has only the second leaf.
    let l2_leaves: Vec<&SpatialKey> = reg
        .leaves_in_layer(&LayerKey::from_string("L2"))
        .map(|f| &f.key)
        .collect();
    assert_eq!(l2_leaves, vec![&SpatialKey::from_string("leaf-2")]);
}

// ---------------------------------------------------------------------------
// Forest scenario — 2 windows + 2 inspectors + 1 dialog = 5 layers, 2 roots
// ---------------------------------------------------------------------------

/// Build a realistic forest: two windows (each with a root layer and an
/// inspector layer), plus a dialog layer stacked on one of the inspectors.
/// Verify that:
/// - `root_for_window` finds each window's root.
/// - `ancestors_of_layer` for the dialog walks `dialog → inspector → root`.
/// - `children_of_layer(window-A)` returns only the window-A inspector.
#[test]
fn forest_with_two_windows_and_stacked_overlays() {
    let mut reg = SpatialRegistry::new();

    // Two windows, each with a root layer.
    reg.push_layer(make_layer("root-a", "window", "win-a", None));
    reg.push_layer(make_layer("root-b", "window", "win-b", None));

    // Each window has an inspector overlay.
    reg.push_layer(make_layer("ins-a", "inspector", "win-a", Some("root-a")));
    reg.push_layer(make_layer("ins-b", "inspector", "win-b", Some("root-b")));

    // Window A has a dialog stacked on top of its inspector.
    reg.push_layer(make_layer("dlg-a", "dialog", "win-a", Some("ins-a")));

    // 5 layers, 2 roots.
    let root_a = reg
        .root_for_window(&WindowLabel::from_string("win-a"))
        .unwrap();
    let root_b = reg
        .root_for_window(&WindowLabel::from_string("win-b"))
        .unwrap();
    assert_eq!(root_a.key, LayerKey::from_string("root-a"));
    assert_eq!(root_b.key, LayerKey::from_string("root-b"));

    // Dialog ancestors: ins-a, root-a (innermost first, no cross to win-b).
    let dlg_ancestors: Vec<&LayerKey> = reg
        .ancestors_of_layer(&LayerKey::from_string("dlg-a"))
        .into_iter()
        .map(|l| &l.key)
        .collect();
    assert_eq!(
        dlg_ancestors,
        vec![
            &LayerKey::from_string("ins-a"),
            &LayerKey::from_string("root-a"),
        ]
    );

    // Children of root-a: ins-a only (ins-b belongs to root-b).
    let root_a_kids: Vec<&LayerKey> = reg
        .children_of_layer(&LayerKey::from_string("root-a"))
        .into_iter()
        .map(|l| &l.key)
        .collect();
    assert_eq!(root_a_kids, vec![&LayerKey::from_string("ins-a")]);
}

// ---------------------------------------------------------------------------
// Overrides round-trip on FocusScope
// ---------------------------------------------------------------------------

/// `FocusScope.overrides` map round-trips through serde with `Direction` as
/// the JSON key. `None` value preserves an explicit "wall" override (block
/// nav in this direction); `Some(Moniker)` is an explicit redirect.
#[test]
fn focus_scope_overrides_round_trip_with_direction_keys() {
    let mut overrides: HashMap<Direction, Option<Moniker>> = HashMap::new();
    overrides.insert(Direction::Up, Some(Moniker::from_string("ui:above")));
    overrides.insert(Direction::Left, None);

    let f = FocusScope {
        key: SpatialKey::from_string("k"),
        moniker: Moniker::from_string("ui:k"),
        rect: Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(1.0),
            height: Pixels::new(1.0),
        },
        layer_key: LayerKey::from_string("L"),
        parent_zone: None,
        overrides,
    };

    let json = serde_json::to_value(&f).unwrap();
    let back: FocusScope = serde_json::from_value(json).unwrap();

    assert_eq!(back.overrides.len(), 2);
    assert_eq!(
        back.overrides.get(&Direction::Up),
        Some(&Some(Moniker::from_string("ui:above")))
    );
    assert_eq!(back.overrides.get(&Direction::Left), Some(&None));
}
