//! Snapshot-path regression suite covering every production navigation
//! scenario.
//!
//! The kernel sees scope state only via per-decision snapshots. This
//! suite drives each production scenario through the snapshot path
//! (`navigate_with_snapshot`, `focus_with_snapshot`, `focus_lost`) and
//! asserts on the resulting `FocusChangedEvent` — focus targets,
//! fallback resolutions, ancestor-walk side effects — not just absence
//! of panic.
//!
//! Scenarios covered:
//!
//! 1. Arrow nav across all four directions from every position on a
//!    multi-column board (every card finds at least one peer).
//! 2. Click focus on every scope kind (chip, field, button, card,
//!    column header) — each click emits an event with the expected
//!    prev/next pair and writes through the ancestor walk.
//! 3. Drag-drop a card between columns and nav from the moved card —
//!    the click commits, the parent-zone walk records `last_focused`
//!    on the new column, and left-nav crosses into the previous
//!    column.
//! 4. Filter changes that hide the focused row — `focus_lost`
//!    cascades to a sibling.
//! 5. Layer push (open inspector), nav inside, focus loss — fallback
//!    stays inside the inspector layer.
//! 6. Modal dialog push, focus inside, cancel — fallback resolves to
//!    the surviving sibling inside the modal.
//! 7. Bulk actions deleting multiple cards including the focused one
//!    — fallback emits and prev_fq matches the deleted FQM.

use std::collections::HashMap;

use swissarmyhammer_focus::{
    Direction, FocusLayer, FocusOverrides, FocusScope, FullyQualifiedMoniker, LayerName,
    NavSnapshot, Pixels, Rect, SegmentMoniker, SnapshotScope, SpatialRegistry, SpatialState,
    WindowLabel,
};

// ---------------------------------------------------------------------------
// Builders mirroring the kanban board's typical scope shape.
// ---------------------------------------------------------------------------

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

fn seg(s: &str) -> SegmentMoniker {
    SegmentMoniker::from_string(s)
}

fn make_layer(
    fq_str: &str,
    parent: Option<&str>,
    last_focused: Option<FullyQualifiedMoniker>,
) -> FocusLayer {
    FocusLayer {
        fq: fq(fq_str),
        segment: seg("window"),
        name: LayerName::from_string("window"),
        parent: parent.map(fq),
        window_label: WindowLabel::from_string("main"),
        last_focused,
    }
}

fn leaf(
    fq_str: &str,
    segment_str: &str,
    layer: &str,
    parent_zone: Option<&str>,
    r: Rect,
) -> FocusScope {
    FocusScope {
        fq: fq(fq_str),
        segment: seg(segment_str),
        rect: r,
        layer_fq: fq(layer),
        parent_zone: parent_zone.map(fq),
        overrides: HashMap::new(),
        last_focused: None,
    }
}

fn snapshot_for_layer(reg: &SpatialRegistry, layer_fq: &FullyQualifiedMoniker) -> NavSnapshot {
    NavSnapshot {
        scopes: reg
            .scopes_in_layer(layer_fq)
            .map(|s| SnapshotScope {
                fq: s.fq.clone(),
                rect: s.rect,
                parent_zone: s.parent_zone.clone(),
                nav_override: s.overrides.clone(),
            })
            .collect(),
        layer_fq: layer_fq.clone(),
    }
}

fn snapshot_excluding(
    reg: &SpatialRegistry,
    layer_fq: &FullyQualifiedMoniker,
    omit_fq: &FullyQualifiedMoniker,
) -> NavSnapshot {
    NavSnapshot {
        scopes: reg
            .scopes_in_layer(layer_fq)
            .filter(|s| &s.fq != omit_fq)
            .map(|s| SnapshotScope {
                fq: s.fq.clone(),
                rect: s.rect,
                parent_zone: s.parent_zone.clone(),
                nav_override: FocusOverrides::new(),
            })
            .collect(),
        layer_fq: layer_fq.clone(),
    }
}

// ---------------------------------------------------------------------------
// Scenario fixtures
// ---------------------------------------------------------------------------

