//! Comprehensive soak suite for the snapshot/registry parity invariant
//! that backs the kanban-app `compare_paths` divergence diagnostic.
//!
//! Each test mirrors one of the production navigation scenarios listed
//! in the spatial-nav redesign card and asserts the snapshot path and
//! the registry path produce identical results — the same invariant the
//! debug-build `check_navigate_divergence`, `check_focus_divergence`,
//! and `check_focus_lost_divergence` adapters in `kanban-app/src/commands.rs`
//! pin at the IPC boundary. A `tracing::subscriber::with_default` layer
//! captures any `compare_paths` warns the parity computations might emit
//! and asserts the captured set is empty for every scenario.
//!
//! Scenarios covered (every production nav scenario from the parent
//! card 01KQTC1VNQM9KC90S65P7QX9N1):
//!
//! 1. Arrow nav across all four directions from every column position
//!    on a multi-column board.
//! 2. Click focus on every scope kind (chip, field, button, card,
//!    column header).
//! 3. Drag-drop a card between columns and nav from the moved card.
//! 4. Filter changes that hide the focused row, with focus restoration.
//! 5. Layer push (open inspector), nav inside, layer pop with focus
//!    restoration.
//! 6. Modal dialog push, focus inside, cancel, focus restored.
//! 7. Bulk actions deleting multiple cards including the focused one.
//!
//! When divergence is found in any scenario, add the offending registry/
//! snapshot pair as a regression test to this file rather than chasing
//! the symptom in production logs.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use swissarmyhammer_focus::{
    compare_paths, BeamNavStrategy, Direction, FocusLayer, FocusOverrides, FocusScope,
    FullyQualifiedMoniker, IndexedSnapshot, LayerName, LostFocusContext, NavSnapshot, Pixels, Rect,
    SegmentMoniker, SnapshotScope, SpatialRegistry, SpatialState, WindowLabel,
};
use tracing::{
    field::{Field, Visit},
    span::Attributes,
    Event, Id, Level, Subscriber,
};
use tracing_subscriber::{layer::Context, prelude::*, registry::LookupSpan, Layer};

// ---------------------------------------------------------------------------
// Tracing capture — records every WARN event emitted while a closure runs
// so the suite can assert no `compare_paths` divergence warns fired.
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
struct CapturedEvent {
    fields: HashMap<String, String>,
    message: String,
}

struct FieldVisitor<'a> {
    fields: &'a mut HashMap<String, String>,
    message: &'a mut String,
}

impl<'a> Visit for FieldVisitor<'a> {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            self.fields
                .insert(field.name().to_string(), value.to_string());
        }
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message.push_str(&format!("{value:?}"));
        } else {
            self.fields
                .insert(field.name().to_string(), format!("{value:?}"));
        }
    }
}

struct CapturingLayer {
    events: Arc<Mutex<Vec<CapturedEvent>>>,
}

impl<S> Layer<S> for CapturingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let level = *event.metadata().level();
        if level > Level::WARN {
            return;
        }
        let mut captured = CapturedEvent::default();
        let mut visitor = FieldVisitor {
            fields: &mut captured.fields,
            message: &mut captured.message,
        };
        event.record(&mut visitor);
        self.events.lock().unwrap().push(captured);
    }
    fn on_new_span(&self, _attrs: &Attributes<'_>, _id: &Id, _ctx: Context<'_, S>) {}
}

/// Run `f` under a tracing layer that captures WARN events; return the
/// list of events whose `op` field starts with `spatial_` and whose
/// message matches the `compare_paths` divergence shape.
fn collect_divergence_warns<F: FnOnce()>(f: F) -> Vec<CapturedEvent> {
    let events = Arc::new(Mutex::new(Vec::<CapturedEvent>::new()));
    let layer = CapturingLayer {
        events: events.clone(),
    };
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, f);
    let captured = events.lock().unwrap().clone();
    captured
        .into_iter()
        .filter(|e| {
            e.message
                .contains("spatial-nav snapshot/registry divergence")
        })
        .collect()
}

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
// Parity helpers — duplicate the bodies of kanban-app's three
// `check_*_divergence` adapters using only public APIs, so the soak
// suite drives the real `compare_paths` on each scenario.
// ---------------------------------------------------------------------------

