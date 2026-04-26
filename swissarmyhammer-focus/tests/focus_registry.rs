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
//! - `FocusScope` is a sum type that JSON-round-trips with a `"kind"` tag
//!   discriminator.
//! - `SpatialRegistry` stores both `Focusable` leaves and `FocusZone`
//!   containers behind a single `SpatialKey` map.
//! - Zone tree ops (`children_of_zone`, `ancestor_zones`) walk the
//!   `parent_zone` chain inside a layer.
//! - Layer forest ops (`children_of_layer`, `root_for_window`,
//!   `ancestors_of_layer`) walk the `layer.parent` chain across windows.
//! - `scopes_in_layer` returns both `Focusable` and `Zone` variants
//!   filtered by `layer_key`.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    Direction, FocusLayer, FocusScope, FocusZone, Focusable, LayerKey, LayerName, Moniker, Pixels,
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
// FocusScope round-trip
// ---------------------------------------------------------------------------

/// `FocusScope::Focusable(_)` and `FocusScope::Zone(_)` round-trip with a
/// `"kind"` discriminator and a `snake_case` rename. The frontend reads the
/// `kind` field to pick the right component shape.
#[test]
fn focus_scope_round_trips_with_kind_tag() {
    let leaf = FocusScope::Focusable(Focusable {
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
    });

    let zone = FocusScope::Zone(FocusZone {
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
    });

    let leaf_json = serde_json::to_value(&leaf).unwrap();
    assert_eq!(leaf_json["kind"], "focusable");

    let zone_json = serde_json::to_value(&zone).unwrap();
    assert_eq!(zone_json["kind"], "zone");

    let leaf_back: FocusScope = serde_json::from_value(leaf_json).unwrap();
    let zone_back: FocusScope = serde_json::from_value(zone_json).unwrap();

    assert!(matches!(leaf_back, FocusScope::Focusable(_)));
    assert!(matches!(zone_back, FocusScope::Zone(_)));
}

/// `FocusScope` accessors return the right field across both variants —
/// pattern matching belongs to the registry; consumers that just need the
/// shared fields (`key`, `moniker`, `rect`, `layer_key`) can use the
/// helpers without unwrapping.
#[test]
fn focus_scope_accessors_work_across_variants() {
    let leaf = FocusScope::Focusable(Focusable {
        key: SpatialKey::from_string("k-leaf"),
        moniker: Moniker::from_string("ui:leaf"),
        rect: Rect {
            x: Pixels::new(1.0),
            y: Pixels::new(2.0),
            width: Pixels::new(3.0),
            height: Pixels::new(4.0),
        },
        layer_key: LayerKey::from_string("layer-1"),
        parent_zone: Some(SpatialKey::from_string("parent")),
        overrides: HashMap::new(),
    });

    assert!(leaf.is_focusable());
    assert!(!leaf.is_zone());
    assert_eq!(leaf.as_zone(), None);
    assert_eq!(leaf.key(), &SpatialKey::from_string("k-leaf"));
    assert_eq!(leaf.moniker(), &Moniker::from_string("ui:leaf"));
    assert_eq!(leaf.layer_key(), &LayerKey::from_string("layer-1"));
    assert_eq!(leaf.parent_zone(), Some(&SpatialKey::from_string("parent")));
}

// ---------------------------------------------------------------------------
// Registry — round-trip register/lookup
// ---------------------------------------------------------------------------