/// Build a 3-column × 3-card kanban-shaped registry under a single
/// layer. Columns are zones; cards are leaves under each column zone.
/// Returns the registry, layer FQM, the per-column zone FQMs, and the
/// flat list of card FQMs in left-to-right, top-to-bottom order.
fn build_three_column_board() -> (
    SpatialRegistry,
    FullyQualifiedMoniker,
    Vec<FullyQualifiedMoniker>,
    Vec<FullyQualifiedMoniker>,
) {
    let mut reg = SpatialRegistry::new();
    let layer_fq = fq("/L");
    reg.push_layer(make_layer("/L", None, None));

    let mut columns = Vec::new();
    let mut cards = Vec::new();
    for (col_idx, col_x) in [0.0_f64, 200.0, 400.0].iter().enumerate() {
        let col_fq = fq(&format!("/L/col:{col_idx}"));
        reg.register_scope(leaf(
            &format!("/L/col:{col_idx}"),
            &format!("col:{col_idx}"),
            "/L",
            None,
            rect(*col_x, 0.0, 180.0, 600.0),
        ));
        columns.push(col_fq.clone());
        for (row_idx, row_y) in [40.0_f64, 140.0, 240.0].iter().enumerate() {
            let card_fq_str = format!("/L/col:{col_idx}/card:{col_idx}-{row_idx}");
            reg.register_scope(leaf(
                &card_fq_str,
                &format!("card:{col_idx}-{row_idx}"),
                "/L",
                Some(col_fq.as_str()),
                rect(col_x + 10.0, *row_y, 160.0, 80.0),
            ));
            cards.push(fq(&card_fq_str));
        }
    }
    (reg, layer_fq, columns, cards)
}

// ---------------------------------------------------------------------------
// Scenario 1: arrow nav across all four directions from every position.
// ---------------------------------------------------------------------------

/// Drive every cardinal direction from every card on a 3×3 board through
/// the snapshot-driven nav path. Every card on the interior or edge has
/// at least one navigable peer, so each card must produce a focus event
/// for at least one direction.
#[test]
fn soak_arrow_nav_every_card_every_direction() {
    let (mut reg, layer_fq, _columns, cards) = build_three_column_board();
    let snapshot = snapshot_for_layer(&reg, &layer_fq);

    for card in &cards {
        let mut state = SpatialState::new();
        state.focus(&mut reg, card.clone()).expect("focus card");
        let mut moved_at_least_once = false;
        for direction in [
            Direction::Up,
            Direction::Down,
            Direction::Left,
            Direction::Right,
        ] {
            // Snapshot-only path — never panics, returns a sensible
            // result whether or not focus actually moved.
            let event = state.navigate_with_snapshot(&mut reg, &snapshot, card.clone(), direction);
            if event.is_some() {
                moved_at_least_once = true;
            }
        }
        assert!(
            moved_at_least_once,
            "every card on a 3x3 board has at least one direction with a peer; {card:?} found none",
        );
    }
}

// ---------------------------------------------------------------------------
// Scenario 2: click focus on every scope kind.
// ---------------------------------------------------------------------------