/// Drive the navigate diagnostic the same way `spatial_navigate` does in
/// debug builds. Returns the snapshot result for chaining.
fn drive_navigate_check(
    registry: &SpatialRegistry,
    snapshot: &NavSnapshot,
    focused_fq: &FullyQualifiedMoniker,
    direction: Direction,
) -> Option<FullyQualifiedMoniker> {
    use swissarmyhammer_focus::navigate::NavStrategy;
    use swissarmyhammer_focus::pick_target_via_view;

    let entry = registry.find_by_fq(focused_fq)?;
    let focused_segment = entry.segment.clone();
    let view = IndexedSnapshot::new(snapshot);
    Some(compare_paths(
        "spatial_navigate.divergence",
        || pick_target_via_view(&view, focused_fq, &focused_segment, direction),
        || BeamNavStrategy::new().next(registry, focused_fq, &focused_segment, direction),
    ))
}

/// Drive the focus-ancestor diagnostic the same way `spatial_focus` does
/// in debug builds.
fn drive_focus_check(
    registry: &SpatialRegistry,
    snapshot: &NavSnapshot,
    fq: &FullyQualifiedMoniker,
) -> Vec<FullyQualifiedMoniker> {
    let view = IndexedSnapshot::new(snapshot);
    compare_paths(
        "spatial_focus.divergence",
        || -> Vec<FullyQualifiedMoniker> {
            view.parent_zone_chain(fq).map(|s| s.fq.clone()).collect()
        },
        || -> Vec<FullyQualifiedMoniker> {
            let mut chain = Vec::new();
            let mut next = registry.find_by_fq(fq).and_then(|s| s.parent_zone.clone());
            let mut visited = std::collections::HashSet::new();
            while let Some(zone_fq) = next {
                if !visited.insert(zone_fq.clone()) {
                    break;
                }
                let Some(zone) = registry.find_by_fq(&zone_fq) else {
                    break;
                };
                chain.push(zone.fq.clone());
                next = zone.parent_zone.clone();
            }
            chain
        },
    )
}