fn make_focusable(key: &str, moniker: &str, layer: &str, parent_zone: Option<&str>) -> Focusable {
    Focusable {
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

/// Registering a `Focusable` and a `FocusZone` puts them in the same
/// `SpatialKey`-indexed map. `scope(key)` returns the right variant for
/// each key.
#[test]
fn registry_returns_correct_variant_for_each_key() {
    let mut reg = SpatialRegistry::new();
    let leaf = make_focusable("k-leaf", "ui:leaf", "L1", None);
    let zone = make_zone("k-zone", "ui:zone", "L1", None);
    reg.register_focusable(leaf);
    reg.register_zone(zone);

    let leaf_back = reg.scope(&SpatialKey::from_string("k-leaf"));
    let zone_back = reg.scope(&SpatialKey::from_string("k-zone"));

    assert!(matches!(leaf_back, Some(FocusScope::Focusable(_))));
    assert!(matches!(zone_back, Some(FocusScope::Zone(_))));
}

/// `update_rect` mutates the stored rect of a registered scope without
/// changing its variant or other fields.
#[test]
fn update_rect_preserves_variant_and_other_fields() {
    let mut reg = SpatialRegistry::new();
    let leaf = make_focusable("k", "ui:leaf", "L1", Some("parent"));
    reg.register_focusable(leaf);

    let new_rect = Rect {
        x: Pixels::new(5.0),
        y: Pixels::new(6.0),
        width: Pixels::new(7.0),
        height: Pixels::new(8.0),
    };
    reg.update_rect(&SpatialKey::from_string("k"), new_rect);

    let scope = reg.scope(&SpatialKey::from_string("k")).unwrap();
    assert_eq!(scope.rect(), &new_rect);
    assert_eq!(
        scope.parent_zone(),
        Some(&SpatialKey::from_string("parent"))
    );
    assert!(scope.is_focusable());
}

/// `unregister_scope` removes the scope from the map.
#[test]
fn unregister_removes_scope() {
    let mut reg = SpatialRegistry::new();
    let leaf = make_focusable("k", "ui:leaf", "L1", None);
    reg.register_focusable(leaf);
    assert!(reg.scope(&SpatialKey::from_string("k")).is_some());

    reg.unregister_scope(&SpatialKey::from_string("k"));
    assert!(reg.scope(&SpatialKey::from_string("k")).is_none());
}

// ---------------------------------------------------------------------------
// Zone tree ops
// ---------------------------------------------------------------------------

/// `children_of_zone` returns direct children only — a grandchild whose
/// `parent_zone` points at a different zone must NOT show up.
#[test]
fn children_of_zone_returns_direct_children_only() {
    let mut reg = SpatialRegistry::new();
    // outer zone → inner zone → leaf
    reg.register_zone(make_zone("outer", "ui:outer", "L1", None));
    reg.register_zone(make_zone("inner", "ui:inner", "L1", Some("outer")));
    reg.register_focusable(make_focusable(
        "leaf-outer",
        "ui:leaf-outer",
        "L1",
        Some("outer"),
    ));
    reg.register_focusable(make_focusable(
        "leaf-inner",
        "ui:leaf-inner",
        "L1",
        Some("inner"),
    ));

    let outer_kids: Vec<&SpatialKey> = reg
        .children_of_zone(&SpatialKey::from_string("outer"))
        .map(|s| s.key())
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
    reg.register_focusable(make_focusable("leaf", "ui:leaf", "L1", Some("inner")));

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
    reg.register_focusable(make_focusable("leaf", "ui:leaf", "L1", None));

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

/// `scopes_in_layer` returns both `Focusable` and `Zone` variants whose
/// `layer_key` matches the queried layer. Scopes in other layers are
/// excluded.
#[test]
fn scopes_in_layer_returns_both_variants_filtered_by_layer() {
    let mut reg = SpatialRegistry::new();
    reg.register_focusable(make_focusable("leaf-1", "ui:leaf-1", "L1", None));
    reg.register_zone(make_zone("zone-1", "ui:zone-1", "L1", None));
    reg.register_focusable(make_focusable("leaf-2", "ui:leaf-2", "L2", None));

    let in_l1: Vec<&SpatialKey> = reg
        .scopes_in_layer(&LayerKey::from_string("L1"))
        .map(|s| s.key())
        .collect();

    assert_eq!(in_l1.len(), 2);
    assert!(in_l1.contains(&&SpatialKey::from_string("leaf-1")));
    assert!(in_l1.contains(&&SpatialKey::from_string("zone-1")));
    assert!(!in_l1.contains(&&SpatialKey::from_string("leaf-2")));
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
// Overrides round-trip on Focusable
// ---------------------------------------------------------------------------

/// `Focusable.overrides` map round-trips through serde with `Direction` as
/// the JSON key. `None` value preserves an explicit "wall" override (block
/// nav in this direction); `Some(Moniker)` is an explicit redirect.
#[test]
fn focusable_overrides_round_trip_with_direction_keys() {
    let mut overrides: HashMap<Direction, Option<Moniker>> = HashMap::new();
    overrides.insert(Direction::Up, Some(Moniker::from_string("ui:above")));
    overrides.insert(Direction::Left, None);

    let f = Focusable {
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
    let back: Focusable = serde_json::from_value(json).unwrap();

    assert_eq!(back.overrides.len(), 2);
    assert_eq!(
        back.overrides.get(&Direction::Up),
        Some(&Some(Moniker::from_string("ui:above")))
    );
    assert_eq!(back.overrides.get(&Direction::Left), Some(&None));
}