/// Drive `focus_with_snapshot` on every scope kind: chip, field, button,
/// card, column header. Each click commits focus on its target and runs
/// the ancestor `record_focus` walk. The first click on a given window
/// emits a `prev=None → next=Some(scope)` event; subsequent clicks emit
/// a `prev=Some(prior) → next=Some(scope)` transition.
#[test]
fn soak_click_focus_every_scope_kind() {
    let mut reg = SpatialRegistry::new();
    let layer_fq = fq("/L");
    reg.push_layer(make_layer("/L", None, None));

    let col_fq = fq("/L/col:1");
    reg.register_scope(leaf(
        col_fq.as_str(),
        "col:1",
        "/L",
        None,
        rect(0.0, 0.0, 200.0, 600.0),
    ));
    let col_header = fq("/L/col:1/header");
    reg.register_scope(leaf(
        col_header.as_str(),
        "header",
        "/L",
        Some(col_fq.as_str()),
        rect(0.0, 0.0, 200.0, 30.0),
    ));

    let card_fq = fq("/L/col:1/card:01");
    reg.register_scope(leaf(
        card_fq.as_str(),
        "card:01",
        "/L",
        Some(col_fq.as_str()),
        rect(10.0, 40.0, 180.0, 100.0),
    ));
    let chip = fq("/L/col:1/card:01/tag:bug");
    reg.register_scope(leaf(
        chip.as_str(),
        "tag:bug",
        "/L",
        Some(card_fq.as_str()),
        rect(20.0, 50.0, 30.0, 16.0),
    ));
    let field = fq("/L/col:1/card:01/field:title");
    reg.register_scope(leaf(
        field.as_str(),
        "field:title",
        "/L",
        Some(card_fq.as_str()),
        rect(20.0, 70.0, 150.0, 20.0),
    ));
    let button = fq("/L/col:1/card:01/btn:open");
    reg.register_scope(leaf(
        button.as_str(),
        "btn:open",
        "/L",
        Some(card_fq.as_str()),
        rect(20.0, 100.0, 60.0, 20.0),
    ));

    let snapshot = snapshot_for_layer(&reg, &layer_fq);
    let mut state = SpatialState::new();

    let click_order = [&col_header, &card_fq, &chip, &field, &button];
    let mut prev_fq: Option<FullyQualifiedMoniker> = None;
    for fq_to_click in click_order {
        let event = state
            .focus_with_snapshot(&mut reg, &snapshot, fq_to_click.clone())
            .expect("click commits focus and emits an event");
        assert_eq!(event.next_fq.as_ref(), Some(fq_to_click));
        assert_eq!(event.prev_fq, prev_fq);
        // The ancestor walk records `last_focused` on this scope's
        // parent_zone (when present) and on its layer.
        let layer = reg.layer(&layer_fq).expect("layer present");
        assert_eq!(layer.last_focused.as_ref(), Some(fq_to_click));
        prev_fq = Some(fq_to_click.clone());
    }
    // After all clicks, the column zone records the deepest leaf — the
    // ancestor walk has run on every click and the most recent descendant
    // wins.
    let col_entry = reg.find_by_fq(&col_fq).expect("col still registered");
    let col_last = col_entry.last_focused.clone();
    assert!(
        col_last.is_some(),
        "ancestor walk records last_focused on the parent zone"
    );
}

// ---------------------------------------------------------------------------
// Scenario 3: drag-drop a card between columns; nav from the moved card.
// ---------------------------------------------------------------------------

/// Drag-drop scenario: relocate a card from one column to another.
/// Click-focus on the moved card commits and writes through the new
/// parent zone via the ancestor walk; left-nav crosses the column
/// boundary into the previous column.
#[test]
fn soak_drag_drop_card_between_columns() {
    let (mut reg, layer_fq, _columns, _cards) = build_three_column_board();

    let moved_old = fq("/L/col:0/card:0-1");
    let moved_new = fq("/L/col:2/card:moved");
    reg.unregister_scope(&moved_old);
    reg.register_scope(leaf(
        moved_new.as_str(),
        "card:moved",
        "/L",
        Some("/L/col:2"),
        rect(410.0, 140.0, 160.0, 80.0),
    ));

    let snapshot = snapshot_for_layer(&reg, &layer_fq);
    let mut state = SpatialState::new();

    // Click-focus on the moved card via the snapshot path — exercises
    // the ancestor walk through the new parent zone. `record_focus`
    // walks the snapshot's parent_zone chain and dual-writes into
    // `last_focused_by_fq`.
    let click = state
        .focus_with_snapshot(&mut reg, &snapshot, moved_new.clone())
        .expect("snapshot click on moved card emits an event");
    assert_eq!(click.next_fq.as_ref(), Some(&moved_new));
    let new_zone = fq("/L/col:2");
    assert_eq!(
        reg.last_focused_by_fq.get(&new_zone),
        Some(&moved_new),
        "ancestor walk records last_focused on the new parent zone",
    );

    // Left-from-col-2 should land in col-1 (any of /L/col:1/card:1-*).
    let left = state
        .navigate_with_snapshot(&mut reg, &snapshot, moved_new.clone(), Direction::Left)
        .expect("left from col 2 has a peer in col 1");
    let left_target = left.next_fq.expect("event has a next target");
    assert!(
        left_target.as_str().starts_with("/L/col:1/"),
        "left nav crosses column boundary: {left_target:?}",
    );
}