/// Drive the focus-lost fallback diagnostic the same way
/// `spatial_focus_lost` does in debug builds.
#[allow(clippy::too_many_arguments)]
fn drive_focus_lost_check(
    registry: &SpatialRegistry,
    spatial_state: &SpatialState,
    snapshot: &NavSnapshot,
    focused_fq: &FullyQualifiedMoniker,
    lost_parent_zone: Option<&FullyQualifiedMoniker>,
    lost_layer_fq: &FullyQualifiedMoniker,
    lost_rect: Rect,
) -> swissarmyhammer_focus::FallbackResolution {
    let indexed = IndexedSnapshot::new(snapshot);
    let ctx = LostFocusContext {
        view: &indexed,
        lost_layer_fq: lost_layer_fq.clone(),
        lost_parent_zone: lost_parent_zone.cloned(),
        lost_rect,
    };
    compare_paths(
        "spatial_focus_lost.divergence",
        || spatial_state.resolve_fallback_with_snapshot(registry, focused_fq, &ctx),
        || spatial_state.resolve_fallback(registry, focused_fq),
    )
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

/// Soak the four-direction arrow nav from every card on a 3×3 board.
/// Both paths must agree at every position; no divergence warn fires.
#[test]
fn soak_arrow_nav_every_card_every_direction() {
    let (mut reg, layer_fq, _columns, cards) = build_three_column_board();
    let snapshot = snapshot_for_layer(&reg, &layer_fq);

    let warns = collect_divergence_warns(|| {
        for card in &cards {
            let mut state = SpatialState::new();
            state.focus(&mut reg, card.clone()).expect("focus card");
            for direction in [
                Direction::Up,
                Direction::Down,
                Direction::Left,
                Direction::Right,
            ] {
                let _ = drive_navigate_check(&reg, &snapshot, card, direction);
            }
        }
    });

    assert!(
        warns.is_empty(),
        "soak_arrow_nav_every_card_every_direction produced divergence warns: {warns:?}",
    );
}

// ---------------------------------------------------------------------------
// Scenario 2: click focus on every scope kind.
// ---------------------------------------------------------------------------

/// Soak click-focus on each scope kind: chip, field, button, card,
/// column header. Each click triggers a `record_focus` ancestor walk
/// (the snapshot/registry parity that `check_focus_divergence` pins).
#[test]
fn soak_click_focus_every_scope_kind() {
    let mut reg = SpatialRegistry::new();
    let layer_fq = fq("/L");
    reg.push_layer(make_layer("/L", None, None));

    // Column header at the top of a column zone.
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

    // Card and its inner scopes (chip, field, button) under the column.
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

    let warns = collect_divergence_warns(|| {
        for fq_to_click in [&col_header, &card_fq, &chip, &field, &button] {
            let _ = drive_focus_check(&reg, &snapshot, fq_to_click);
        }
    });

    assert!(
        warns.is_empty(),
        "soak_click_focus_every_scope_kind produced divergence warns: {warns:?}",
    );
}

// ---------------------------------------------------------------------------
// Scenario 3: drag-drop a card between columns; nav from the moved card.
// ---------------------------------------------------------------------------

/// Soak a card moving from one column to another. Post-move snapshot
/// matches the post-move registry; nav from the moved card via either
/// path picks the same target.
#[test]
fn soak_drag_drop_card_between_columns() {
    let (mut reg, layer_fq, _columns, _cards) = build_three_column_board();

    // Simulate the drag-drop: relocate /L/col:0/card:0-1 into col:2 by
    // updating its rect and parent_zone. The kernel's stateless rewrite
    // is registry.update_rect + a re-register with new parent_zone.
    let moved_old = fq("/L/col:0/card:0-1");
    let moved_new = fq("/L/col:2/card:moved");
    // Drop the old slot.
    reg.unregister_scope(&moved_old);
    // Register at the new location.
    reg.register_scope(leaf(
        moved_new.as_str(),
        "card:moved",
        "/L",
        Some("/L/col:2"),
        rect(410.0, 140.0, 160.0, 80.0),
    ));

    let snapshot = snapshot_for_layer(&reg, &layer_fq);

    let warns = collect_divergence_warns(|| {
        let mut state = SpatialState::new();
        state
            .focus(&mut reg, moved_new.clone())
            .expect("focus moved card");

        // Nav from the moved card in every direction.
        for direction in [
            Direction::Up,
            Direction::Down,
            Direction::Left,
            Direction::Right,
        ] {
            let _ = drive_navigate_check(&reg, &snapshot, &moved_new, direction);
        }
        // Ancestor walk on the moved card (now under col:2).
        let _ = drive_focus_check(&reg, &snapshot, &moved_new);
    });

    assert!(
        warns.is_empty(),
        "soak_drag_drop_card_between_columns produced divergence warns: {warns:?}",
    );
}

// ---------------------------------------------------------------------------
// Scenario 4: filter changes that hide the focused row; focus restored.
// ---------------------------------------------------------------------------

/// Soak a filter change that unmounts the focused card. The snapshot
/// path's `focus_lost` and the registry path's `handle_unregister` resolve
/// the same fallback target.
#[test]
fn soak_filter_hides_focused_row_focus_restored() {
    let (mut reg, layer_fq, _columns, cards) = build_three_column_board();
    // Focus a middle card.
    let focused = cards[4].clone(); // /L/col:1/card:1-1
    let mut state = SpatialState::new();
    state.focus(&mut reg, focused.clone()).expect("focus");

    // The filter hides this card — React's registry deletes it before
    // dispatching focus_lost; build the snapshot accordingly.
    let snapshot = snapshot_excluding(&reg, &layer_fq, &focused);
    let lost_parent_zone = reg.find_by_fq(&focused).and_then(|s| s.parent_zone.clone());
    let lost_rect = reg.find_by_fq(&focused).map(|s| s.rect).expect("rect");

    let warns = collect_divergence_warns(|| {
        let _ = drive_focus_lost_check(
            &reg,
            &state,
            &snapshot,
            &focused,
            lost_parent_zone.as_ref(),
            &layer_fq,
            lost_rect,
        );
    });

    assert!(
        warns.is_empty(),
        "soak_filter_hides_focused_row_focus_restored produced divergence warns: {warns:?}",
    );
}

// ---------------------------------------------------------------------------
// Scenario 5: layer push (open inspector) → nav inside → layer pop →
//             focus restored.
// ---------------------------------------------------------------------------

/// Soak the inspector lifecycle: a child layer pushes, focus moves
/// inside, then the layer pops, restoring the parent layer's
/// last_focused. Both paths must agree on every step.
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

    // Inspector layer pushed; two scopes inside.
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

    let warns = collect_divergence_warns(|| {
        // Focus inside the inspector and run the focus-ancestor check
        // and the nav check.
        let _ = drive_focus_check(&reg, &inspector_snapshot, &insp_a);
        let _ = drive_navigate_check(&reg, &inspector_snapshot, &insp_a, Direction::Down);

        // Layer pop, snapshot side: the inspector entry unmounts on
        // React, so the snapshot for the inspector layer no longer
        // contains it. The kernel registry still has the inspector
        // scopes and layer (the per-scope unregister IPCs have not
        // arrived yet at the moment focus_lost fires).
        let snapshot_without_a = snapshot_excluding(&reg, &inspector_layer_fq, &insp_a);
        let lost_rect_a = reg.find_by_fq(&insp_a).map(|s| s.rect).expect("rect");

        let _ = drive_focus_lost_check(
            &reg,
            &state,
            &snapshot_without_a,
            &insp_a,
            None,
            &inspector_layer_fq,
            lost_rect_a,
        );
    });

    assert!(
        warns.is_empty(),
        "soak_inspector_push_nav_pop_focus_restored produced divergence warns: {warns:?}",
    );
}

