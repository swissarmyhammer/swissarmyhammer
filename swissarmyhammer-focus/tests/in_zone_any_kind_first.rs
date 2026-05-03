//! Source-of-truth integration tests for the **any-kind iter-0
//! sibling rule** under the unified cascade.
//!
//! # Contract
//!
//! Within a parent `<FocusZone>`, child `<FocusScope>` leaves and child
//! `<FocusZone>` containers are **siblings**. Cardinal navigation must
//! treat them as peers — never filter by kind at the in-zone (iter 0)
//! level. The Android beam score picks the geometrically best candidate
//! of any kind. See `swissarmyhammer-focus/README.md` for the full
//! contract and `src/navigate.rs` for the algorithm.
//!
//! Iter 1 (escalation to peer-zone level) keeps same-kind because the
//! parent IS a zone, so its peers are zones by construction. That is
//! structural, not a kind policy.
//!
//! # Fixture
//!
//! Mirrors the production card layout: a card zone with three children
//! horizontally — a leaf at x=0 (drag-handle), a child zone at x=10
//! (title field), a leaf at x=20 (inspect) — plus a second child zone
//! at y=20 (tags row).
//!
//! ```text
//! card zone
//! ├── [drag-leaf]     (x=0,  y=0)
//! ├── [title-zone]    (x=10, y=0)
//! ├── [inspect-leaf]  (x=20, y=0)
//! └── [tags-zone]     (x=0,  y=20)  ← spans full width below the row above
//! ```
//!
//! Plus a peer zone (`peer-zone`) at the card's parent level, sitting
//! below the card so iter-1 escalation has a target when iter 0 misses.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    BeamNavStrategy, Direction, FocusLayer, FocusScope, FocusZone, FullyQualifiedMoniker,
    LayerName, NavStrategy, Pixels, Rect, SegmentMoniker, SpatialRegistry, WindowLabel,
};

// ---------------------------------------------------------------------------
// Builders.
// ---------------------------------------------------------------------------

/// Build a [`Rect`] from raw `f64` coordinates.
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

/// FQM for a primitive registered inside a parent zone (`parent_fq`).
fn fq_in_zone(parent_fq: &FullyQualifiedMoniker, segment: &str) -> FullyQualifiedMoniker {
    FullyQualifiedMoniker::compose(parent_fq, &SegmentMoniker::from_string(segment))
}

/// Build a `FocusScope` leaf with the given identity, rect, layer, and
/// optional parent zone. Overrides are intentionally empty.
fn leaf(
    fq: FullyQualifiedMoniker,
    segment: &str,
    layer_fq: &str,
    parent_zone: Option<FullyQualifiedMoniker>,
    r: Rect,
) -> FocusScope {
    FocusScope {
        fq,
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: FullyQualifiedMoniker::from_string(layer_fq),
        parent_zone,
        overrides: HashMap::new(),
    }
}

/// Build a `FocusZone` with the given identity, rect, layer, and
/// optional parent zone.
fn zone(
    fq: FullyQualifiedMoniker,
    segment: &str,
    layer_fq: &str,
    parent_zone: Option<FullyQualifiedMoniker>,
    r: Rect,
) -> FocusZone {
    FocusZone {
        fq,
        segment: SegmentMoniker::from_string(segment),
        rect: r,
        layer_fq: FullyQualifiedMoniker::from_string(layer_fq),
        parent_zone,
        last_focused: None,
        overrides: HashMap::new(),
    }
}

/// Build a `FocusLayer` rooted at `fq_str` with the given segment.
fn layer(fq_str: &str, segment: &str, window: &str, parent: Option<&str>) -> FocusLayer {
    FocusLayer {
        fq: FullyQualifiedMoniker::from_string(fq_str),
        segment: SegmentMoniker::from_string(segment),
        name: LayerName::from_string("window"),
        parent: parent.map(FullyQualifiedMoniker::from_string),
        window_label: WindowLabel::from_string(window),
        last_focused: None,
    }
}

/// Run the default `BeamNavStrategy` and return the navigated-to FQM.
fn nav(
    reg: &SpatialRegistry,
    from: &FullyQualifiedMoniker,
    dir: Direction,
) -> FullyQualifiedMoniker {
    let focused_segment = reg
        .find_by_fq(from)
        .map(|e| e.segment().clone())
        .unwrap_or_else(|| panic!("nav called with unregistered FQM {from:?}"));
    BeamNavStrategy::new().next(reg, from, &focused_segment, dir)
}

// ---------------------------------------------------------------------------
// Fixture handles.
// ---------------------------------------------------------------------------

/// FQM helpers for the card-shaped fixture.
struct CardFixture {
    reg: SpatialRegistry,
    drag: FullyQualifiedMoniker,
    title: FullyQualifiedMoniker,
    inspect: FullyQualifiedMoniker,
    tags: FullyQualifiedMoniker,
    card: FullyQualifiedMoniker,
    peer: FullyQualifiedMoniker,
}