// ---------------------------------------------------------------------------
// Scenario 4: filter changes that hide the focused row; focus restored.
// ---------------------------------------------------------------------------

/// Filter scenario: a filter change hides the focused card. The
/// snapshot excludes the lost FQM. The kernel's `focus_lost` cascade
/// picks a fallback target and emits an event whose `prev_fq` matches
/// the lost card.
#[test]
fn soak_filter_hides_focused_row_focus_restored() {
    let (mut reg, layer_fq, _columns, cards) = build_three_column_board();
    let focused = cards[4].clone(); // /L/col:1/card:1-1
    let mut state = SpatialState::new();
    state.focus(&mut reg, focused.clone()).expect("focus");

    let snapshot = snapshot_excluding(&reg, &layer_fq, &focused);
    let lost_parent_zone = reg.find_by_fq(&focused).and_then(|s| s.parent_zone.clone());
    let lost_rect = reg.find_by_fq(&focused).map(|s| s.rect).expect("rect");

    let event = state.focus_lost(
        &mut reg,
        &snapshot,
        &focused,
        lost_parent_zone.as_ref(),
        &layer_fq,
        lost_rect,
    );
    assert!(
        event.is_some(),
        "focus_lost on a filtered-out card produces a fallback event",
    );
    let event = event.unwrap();
    assert_eq!(event.prev_fq, Some(focused));
}

// ---------------------------------------------------------------------------
// Scenario 5: layer push (open inspector) → nav inside → layer pop →
//             focus restored.
// ---------------------------------------------------------------------------

/// Inspector lifecycle: a child layer pushes, focus moves inside,
/// down-nav lands on the body field, and a focus loss inside the
/// inspector resolves to the sibling field — the fallback stays inside
/// the inspector layer rather than escaping to the parent.
#[test]
fn soak_inspector_push_nav_pop_focus_restored() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", None, None));

    let card_fq = fq("/L/card:01");
    reg.register_scope(leaf(
        card_fq.as_str(),
        "card:01",
        "/L",
        None,
        rect(0.0, 0.0, 200.0, 100.0),
    ));

    let mut state = SpatialState::new();
    state
        .focus(&mut reg, card_fq.clone())
        .expect("focus card before inspector");

    let inspector_layer_fq = fq("/L/inspector");
    reg.push_layer(make_layer("/L/inspector", Some("/L"), None));
    let insp_a = fq("/L/inspector/field:title");
    reg.register_scope(leaf(
        insp_a.as_str(),
        "field:title",
        "/L/inspector",
        None,
        rect(0.0, 0.0, 300.0, 30.0),
    ));
    let insp_b = fq("/L/inspector/field:body");
    reg.register_scope(leaf(
        insp_b.as_str(),
        "field:body",
        "/L/inspector",
        None,
        rect(0.0, 50.0, 300.0, 100.0),
    ));

    let inspector_snapshot = snapshot_for_layer(&reg, &inspector_layer_fq);

    // Focus inside the inspector — emits prev=card, next=insp_a.
    let inside = state
        .focus_with_snapshot(&mut reg, &inspector_snapshot, insp_a.clone())
        .expect("focus inside inspector emits an event");
    assert_eq!(inside.prev_fq.as_ref(), Some(&card_fq));
    assert_eq!(inside.next_fq.as_ref(), Some(&insp_a));

    // Down-nav inside the inspector lands on the body field.
    let down = state
        .navigate_with_snapshot(
            &mut reg,
            &inspector_snapshot,
            insp_a.clone(),
            Direction::Down,
        )
        .expect("down nav from title finds body inside inspector");
    assert_eq!(down.next_fq.as_ref(), Some(&insp_b));

    // Inspector field unmounts: snapshot for the inspector layer drops
    // the lost FQM; kernel cascades to a sibling. The fallback target
    // must be the surviving inspector field, not anything from /L.
    let snapshot_without_b = snapshot_excluding(&reg, &inspector_layer_fq, &insp_b);
    let lost_rect_b = reg.find_by_fq(&insp_b).map(|s| s.rect).expect("rect");
    let lost_event = state
        .focus_lost(
            &mut reg,
            &snapshot_without_b,
            &insp_b,
            None,
            &inspector_layer_fq,
            lost_rect_b,
        )
        .expect("focus_lost on the focused inspector field emits an event");
    assert_eq!(lost_event.prev_fq.as_ref(), Some(&insp_b));
    assert_eq!(
        lost_event.next_fq.as_ref(),
        Some(&insp_a),
        "fallback stays inside the inspector layer (sibling), not the parent layer",
    );
}

