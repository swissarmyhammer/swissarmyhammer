//! Snapshot-path regression suite.
//!
//! Each scenario stages a snapshot mirroring a production kanban-board
//! shape and drives focus / navigate / focus_lost calls through the
//! single snapshot-driven API surface. Assertions cover the produced
//! `FocusChangedEvent` and the kernel's `last_focused_by_fq` writes.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    Direction, FocusLayer, FullyQualifiedMoniker, LayerName, NavSnapshot, Pixels, Rect,
    SegmentMoniker, SnapshotScope, SpatialRegistry, SpatialState, WindowLabel,
};

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        x: Pixels::new(x),
        y: Pixels::new(y),
        width: Pixels::new(w),
        height: Pixels::new(h),
    }
}

fn fq(s: &str) -> FullyQualifiedMoniker {
    FullyQualifiedMoniker::from_string(s)
}

fn make_layer(fq_str: &str, parent: Option<&str>) -> FocusLayer {
    FocusLayer {
        fq: fq(fq_str),
        segment: SegmentMoniker::from_string("window"),
        name: LayerName::from_string("window"),
        parent: parent.map(fq),
        window_label: WindowLabel::from_string("main"),
        last_focused: None,
    }
}

fn snap(fq_str: &str, parent_zone: Option<&str>, r: Rect) -> SnapshotScope {
    SnapshotScope {
        fq: fq(fq_str),
        rect: r,
        parent_zone: parent_zone.map(fq),
        nav_override: HashMap::new(),
        focusable: true,
    }
}

/// Build a 3-column × 3-card snapshot under layer `/L`.
///
/// Returns the snapshot, the column FQMs, and the flat card FQM list
/// in row-major order.
fn build_three_column_snapshot() -> (
    NavSnapshot,
    Vec<FullyQualifiedMoniker>,
    Vec<FullyQualifiedMoniker>,
) {
    let mut scopes = Vec::new();
    let mut columns = Vec::new();
    let mut cards = Vec::new();
    for (col_idx, col_x) in [0.0_f64, 200.0, 400.0].iter().enumerate() {
        let col_fq = fq(&format!("/L/col:{col_idx}"));
        scopes.push(snap(col_fq.as_ref(), None, rect(*col_x, 0.0, 180.0, 600.0)));
        columns.push(col_fq.clone());

        for row in 0..3 {
            let card_fq = fq(&format!("/L/col:{col_idx}/card:{row}"));
            scopes.push(snap(
                card_fq.as_ref(),
                Some(col_fq.as_ref()),
                rect(*col_x + 10.0, 40.0 + (row as f64) * 80.0, 160.0, 60.0),
            ));
            cards.push(card_fq);
        }
    }

    let snapshot = NavSnapshot {
        layer_fq: fq("/L"),
        scopes,
    };
    (snapshot, columns, cards)
}

/// Down-arrow on a card lands on the card directly below it.
#[test]
fn arrow_down_picks_card_below() {
    let (snapshot, _columns, cards) = build_three_column_snapshot();
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", None));
    let mut state = SpatialState::new();

    state
        .focus(&mut reg, &snapshot, cards[0].clone(), None)
        .expect("focus first card");
    let event = state
        .navigate(&mut reg, &snapshot, cards[0].clone(), Direction::Down, None)
        .expect("Down moves to next card");

    assert_eq!(event.next_fq, Some(cards[1].clone()));
}

/// Right-arrow on the leftmost card crosses into the next column.
#[test]
fn arrow_right_crosses_columns() {
    let (snapshot, _columns, cards) = build_three_column_snapshot();
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", None));
    let mut state = SpatialState::new();

    state
        .focus(&mut reg, &snapshot, cards[0].clone(), None)
        .expect("focus first card");
    let event = state
        .navigate(
            &mut reg,
            &snapshot,
            cards[0].clone(),
            Direction::Right,
            None,
        )
        .expect("Right crosses columns");

    // First card in column 1 is at index 3 in the row-major list.
    assert_eq!(event.next_fq, Some(cards[3].clone()));
}

/// Click focus emits an event with the expected prev/next pair and
/// records the column zone in `last_focused_by_fq`.
#[test]
fn click_focus_writes_ancestor_walk() {
    let (snapshot, columns, cards) = build_three_column_snapshot();
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", None));
    let mut state = SpatialState::new();

    let event = state
        .focus(&mut reg, &snapshot, cards[1].clone(), None)
        .expect("focus emits");
    assert_eq!(event.prev_fq, None);
    assert_eq!(event.next_fq, Some(cards[1].clone()));
    assert_eq!(
        reg.last_focused_by_fq.get(&columns[0]),
        Some(&cards[1]),
        "column ancestor records the focused card",
    );
}