/// Build the fixture described in the module docs:
///
/// - one layer `/L`
/// - one parent zone `card` at (0, 0, 100, 100), parent_zone = None
/// - one peer zone `peer-zone` at (0, 200, 100, 50) at the same level as
///   the card so iter-1 escalation has a target Down from the card
/// - card children: drag-leaf (x=0, y=0), title-zone (x=10, y=0),
///   inspect-leaf (x=20, y=0), tags-zone (x=0, y=20)
fn build_card_fixture() -> CardFixture {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(layer("/L", "L", "main", None));

    // The card zone is the parent zone for the four siblings.
    let card_fq = fq_in_layer("/L", "card");
    reg.register_zone(zone(
        card_fq.clone(),
        "card",
        "/L",
        None,
        rect(0.0, 0.0, 100.0, 100.0),
    ));

    // A peer zone next to the card, used so iter 1 has a Down target
    // when iter 0 misses below the card.
    let peer_fq = fq_in_layer("/L", "peer-zone");
    reg.register_zone(zone(
        peer_fq.clone(),
        "peer-zone",
        "/L",
        None,
        rect(0.0, 200.0, 100.0, 50.0),
    ));

    // Top row: drag (leaf) at x=0, title (zone) at x=10, inspect (leaf)
    // at x=20. All at y=0, height 10. Sibling under the card zone.
    let drag_fq = fq_in_zone(&card_fq, "drag");
    reg.register_scope(leaf(
        drag_fq.clone(),
        "drag",
        "/L",
        Some(card_fq.clone()),
        rect(0.0, 0.0, 8.0, 10.0),
    ));

    let title_fq = fq_in_zone(&card_fq, "title");
    reg.register_zone(zone(
        title_fq.clone(),
        "title",
        "/L",
        Some(card_fq.clone()),
        rect(10.0, 0.0, 8.0, 10.0),
    ));

    let inspect_fq = fq_in_zone(&card_fq, "inspect");
    reg.register_scope(leaf(
        inspect_fq.clone(),
        "inspect",
        "/L",
        Some(card_fq.clone()),
        rect(20.0, 0.0, 8.0, 10.0),
    ));

    // Tags row directly below the top row, spans the full card width
    // so it is in-beam from any of the top-row siblings.
    let tags_fq = fq_in_zone(&card_fq, "tags");
    reg.register_zone(zone(
        tags_fq.clone(),
        "tags",
        "/L",
        Some(card_fq.clone()),
        rect(0.0, 20.0, 28.0, 10.0),
    ));

    CardFixture {
        reg,
        drag: drag_fq,
        title: title_fq,
        inspect: inspect_fq,
        tags: tags_fq,
        card: card_fq,
        peer: peer_fq,
    }
}

// ---------------------------------------------------------------------------
// Iter-0 any-kind: leaf-origin → mixed-kind sibling.
// ---------------------------------------------------------------------------

/// `Right` from the drag-handle leaf at x=0 lands on the title-field
/// zone at x=10 — NOT on the inspect leaf at x=20.
///
/// Pre-fix behaviour: same-kind iter 0 filtered out the title zone and
/// jumped over it to the inspect leaf. Post-fix: any-kind iter 0
/// considers both candidates, and the title (closer) wins.
#[test]
fn right_from_drag_leaf_lands_on_title_zone_not_inspect_leaf() {
    let f = build_card_fixture();
    let landing = nav(&f.reg, &f.drag, Direction::Right);
    assert_eq!(
        landing, f.title,
        "Right from drag leaf must land on the title zone (any-kind in-zone sibling, \
         geometrically closer than the inspect leaf at x=20)"
    );
}

/// `Left` from the inspect leaf at x=20 lands on the title-field zone
/// at x=10 — NOT on the drag-handle leaf at x=0.
#[test]
fn left_from_inspect_leaf_lands_on_title_zone_not_drag_leaf() {
    let f = build_card_fixture();
    let landing = nav(&f.reg, &f.inspect, Direction::Left);
    assert_eq!(
        landing, f.title,
        "Left from inspect leaf must land on the title zone (any-kind in-zone sibling, \
         geometrically closer than the drag leaf at x=0)"
    );
}

/// `Down` from the drag-handle leaf lands on the tags-row zone in the
/// SAME card — NOT on a peer of the card.
///
/// Pre-fix behaviour: same-kind iter 0 found no leaf below in the card,
/// escalated to iter 1, and landed on a peer card / column zone below.
/// Post-fix: any-kind iter 0 finds the tags-row zone as an in-zone
/// sibling and stays inside the card.
#[test]
fn down_from_drag_leaf_lands_on_tags_zone_in_same_card() {
    let f = build_card_fixture();
    let landing = nav(&f.reg, &f.drag, Direction::Down);
    assert_eq!(
        landing, f.tags,
        "Down from drag leaf must land on the tags zone in the same card \
         (any-kind in-zone sibling); must NOT escalate to a peer of the card"
    );
    assert_ne!(
        landing, f.peer,
        "Down from drag leaf must NOT escalate past the card to a peer zone — \
         the tags zone is an in-zone sibling and must win iter 0"
    );
}