// ---------------------------------------------------------------------------
// Scenario 6: modal dialog push → focus inside → cancel → focus restored.
// ---------------------------------------------------------------------------

/// Modal dialog lifecycle: same shape as the inspector scenario.
/// Right/left nav between confirm and cancel resolves to the matching
/// sibling, and dismissing cancel resolves the fallback to confirm —
/// staying inside the modal layer.
#[test]
fn soak_modal_push_focus_cancel_focus_restored() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", None, None));

    let trigger = fq("/L/btn:open-modal");
    reg.register_scope(leaf(
        trigger.as_str(),
        "btn:open-modal",
        "/L",
        None,
        rect(0.0, 0.0, 100.0, 30.0),
    ));

    let mut state = SpatialState::new();
    state
        .focus(&mut reg, trigger.clone())
        .expect("focus trigger");

    let modal_layer_fq = fq("/L/modal");
    reg.push_layer(make_layer("/L/modal", Some("/L"), None));
    let confirm = fq("/L/modal/btn:confirm");
    reg.register_scope(leaf(
        confirm.as_str(),
        "btn:confirm",
        "/L/modal",
        None,
        rect(40.0, 100.0, 80.0, 30.0),
    ));
    let cancel = fq("/L/modal/btn:cancel");
    reg.register_scope(leaf(
        cancel.as_str(),
        "btn:cancel",
        "/L/modal",
        None,
        rect(140.0, 100.0, 80.0, 30.0),
    ));

    let modal_snapshot = snapshot_for_layer(&reg, &modal_layer_fq);

    // Click into the modal: focus confirm.
    let into = state
        .focus_with_snapshot(&mut reg, &modal_snapshot, confirm.clone())
        .expect("click confirm inside modal emits an event");
    assert_eq!(into.next_fq.as_ref(), Some(&confirm));

    // Right from confirm lands on cancel.
    let right = state
        .navigate_with_snapshot(&mut reg, &modal_snapshot, confirm.clone(), Direction::Right)
        .expect("right from confirm finds cancel inside the modal");
    assert_eq!(right.next_fq.as_ref(), Some(&cancel));

    // Left from cancel returns to confirm.
    let left = state
        .navigate_with_snapshot(&mut reg, &modal_snapshot, cancel.clone(), Direction::Left)
        .expect("left from cancel finds confirm inside the modal");
    assert_eq!(left.next_fq.as_ref(), Some(&confirm));

    // Cancel dismisses (modal still open): snapshot omits cancel; the
    // fallback resolution should pick confirm — the surviving sibling
    // inside the modal layer — not the trigger from /L.
    let snapshot_without_cancel = snapshot_excluding(&reg, &modal_layer_fq, &cancel);
    let lost_rect_cancel = reg.find_by_fq(&cancel).map(|s| s.rect).expect("rect");
    // Move focus onto cancel so focus_lost has something to drop.
    state
        .focus_with_snapshot(&mut reg, &modal_snapshot, cancel.clone())
        .expect("focus cancel before dismissal");
    let lost = state
        .focus_lost(
            &mut reg,
            &snapshot_without_cancel,
            &cancel,
            None,
            &modal_layer_fq,
            lost_rect_cancel,
        )
        .expect("focus_lost on cancel emits a fallback event");
    assert_eq!(lost.prev_fq.as_ref(), Some(&cancel));
    assert_eq!(
        lost.next_fq.as_ref(),
        Some(&confirm),
        "fallback stays inside the modal layer",
    );
}