/// Filter changes that hide the focused card → `focus_lost` cascades
/// to a sibling.
#[test]
fn filter_change_hides_focused_card_falls_back_to_sibling() {
    let (pre_snapshot, _columns, cards) = build_three_column_snapshot();
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", None));
    let mut state = SpatialState::new();

    let lost = cards[1].clone();
    state
        .focus(&mut reg, &pre_snapshot, lost.clone(), None)
        .expect("focus middle card");

    // Post-filter snapshot: drop the focused card, keep the rest.
    let post_snapshot = NavSnapshot {
        layer_fq: fq("/L"),
        scopes: pre_snapshot
            .scopes
            .iter()
            .filter(|s| s.fq != lost)
            .cloned()
            .collect(),
    };

    let event = state
        .focus_lost(
            &mut reg,
            &post_snapshot,
            &lost,
            Some(&fq("/L/col:0")),
            &fq("/L"),
            rect(10.0, 120.0, 160.0, 60.0),
            None,
        )
        .expect("focus_lost emits");

    // Sibling fallback should land on a sibling under col:0.
    let next = event.next_fq.expect("a sibling target was resolved");
    let next_str: &str = next.as_ref();
    assert!(
        next_str.starts_with("/L/col:0/card:"),
        "expected sibling under col:0, got {next}"
    );
    assert_ne!(next, lost, "fallback must not pick the lost FQM");
}

/// Bulk delete of the focused card's entire column → cascade walks
/// upward to a layer-root sibling (the nearest column zone).
#[test]
fn bulk_delete_falls_back_to_layer_root_sibling() {
    let (pre_snapshot, _columns, cards) = build_three_column_snapshot();
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", None));
    let mut state = SpatialState::new();

    let lost = cards[1].clone();
    state
        .focus(&mut reg, &pre_snapshot, lost.clone(), None)
        .expect("focus middle card of col:0");

    // Post-delete snapshot: drop the entire col:0 + all its cards.
    let post_snapshot = NavSnapshot {
        layer_fq: fq("/L"),
        scopes: pre_snapshot
            .scopes
            .iter()
            .filter(|s| {
                let s_str: &str = s.fq.as_ref();
                !s_str.starts_with("/L/col:0")
            })
            .cloned()
            .collect(),
    };

    let event = state
        .focus_lost(
            &mut reg,
            &post_snapshot,
            &lost,
            Some(&fq("/L/col:0")),
            &fq("/L"),
            rect(10.0, 120.0, 160.0, 60.0),
            None,
        )
        .expect("focus_lost emits");

    assert_eq!(event.prev_fq, Some(lost));
    let next = event
        .next_fq
        .expect("cascade resolves to a layer-root zone");
    let next_str: &str = next.as_ref();
    assert!(
        matches!(next_str, "/L/col:1" | "/L/col:2"),
        "expected the cascade to land on a surviving column zone, got {next}"
    );
}

/// Layer push (inspector) followed by focus inside, then focus_lost
/// resolves within the inspector layer.
#[test]
fn inspector_layer_focus_lost_resolves_within_layer() {
    let inspector_layer = fq("/L/inspector");
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", None));
    reg.push_layer(make_layer(inspector_layer.as_ref(), Some("/L")));
    let mut state = SpatialState::new();

    let panel = fq("/L/inspector/panel");
    let title = fq("/L/inspector/panel/title");
    let status = fq("/L/inspector/panel/status");

    let pre = NavSnapshot {
        layer_fq: inspector_layer.clone(),
        scopes: vec![
            snap(panel.as_ref(), None, rect(800.0, 0.0, 400.0, 600.0)),
            snap(
                title.as_ref(),
                Some(panel.as_ref()),
                rect(810.0, 10.0, 380.0, 40.0),
            ),
            snap(
                status.as_ref(),
                Some(panel.as_ref()),
                rect(810.0, 60.0, 380.0, 40.0),
            ),
        ],
    };
    state
        .focus(&mut reg, &pre, title.clone(), None)
        .expect("focus title");

    let post = NavSnapshot {
        layer_fq: inspector_layer.clone(),
        scopes: vec![
            snap(panel.as_ref(), None, rect(800.0, 0.0, 400.0, 600.0)),
            snap(
                status.as_ref(),
                Some(panel.as_ref()),
                rect(810.0, 60.0, 380.0, 40.0),
            ),
        ],
    };
    let event = state
        .focus_lost(
            &mut reg,
            &post,
            &title,
            Some(&panel),
            &inspector_layer,
            rect(810.0, 10.0, 380.0, 40.0),
            None,
        )
        .expect("focus_lost emits");

    // Sibling fallback inside the inspector panel.
    assert_eq!(event.next_fq, Some(status));
}

/// Re-focus is a no-op (no event emitted, no ancestor walk re-run).
#[test]
fn re_focus_same_fq_emits_no_event() {
    let (snapshot, _columns, cards) = build_three_column_snapshot();
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", None));
    let mut state = SpatialState::new();

    state
        .focus(&mut reg, &snapshot, cards[0].clone(), None)
        .expect("first focus emits");
    let second = state.focus(&mut reg, &snapshot, cards[0].clone(), None);
    assert!(second.is_none(), "re-focus same FQM must not emit");
}