// ---------------------------------------------------------------------------
// Scenario 6: modal dialog push → focus inside → cancel → focus restored.
// ---------------------------------------------------------------------------

/// Soak a modal dialog lifecycle. Same shape as the inspector scenario
/// but with a modal layer that gets dismissed (pop) without committing
/// any state. Focus restoration must match between paths.
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

    let warns = collect_divergence_warns(|| {
        // Nav between confirm and cancel inside the modal.
        let _ = drive_navigate_check(&reg, &modal_snapshot, &confirm, Direction::Right);
        let _ = drive_navigate_check(&reg, &modal_snapshot, &cancel, Direction::Left);
        let _ = drive_focus_check(&reg, &modal_snapshot, &confirm);

        // Cancel dismisses the modal. React-side: the cancel button
        // unmounts and focus_lost fires immediately. Kernel-side: the
        // unregister IPCs have not arrived yet, so the modal layer
        // and its scopes are still in the registry. Snapshot omits
        // the cancel scope (and only the cancel scope) — that is what
        // React's registry holds at dispatch time.
        let snapshot_without_cancel = snapshot_excluding(&reg, &modal_layer_fq, &cancel);
        let lost_rect_cancel = reg.find_by_fq(&cancel).map(|s| s.rect).expect("rect");

        let _ = drive_focus_lost_check(
            &reg,
            &state,
            &snapshot_without_cancel,
            &cancel,
            None,
            &modal_layer_fq,
            lost_rect_cancel,
        );
    });

    assert!(
        warns.is_empty(),
        "soak_modal_push_focus_cancel_focus_restored produced divergence warns: {warns:?}",
    );
}