// ---------------------------------------------------------------------------
// Scenario 7: bulk actions delete multiple cards including the focused.
// ---------------------------------------------------------------------------

/// Bulk delete scenario: the focused card disappears alongside several
/// peers. The deletion listener fires only for the focused FQM, so the
/// snapshot it builds excludes only that FQM — sibling bulk-deletes
/// produce their own separate notifications. The kernel's `focus_lost`
/// cascade resolves a fallback target inside the same layer.
#[test]
fn soak_bulk_delete_includes_focused_card() {
    let (mut reg, layer_fq, _columns, cards) = build_three_column_board();
    let focused = cards[4].clone(); // /L/col:1/card:1-1
    let mut state = SpatialState::new();
    state.focus(&mut reg, focused.clone()).expect("focus");

    let lost_rect = reg.find_by_fq(&focused).map(|s| s.rect).expect("rect");
    let lost_parent_zone = reg.find_by_fq(&focused).and_then(|s| s.parent_zone.clone());
    let snapshot = snapshot_excluding(&reg, &layer_fq, &focused);

    let event = state.focus_lost(
        &mut reg,
        &snapshot,
        &focused,
        lost_parent_zone.as_ref(),
        &layer_fq,
        lost_rect,
    );
    assert!(
        event.is_some(),
        "bulk delete of the focused card produces a fallback event",
    );
}

// ---------------------------------------------------------------------------
// Cross-cutting: every scenario in sequence on a fresh kernel.
// Mirrors a continuous "user works the board for a while" flow.
// ---------------------------------------------------------------------------

/// Run every scenario back-to-back, mirroring a continuous user
/// session. Each phase asserts the same key behavioral outcome as its
/// dedicated scenario test.
#[test]
fn soak_all_scenarios_in_sequence() {
    run_scenario_arrow_nav();
    run_scenario_click_focus();
    run_scenario_drag_drop();
    run_scenario_filter_hide();
    run_scenario_inspector();
    run_scenario_modal();
    run_scenario_bulk_delete();
}

// Compact fixtures the cross-cutting test runs in sequence. Each one
// asserts on the same key behavioral outcome as its dedicated scenario
// test (event presence, fallback target, cross-column nav) so a panic
// in any one phase or a silent change in fallback resolution still
// fails the suite.

fn run_scenario_arrow_nav() {
    let (mut reg, layer_fq, _columns, cards) = build_three_column_board();
    let snapshot = snapshot_for_layer(&reg, &layer_fq);
    for card in &cards {
        let mut state = SpatialState::new();
        state.focus(&mut reg, card.clone()).expect("focus card");
        let mut moved = false;
        for direction in [
            Direction::Up,
            Direction::Down,
            Direction::Left,
            Direction::Right,
        ] {
            if state
                .navigate_with_snapshot(&mut reg, &snapshot, card.clone(), direction)
                .is_some()
            {
                moved = true;
            }
        }
        assert!(moved, "card {card:?} has at least one navigable peer");
    }
}

