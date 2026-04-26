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
    FallbackResolution, FocusLayer, FocusZone, Focusable, LayerKey, LayerName, Moniker, Pixels,
    Rect, SpatialKey, SpatialRegistry, SpatialState, WindowLabel,
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

/// Build a [`Focusable`] with the given identity, rect, layer, and
/// optional parent zone.
fn focusable(
    key: &str,
    moniker: &str,
    layer: &str,
    parent_zone: Option<&str>,
    r: Rect,
) -> Focusable {
    Focusable {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect: r,
        layer_key: LayerKey::from_string(layer),
        parent_zone: parent_zone.map(SpatialKey::from_string),
        overrides: HashMap::new(),
    }
}

/// Build a [`FocusZone`] with optional `last_focused` and parent zone.
fn zone(
    key: &str,
    moniker: &str,
    layer: &str,
    parent_zone: Option<&str>,
    last_focused: Option<&str>,
    r: Rect,
) -> FocusZone {
    FocusZone {
        key: SpatialKey::from_string(key),
        moniker: Moniker::from_string(moniker),
        rect: r,
        layer_key: LayerKey::from_string(layer),
        parent_zone: parent_zone.map(SpatialKey::from_string),
        last_focused: last_focused.map(SpatialKey::from_string),
        overrides: HashMap::new(),
    }
}

/// Build a [`FocusLayer`] tied to a window.
fn layer(key: &str, window: &str, parent: Option<&str>, last_focused: Option<&str>) -> FocusLayer {
    FocusLayer {
        key: LayerKey::from_string(key),
        name: LayerName::from_string("window"),
        parent: parent.map(LayerKey::from_string),
        window_label: WindowLabel::from_string(window),
        last_focused: last_focused.map(SpatialKey::from_string),
    }
}

// ---------------------------------------------------------------------------
// Sibling in same zone (rule 1)
// ---------------------------------------------------------------------------