/// `Down` from the inspect leaf lands on the tags-row zone in the SAME
/// card — same any-kind in-zone rule as the drag-leaf case.
#[test]
fn down_from_inspect_leaf_lands_on_tags_zone_in_same_card() {
    let f = build_card_fixture();
    let landing = nav(&f.reg, &f.inspect, Direction::Down);
    assert_eq!(
        landing, f.tags,
        "Down from inspect leaf must land on the tags zone in the same card \
         (any-kind in-zone sibling); must NOT escalate to a peer of the card"
    );
    assert_ne!(landing, f.peer);
}

/// `Down` from the bottom-most field zone in the card escalates to
/// iter 1 — returns either the parent's peer zone or drills out to the
/// card itself if no peer.
///
/// In this fixture the card has a peer zone below it (`peer-zone`), so
/// iter 1 finds it.
#[test]
fn down_from_bottommost_in_card_escalates_to_iter1_peer_zone() {
    let f = build_card_fixture();
    let landing = nav(&f.reg, &f.tags, Direction::Down);
    assert_eq!(
        landing, f.peer,
        "Down from the bottom-most child of the card has no in-zone Down peer; \
         iter 0 misses, iter 1 escalates to the card's peer-zone (a sibling at \
         the parent's level)"
    );
}

// ---------------------------------------------------------------------------
// Iter-0 any-kind: zone-origin → mixed-kind sibling. Symmetric coverage
// pinning that the rule works in BOTH directions of the kind mix.
// ---------------------------------------------------------------------------

/// `Right` from the title-field ZONE at x=10 lands on the inspect LEAF
/// at x=20 — any-kind iter 0 happily picks a leaf sibling when one is
/// the geometrically best candidate.
#[test]
fn right_from_title_zone_lands_on_inspect_leaf() {
    let f = build_card_fixture();
    let landing = nav(&f.reg, &f.title, Direction::Right);
    assert_eq!(
        landing, f.inspect,
        "Right from the title zone must land on the inspect leaf (any-kind in-zone \
         sibling). Pre-fix this would have been filtered out by the same-kind iter-0 \
         rule because the focused entry is a zone."
    );
}

/// `Left` from the title-field ZONE at x=10 lands on the drag LEAF at
/// x=0 — symmetric to the previous case.
#[test]
fn left_from_title_zone_lands_on_drag_leaf() {
    let f = build_card_fixture();
    let landing = nav(&f.reg, &f.title, Direction::Left);
    assert_eq!(
        landing, f.drag,
        "Left from the title zone must land on the drag leaf (any-kind in-zone sibling)"
    );
}

/// `Down` from the title-field zone lands on the tags-row zone — both
/// are zones, so this case looks like a same-kind match. Pin it
/// explicitly so the symmetry remains intact even after future tweaks.
#[test]
fn down_from_title_zone_lands_on_tags_zone() {
    let f = build_card_fixture();
    let landing = nav(&f.reg, &f.title, Direction::Down);
    assert_eq!(
        landing, f.tags,
        "Down from the title zone must land on the tags zone (in-zone sibling, both zones)"
    );
}

/// `Up` from the tags-row zone lands on the title-field zone — same
/// kind, both zones. Sanity check that iter 0 still works for
/// same-kind candidates.
#[test]
fn up_from_tags_zone_lands_on_title_zone() {
    let f = build_card_fixture();
    let landing = nav(&f.reg, &f.tags, Direction::Up);
    assert_eq!(
        landing, f.title,
        "Up from the tags zone must land on the title zone (in-zone sibling)"
    );
}

// ---------------------------------------------------------------------------
// Iter 1 still filters by kind: when iter 0 misses, only sibling zones
// of the parent enter the search.
// ---------------------------------------------------------------------------

/// `Right` from the inspect leaf at x=20 (rightmost in the top row of
/// the card) misses iter 0 and escalates. Iter 1 looks at the card's
/// peers — the `peer-zone` is below, not to the right, so iter 1 also
/// misses, and the cascade drills out to the card itself.
#[test]
fn right_from_rightmost_in_row_drills_out_to_card_when_no_iter1_peer_right() {
    let f = build_card_fixture();
    let landing = nav(&f.reg, &f.inspect, Direction::Right);
    assert_eq!(
        landing, f.card,
        "Right from inspect leaf has no in-zone Right peer (it's the rightmost) — \
         iter 0 misses, iter 1 has no Right peer of the card at the layer root \
         (peer-zone is below, not right), and the cascade drills out to the card"
    );
}