fn run_scenario_click_focus() {
    let mut reg = SpatialRegistry::new();
    let layer_fq = fq("/L");
    reg.push_layer(make_layer("/L", None, None));
    let col_fq = fq("/L/col:1");
    reg.register_scope(leaf(
        col_fq.as_str(),
        "col:1",
        "/L",
        None,
        rect(0.0, 0.0, 200.0, 600.0),
    ));
    let card = fq("/L/col:1/card:01");
    reg.register_scope(leaf(
        card.as_str(),
        "card:01",
        "/L",
        Some(col_fq.as_str()),
        rect(10.0, 40.0, 180.0, 100.0),
    ));
    let chip = fq("/L/col:1/card:01/tag:bug");
    reg.register_scope(leaf(
        chip.as_str(),
        "tag:bug",
        "/L",
        Some(card.as_str()),
        rect(20.0, 50.0, 30.0, 16.0),
    ));
    let snapshot = snapshot_for_layer(&reg, &layer_fq);
    let mut state = SpatialState::new();
    let card_event = state
        .focus_with_snapshot(&mut reg, &snapshot, card.clone())
        .expect("first click commits focus");
    assert_eq!(card_event.next_fq.as_ref(), Some(&card));
    let chip_event = state
        .focus_with_snapshot(&mut reg, &snapshot, chip.clone())
        .expect("second click transitions focus");
    assert_eq!(chip_event.prev_fq.as_ref(), Some(&card));
    assert_eq!(chip_event.next_fq.as_ref(), Some(&chip));
}

fn run_scenario_drag_drop() {
    let (mut reg, layer_fq, _columns, _cards) = build_three_column_board();
    let moved_old = fq("/L/col:0/card:0-1");
    let moved_new = fq("/L/col:2/card:moved");
    reg.unregister_scope(&moved_old);
    reg.register_scope(leaf(
        moved_new.as_str(),
        "card:moved",
        "/L",
        Some("/L/col:2"),
        rect(410.0, 140.0, 160.0, 80.0),
    ));
    let snapshot = snapshot_for_layer(&reg, &layer_fq);
    let mut state = SpatialState::new();
    let click = state
        .focus_with_snapshot(&mut reg, &snapshot, moved_new.clone())
        .expect("snapshot click on moved card emits an event");
    assert_eq!(click.next_fq.as_ref(), Some(&moved_new));
    let left = state
        .navigate_with_snapshot(&mut reg, &snapshot, moved_new.clone(), Direction::Left)
        .expect("left from moved card finds a peer in col 1");
    let left_target = left.next_fq.expect("event has a next target");
    assert!(
        left_target.as_str().starts_with("/L/col:1/"),
        "left nav crosses column boundary: {left_target:?}",
    );
}

fn run_scenario_filter_hide() {
    let (mut reg, layer_fq, _columns, cards) = build_three_column_board();
    let focused = cards[4].clone();
    let mut state = SpatialState::new();
    state.focus(&mut reg, focused.clone()).expect("focus");
    let snapshot = snapshot_excluding(&reg, &layer_fq, &focused);
    let lost_parent_zone = reg.find_by_fq(&focused).and_then(|s| s.parent_zone.clone());
    let lost_rect = reg.find_by_fq(&focused).map(|s| s.rect).expect("rect");
    let event = state
        .focus_lost(
            &mut reg,
            &snapshot,
            &focused,
            lost_parent_zone.as_ref(),
            &layer_fq,
            lost_rect,
        )
        .expect("filter-hide fallback emits an event");
    assert_eq!(event.prev_fq.as_ref(), Some(&focused));
}