/// When the focused entry's parent_zone still has live siblings,
/// fallback picks the nearest sibling in the same zone. The sibling's
/// [`SpatialKey`] and [`Moniker`] are returned in the
/// [`FallbackResolution::FallbackSiblingInZone`] variant.
#[test]
fn fallback_returns_sibling_in_zone() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None, None));
    reg.register_zone(zone(
        "z",
        "ui:zone",
        "L",
        None,
        None,
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    // Two sibling leaves in the same zone, plus the lost focused leaf.
    reg.register_focusable(focusable(
        "lost",
        "ui:lost",
        "L",
        Some("z"),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    reg.register_focusable(focusable(
        "sib-near",
        "ui:sib-near",
        "L",
        Some("z"),
        rect(20.0, 0.0, 10.0, 10.0),
    ));
    reg.register_focusable(focusable(
        "sib-far",
        "ui:sib-far",
        "L",
        Some("z"),
        rect(100.0, 100.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state
        .focus(&reg, SpatialKey::from_string("lost"))
        .expect("focus lost");

    let resolution = state.resolve_fallback(&reg, &SpatialKey::from_string("lost"));
    match resolution {
        FallbackResolution::FallbackSiblingInZone(key, moniker) => {
            assert_eq!(key, SpatialKey::from_string("sib-near"));
            assert_eq!(moniker, Moniker::from_string("ui:sib-near"));
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
    reg.push_layer(layer("L", "main", None, None));
    // The lost entry's zone has a `last_focused` slot pointing at
    // "remembered" — but rule 1 should ignore it and pick the nearest
    // sibling instead.
    reg.register_zone(zone(
        "z",
        "ui:zone",
        "L",
        None,
        Some("remembered"),
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    reg.register_focusable(focusable(
        "lost",
        "ui:lost",
        "L",
        Some("z"),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    // Closest sibling — should win even though "remembered" is the
    // zone's recorded last-focused slot.
    reg.register_focusable(focusable(
        "sib-near",
        "ui:sib-near",
        "L",
        Some("z"),
        rect(20.0, 0.0, 10.0, 10.0),
    ));
    // The "remembered" leaf is registered but is geometrically farther
    // than `sib-near`. If rule 1 incorrectly consulted the zone's
    // `last_focused`, this test would surface
    // `FallbackParentZoneLastFocused` instead of the expected
    // `FallbackSiblingInZone`.
    reg.register_focusable(focusable(
        "remembered",
        "ui:remembered",
        "L",
        Some("z"),
        rect(150.0, 150.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state
        .focus(&reg, SpatialKey::from_string("lost"))
        .expect("focus lost");

    let resolution = state.resolve_fallback(&reg, &SpatialKey::from_string("lost"));
    match resolution {
        FallbackResolution::FallbackSiblingInZone(key, moniker) => {
            assert_eq!(key, SpatialKey::from_string("sib-near"));
            assert_eq!(moniker, Moniker::from_string("ui:sib-near"));
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
#[test]
fn fallback_returns_parent_zone_last_focused() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None, None));
    // Outer zone with last_focused pointing at "remembered".
    reg.register_zone(zone(
        "outer",
        "ui:outer",
        "L",
        None,
        Some("remembered"),
        rect(0.0, 0.0, 500.0, 500.0),
    ));
    // Inner zone (about to empty when "lost" is unregistered).
    reg.register_zone(zone(
        "inner",
        "ui:inner",
        "L",
        Some("outer"),
        None,
        rect(0.0, 0.0, 100.0, 100.0),
    ));
    // Sole leaf in the inner zone.
    reg.register_focusable(focusable(
        "lost",
        "ui:lost",
        "L",
        Some("inner"),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    // The remembered scope sits in the outer zone (parent of inner).
    reg.register_focusable(focusable(
        "remembered",
        "ui:remembered",
        "L",
        Some("outer"),
        rect(200.0, 200.0, 10.0, 10.0),
    ));
    // Another sibling in outer for variety, but rule 2 should still pick
    // the remembered slot.
    reg.register_focusable(focusable(
        "other-in-outer",
        "ui:other",
        "L",
        Some("outer"),
        rect(300.0, 300.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state
        .focus(&reg, SpatialKey::from_string("lost"))
        .expect("focus lost");

    // Simulate: the lost entry has been unregistered, so the inner zone
    // is empty. Resolve fallback walking outward.
    reg.unregister_scope(&SpatialKey::from_string("lost"));

    let resolution = state.resolve_fallback(&reg, &SpatialKey::from_string("lost"));
    // The metadata for "lost" is gone — the resolver must have remembered
    // its parent_zone before we unregistered. The cleanest contract is to
    // resolve BEFORE the registry mutation, so the lost entry's metadata
    // is still readable. This test asserts the pre-unregister flow.
    // Re-register and try again below.
    let _ = resolution;
    // Re-register so we can run resolution before unregister:
    reg.register_focusable(focusable(
        "lost",
        "ui:lost",
        "L",
        Some("inner"),
        rect(0.0, 0.0, 10.0, 10.0),
    ));

    let resolution = state.resolve_fallback(&reg, &SpatialKey::from_string("lost"));
    match resolution {
        FallbackResolution::FallbackParentZoneLastFocused(key, moniker) => {
            assert_eq!(key, SpatialKey::from_string("remembered"));
            assert_eq!(moniker, Moniker::from_string("ui:remembered"));
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
    reg.push_layer(layer("L", "main", None, None));
    reg.register_zone(zone(
        "outer",
        "ui:outer",
        "L",
        None,
        Some("ghost"), // points at unregistered key
        rect(0.0, 0.0, 500.0, 500.0),
    ));
    // Inner zone positioned far from the lost rect so it does not
    // beat `near` on distance once rule 2's nearest scan runs.
    reg.register_zone(zone(
        "inner",
        "ui:inner",
        "L",
        Some("outer"),
        None,
        rect(400.0, 400.0, 100.0, 100.0),
    ));
    reg.register_focusable(focusable(
        "lost",
        "ui:lost",
        "L",
        Some("inner"),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    // Two leaves in the outer zone — the nearest by top-left wins.
    reg.register_focusable(focusable(
        "near",
        "ui:near",
        "L",
        Some("outer"),
        rect(20.0, 0.0, 10.0, 10.0),
    ));
    reg.register_focusable(focusable(
        "far",
        "ui:far",
        "L",
        Some("outer"),
        rect(300.0, 300.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state
        .focus(&reg, SpatialKey::from_string("lost"))
        .expect("focus lost");

    let resolution = state.resolve_fallback(&reg, &SpatialKey::from_string("lost"));
    match resolution {
        FallbackResolution::FallbackParentZoneNearest(key, moniker) => {
            assert_eq!(key, SpatialKey::from_string("near"));
            assert_eq!(moniker, Moniker::from_string("ui:near"));
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
#[test]
fn fallback_returns_parent_layer_last_focused() {
    let mut reg = SpatialRegistry::new();
    // Root layer of the window with a remembered slot inside.
    reg.push_layer(layer("root", "main", None, Some("root-leaf")));
    // Child layer (e.g. inspector overlay). The lost focused entry
    // lives here; when it goes the layer has no entries left.
    reg.push_layer(layer("child", "main", Some("root"), None));
    reg.register_focusable(focusable(
        "lost",
        "ui:lost",
        "child",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    // Live entry in the parent layer.
    reg.register_focusable(focusable(
        "root-leaf",
        "ui:root-leaf",
        "root",
        None,
        rect(20.0, 20.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state
        .focus(&reg, SpatialKey::from_string("lost"))
        .expect("focus lost");

    let resolution = state.resolve_fallback(&reg, &SpatialKey::from_string("lost"));
    match resolution {
        FallbackResolution::FallbackParentLayerLastFocused(key, moniker) => {
            assert_eq!(key, SpatialKey::from_string("root-leaf"));
            assert_eq!(moniker, Moniker::from_string("ui:root-leaf"));
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
    reg.push_layer(layer("root", "main", None, None));
    // Child layer (e.g. inspector overlay) holds the lost entry alone.
    reg.push_layer(layer("child", "main", Some("root"), None));
    reg.register_focusable(focusable(
        "lost",
        "ui:lost",
        "child",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    // A nested leaf is the only candidate in the parent layer. Its
    // enclosing zone is positioned far away so the leaf wins on raw
    // distance regardless of variant — the assertion is that rule 4's
    // nearest-fallback can reach a zone-nested leaf at all (the bug
    // this test guards against was the candidate set being limited to
    // `parent_zone is None`).
    reg.register_zone(zone(
        "root-zone",
        "ui:root-zone",
        "root",
        None,
        None,
        rect(500.0, 500.0, 100.0, 100.0),
    ));
    reg.register_focusable(focusable(
        "nested",
        "ui:nested",
        "root",
        Some("root-zone"),
        rect(20.0, 20.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state
        .focus(&reg, SpatialKey::from_string("lost"))
        .expect("focus lost");

    let resolution = state.resolve_fallback(&reg, &SpatialKey::from_string("lost"));
    match resolution {
        FallbackResolution::FallbackParentLayerNearest(key, moniker) => {
            assert_eq!(key, SpatialKey::from_string("nested"));
            assert_eq!(moniker, Moniker::from_string("ui:nested"));
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
    reg.push_layer(layer("root", "main", None, None));
    reg.register_focusable(focusable(
        "lost",
        "ui:lost",
        "root",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state
        .focus(&reg, SpatialKey::from_string("lost"))
        .expect("focus lost");

    let resolution = state.resolve_fallback(&reg, &SpatialKey::from_string("lost"));
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
    reg.push_layer(layer("La", "win-a", Some("Lb"), None));
    reg.register_focusable(focusable(
        "lost",
        "ui:lost",
        "La",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    // Window B: fully populated. If the barrier were not enforced, the
    // resolver would land on `b-leaf` (via `Lb`'s `last_focused`).
    reg.push_layer(layer("Lb", "win-b", None, Some("b-leaf")));
    reg.register_focusable(focusable(
        "b-leaf",
        "ui:b-leaf",
        "Lb",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state
        .focus(&reg, SpatialKey::from_string("lost"))
        .expect("focus lost");

    let resolution = state.resolve_fallback(&reg, &SpatialKey::from_string("lost"));
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
/// fallback and emits a [`FocusChangedEvent`] whose `next_key` /
/// `next_moniker` reflect the fallback target.
#[test]
fn handle_unregister_emits_event_with_fallback_target() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None, None));
    reg.register_zone(zone(
        "z",
        "ui:zone",
        "L",
        None,
        None,
        rect(0.0, 0.0, 200.0, 200.0),
    ));
    reg.register_focusable(focusable(
        "lost",
        "ui:lost",
        "L",
        Some("z"),
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    reg.register_focusable(focusable(
        "sibling",
        "ui:sibling",
        "L",
        Some("z"),
        rect(20.0, 0.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state
        .focus(&reg, SpatialKey::from_string("lost"))
        .expect("focus lost");

    let event = state
        .handle_unregister(&reg, &SpatialKey::from_string("lost"))
        .expect("handle_unregister emits an event when the focused key is unregistered");

    assert_eq!(event.window_label, WindowLabel::from_string("main"));
    assert_eq!(event.prev_key, Some(SpatialKey::from_string("lost")));
    assert_eq!(event.next_key, Some(SpatialKey::from_string("sibling")));
    assert_eq!(event.next_moniker, Some(Moniker::from_string("ui:sibling")),);

    // The window's slot must now point at the fallback target.
    assert_eq!(
        state.focused_in(&WindowLabel::from_string("main")),
        Some(&SpatialKey::from_string("sibling")),
    );
}

/// At a lone window root, `handle_unregister` clears the focus slot and
/// emits a `Some → None` event so the React claim registry can release
/// the focus visual.
#[test]
fn handle_unregister_clears_focus_when_no_fallback() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None, None));
    reg.register_focusable(focusable(
        "lost",
        "ui:lost",
        "L",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state
        .focus(&reg, SpatialKey::from_string("lost"))
        .expect("focus lost");

    let event = state
        .handle_unregister(&reg, &SpatialKey::from_string("lost"))
        .expect("handle_unregister emits a clear event when there is no fallback");

    assert_eq!(event.window_label, WindowLabel::from_string("main"));
    assert_eq!(event.prev_key, Some(SpatialKey::from_string("lost")));
    assert_eq!(event.next_key, None);
    assert_eq!(event.next_moniker, None);
    assert_eq!(state.focused_in(&WindowLabel::from_string("main")), None);
}

/// `handle_unregister` for an unfocused key is a no-op — no fallback
/// resolution runs and no event is emitted.
#[test]
fn handle_unregister_unfocused_key_is_noop() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("L", "main", None, None));
    reg.register_focusable(focusable(
        "focused",
        "ui:focused",
        "L",
        None,
        rect(0.0, 0.0, 10.0, 10.0),
    ));
    reg.register_focusable(focusable(
        "other",
        "ui:other",
        "L",
        None,
        rect(20.0, 20.0, 10.0, 10.0),
    ));

    let mut state = SpatialState::new();
    state
        .focus(&reg, SpatialKey::from_string("focused"))
        .expect("focus focused");

    assert!(
        state
            .handle_unregister(&reg, &SpatialKey::from_string("other"))
            .is_none(),
        "unregistering an unfocused key emits nothing",
    );
    // Focus slot still points at "focused".
    assert_eq!(
        state.focused_in(&WindowLabel::from_string("main")),
        Some(&SpatialKey::from_string("focused")),
    );
}