// ---------------------------------------------------------------------------
// Scenario 7: bulk actions delete multiple cards including the focused.
// ---------------------------------------------------------------------------

/// Soak a bulk delete that takes the focused card down with several
/// peers. The focus-lost cascade resolves the same fallback under both
/// paths.
///
/// The React-side `spatial_focus_lost` IPC fires only on the deletion
/// of the focused FQM (`SpatialFocusActions.focus` listens to
/// `LayerScopeRegistry.onDeleted` and ignores deletions of unfocused
/// scopes). Sibling cards deleted in the same React commit run their
/// own `useEffect` cleanups separately; their `spatial_unregister_scope`
/// IPCs land asynchronously. So the snapshot built inside the focused
/// scope's deletion listener excludes only the focused FQM — the other
/// bulk-deleted siblings are still in the React layer registry at the
/// moment the listener runs.
#[test]
fn soak_bulk_delete_includes_focused_card() {
    let (mut reg, layer_fq, _columns, cards) = build_three_column_board();
    // Focus a card we're about to delete.
    let focused = cards[4].clone(); // /L/col:1/card:1-1
    let mut state = SpatialState::new();
    state.focus(&mut reg, focused.clone()).expect("focus");

    let lost_rect = reg.find_by_fq(&focused).map(|s| s.rect).expect("rect");
    let lost_parent_zone = reg.find_by_fq(&focused).and_then(|s| s.parent_zone.clone());

    // Snapshot excludes only the focused FQM — the React listener that
    // dispatches `spatial_focus_lost` runs from the deletion of the
    // focused scope alone. Sibling deletions in the same commit produce
    // separate `spatial_unregister_scope` IPCs and do not contribute to
    // this snapshot.
    let snapshot = snapshot_excluding(&reg, &layer_fq, &focused);

    let warns = collect_divergence_warns(|| {
        let _ = drive_focus_lost_check(
            &reg,
            &state,
            &snapshot,
            &focused,
            lost_parent_zone.as_ref(),
            &layer_fq,
            lost_rect,
        );
    });

    assert!(
        warns.is_empty(),
        "soak_bulk_delete_includes_focused_card produced divergence warns: {warns:?}",
    );
}

// ---------------------------------------------------------------------------
// Cross-cutting: every scenario in sequence on a shared subscriber.
// Mirrors a continuous "user works the board for a while" flow in dev
// builds — a single divergence in any phase should fail the suite.
// ---------------------------------------------------------------------------

/// Run every scenario back-to-back under a single tracing subscriber and
/// assert the captured divergence-warn list is empty across the union.
#[test]
fn soak_all_scenarios_in_sequence() {
    let warns = collect_divergence_warns(|| {
        run_scenario_arrow_nav();
        run_scenario_click_focus();
        run_scenario_drag_drop();
        run_scenario_filter_hide();
        run_scenario_inspector();
        run_scenario_modal();
        run_scenario_bulk_delete();
    });

    assert!(
        warns.is_empty(),
        "soak_all_scenarios_in_sequence produced divergence warns: {warns:?}",
    );
}

// Compact, side-effect-only fixtures the cross-cutting test runs in
// sequence. Each one exercises the same shape as the matching dedicated
// scenario test but without its own subscriber so the cross-cutting
// test can assert across the union.