fn run_scenario_inspector() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", None, None));
    let card_fq = fq("/L/card:01");
    reg.register_scope(leaf(
        card_fq.as_str(),
        "card:01",
        "/L",
        None,
        rect(0.0, 0.0, 200.0, 100.0),
    ));
    let mut state = SpatialState::new();
    state.focus(&mut reg, card_fq.clone()).expect("focus");
    let inspector_layer_fq = fq("/L/inspector");
    reg.push_layer(make_layer("/L/inspector", Some("/L"), None));
    let insp_a = fq("/L/inspector/field:title");
    reg.register_scope(leaf(
        insp_a.as_str(),
        "field:title",
        "/L/inspector",
        None,
        rect(0.0, 0.0, 300.0, 30.0),
    ));
    let insp_b = fq("/L/inspector/field:body");
    reg.register_scope(leaf(
        insp_b.as_str(),
        "field:body",
        "/L/inspector",
        None,
        rect(0.0, 50.0, 300.0, 100.0),
    ));
    let snapshot = snapshot_for_layer(&reg, &inspector_layer_fq);
    let inside = state
        .focus_with_snapshot(&mut reg, &snapshot, insp_a.clone())
        .expect("focus inside inspector");
    assert_eq!(inside.next_fq.as_ref(), Some(&insp_a));
    let down = state
        .navigate_with_snapshot(&mut reg, &snapshot, insp_a.clone(), Direction::Down)
        .expect("down from title finds body");
    assert_eq!(down.next_fq.as_ref(), Some(&insp_b));

    let snapshot_without_b = snapshot_excluding(&reg, &inspector_layer_fq, &insp_b);
    let lost_rect_b = reg.find_by_fq(&insp_b).map(|s| s.rect).expect("rect");
    let lost = state
        .focus_lost(
            &mut reg,
            &snapshot_without_b,
            &insp_b,
            None,
            &inspector_layer_fq,
            lost_rect_b,
        )
        .expect("focus_lost emits fallback event");
    assert_eq!(lost.next_fq.as_ref(), Some(&insp_a));
}

fn run_scenario_modal() {
    let mut reg = SpatialRegistry::new();
    reg.push_layer(make_layer("/L", None, None));
    let trigger = fq("/L/btn:open-modal");
    reg.register_scope(leaf(
        trigger.as_str(),
        "btn:open-modal",
        "/L",
        None,
        rect(0.0, 0.0, 100.0, 30.0),
    ));
    let mut state = SpatialState::new();
    state.focus(&mut reg, trigger.clone()).expect("focus");
    let modal_layer_fq = fq("/L/modal");
    reg.push_layer(make_layer("/L/modal", Some("/L"), None));
    let confirm = fq("/L/modal/btn:confirm");
    reg.register_scope(leaf(
        confirm.as_str(),
        "btn:confirm",
        "/L/modal",
        None,
        rect(40.0, 100.0, 80.0, 30.0),
    ));
    let cancel = fq("/L/modal/btn:cancel");
    reg.register_scope(leaf(
        cancel.as_str(),
        "btn:cancel",
        "/L/modal",
        None,
        rect(140.0, 100.0, 80.0, 30.0),
    ));
    let modal_snapshot = snapshot_for_layer(&reg, &modal_layer_fq);
    let into = state
        .focus_with_snapshot(&mut reg, &modal_snapshot, cancel.clone())
        .expect("click cancel inside modal");
    assert_eq!(into.next_fq.as_ref(), Some(&cancel));

    let snapshot_without_cancel = snapshot_excluding(&reg, &modal_layer_fq, &cancel);
    let lost_rect = reg.find_by_fq(&cancel).map(|s| s.rect).expect("rect");
    let lost = state
        .focus_lost(
            &mut reg,
            &snapshot_without_cancel,
            &cancel,
            None,
            &modal_layer_fq,
            lost_rect,
        )
        .expect("modal cancel-dismiss fallback emits an event");
    assert_eq!(lost.next_fq.as_ref(), Some(&confirm));
}

fn run_scenario_bulk_delete() {
    let (mut reg, layer_fq, _columns, cards) = build_three_column_board();
    let focused = cards[4].clone();
    let mut state = SpatialState::new();
    state.focus(&mut reg, focused.clone()).expect("focus");
    let lost_rect = reg.find_by_fq(&focused).map(|s| s.rect).expect("rect");
    let lost_parent_zone = reg.find_by_fq(&focused).and_then(|s| s.parent_zone.clone());
    let snapshot = snapshot_excluding(&reg, &layer_fq, &focused);
    let event = state
        .focus_lost(
            &mut reg,
            &snapshot,
            &focused,
            lost_parent_zone.as_ref(),
            &layer_fq,
            lost_rect,
        )
        .expect("bulk delete fallback emits an event");
    assert_eq!(event.prev_fq.as_ref(), Some(&focused));
}