fn run_scenario_arrow_nav() {
    let (mut reg, layer_fq, _columns, cards) = build_three_column_board();
    let snapshot = snapshot_for_layer(&reg, &layer_fq);
    for card in &cards {
        let mut state = SpatialState::new();
        state.focus(&mut reg, card.clone()).expect("focus card");
        for direction in [
            Direction::Up,
            Direction::Down,
            Direction::Left,
            Direction::Right,
        ] {
            let _ = drive_navigate_check(&reg, &snapshot, card, direction);
        }
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
    for f in [&card, &chip] {
        let _ = drive_focus_check(&reg, &snapshot, f);
    }
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
    state
        .focus(&mut reg, moved_new.clone())
        .expect("focus moved card");
    for direction in [
        Direction::Up,
        Direction::Down,
        Direction::Left,
        Direction::Right,
    ] {
        let _ = drive_navigate_check(&reg, &snapshot, &moved_new, direction);
    }
    let _ = drive_focus_check(&reg, &snapshot, &moved_new);
}

fn run_scenario_filter_hide() {
    let (mut reg, layer_fq, _columns, cards) = build_three_column_board();
    let focused = cards[4].clone();
    let mut state = SpatialState::new();
    state.focus(&mut reg, focused.clone()).expect("focus");
    let snapshot = snapshot_excluding(&reg, &layer_fq, &focused);
    let lost_parent_zone = reg.find_by_fq(&focused).and_then(|s| s.parent_zone.clone());
    let lost_rect = reg.find_by_fq(&focused).map(|s| s.rect).expect("rect");
    let _ = drive_focus_lost_check(
        &reg,
        &state,
        &snapshot,
        &focused,
        lost_parent_zone.as_ref(),
        &layer_fq,
        lost_rect,
    );
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
    let snapshot = snapshot_for_layer(&reg, &inspector_layer_fq);
    let _ = drive_focus_check(&reg, &snapshot, &insp_a);
    let _ = drive_navigate_check(&reg, &snapshot, &insp_a, Direction::Down);

    let snapshot_without_a = snapshot_excluding(&reg, &inspector_layer_fq, &insp_a);
    let lost_rect_a = reg.find_by_fq(&insp_a).map(|s| s.rect).expect("rect");
    let _ = drive_focus_lost_check(
        &reg,
        &state,
        &snapshot_without_a,
        &insp_a,
        None,
        &inspector_layer_fq,
        lost_rect_a,
    );
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
    let cancel = fq("/L/modal/btn:cancel");
    reg.register_scope(leaf(
        cancel.as_str(),
        "btn:cancel",
        "/L/modal",
        None,
        rect(140.0, 100.0, 80.0, 30.0),
    ));
    let modal_snapshot = snapshot_for_layer(&reg, &modal_layer_fq);
    let _ = drive_focus_check(&reg, &modal_snapshot, &cancel);

    let snapshot_without_cancel = snapshot_excluding(&reg, &modal_layer_fq, &cancel);
    let lost_rect = reg.find_by_fq(&cancel).map(|s| s.rect).expect("rect");
    let _ = drive_focus_lost_check(
        &reg,
        &state,
        &snapshot_without_cancel,
        &cancel,
        None,
        &modal_layer_fq,
        lost_rect,
    );
}

fn run_scenario_bulk_delete() {
    let (mut reg, layer_fq, _columns, cards) = build_three_column_board();
    let focused = cards[4].clone();
    let mut state = SpatialState::new();
    state.focus(&mut reg, focused.clone()).expect("focus");
    let lost_rect = reg.find_by_fq(&focused).map(|s| s.rect).expect("rect");
    let lost_parent_zone = reg.find_by_fq(&focused).and_then(|s| s.parent_zone.clone());
    // Snapshot excludes only the focused FQM — sibling deletions from
    // the same commit fire separately and do not contribute to the
    // listener's snapshot. See the dedicated test for the rationale.
    let snapshot = snapshot_excluding(&reg, &layer_fq, &focused);
    let _ = drive_focus_lost_check(
        &reg,
        &state,
        &snapshot,
        &focused,
        lost_parent_zone.as_ref(),
        &layer_fq,
        lost_rect,
    );
}
