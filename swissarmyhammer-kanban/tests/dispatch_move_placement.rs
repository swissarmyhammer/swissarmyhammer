//! Integration tests for move task placement via parse_input → execute_operation.
//!
//! These tests exercise the exact same code path as the MCP kanban tool:
//! JSON input → parse_input → execute_operation → MoveTask with before_id/after_id.

use serde_json::json;
use swissarmyhammer_kanban::{
    board::InitBoard, dispatch::execute_operation, parse::parse_input, task::AddTask, Execute,
    KanbanContext,
};
use tempfile::TempDir;

/// Set up a board with three tasks in "todo", returning (temp, ctx, [id_a, id_b, id_c]).
async fn setup_board_with_tasks() -> (TempDir, KanbanContext, String, String, String) {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);

    InitBoard::new("Test")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    let a = AddTask::new("Task A")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    let b = AddTask::new("Task B")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    let c = AddTask::new("Task C")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    let id_a = a["id"].as_str().unwrap().to_string();
    let id_b = b["id"].as_str().unwrap().to_string();
    let id_c = c["id"].as_str().unwrap().to_string();

    (temp, ctx, id_a, id_b, id_c)
}

/// Read the ordinal for a task.
async fn ordinal(ctx: &KanbanContext, id: &str) -> String {
    let ectx = ctx.entity_context().await.unwrap();
    let entity = ectx.read("task", id).await.unwrap();
    entity
        .get_str("position_ordinal")
        .unwrap_or("80")
        .to_string()
}

/// Execute a move via the parse_input → execute_operation path (same as MCP).
async fn dispatch_move(ctx: &KanbanContext, input: serde_json::Value) -> serde_json::Value {
    let ops = parse_input(input).expect("parse should succeed");
    assert_eq!(ops.len(), 1);
    execute_operation(ctx, &ops[0])
        .await
        .expect("execute should succeed")
}

#[tokio::test]
async fn move_before_first_task_via_dispatch() {
    let (_temp, ctx, id_a, _id_b, id_c) = setup_board_with_tasks().await;

    // Verify initial order: A < B < C
    let ord_a_before = ordinal(&ctx, &id_a).await;
    let ord_c_before = ordinal(&ctx, &id_c).await;
    assert!(ord_a_before < ord_c_before, "A should start before C");

    // Move C before A via JSON dispatch (MCP path)
    dispatch_move(
        &ctx,
        json!({
            "op": "move task",
            "id": id_c,
            "column": "todo",
            "before_id": id_a
        }),
    )
    .await;

    // C should now be before A
    let ord_c_after = ordinal(&ctx, &id_c).await;
    let ord_a_after = ordinal(&ctx, &id_a).await;
    assert!(
        ord_c_after < ord_a_after,
        "C ({}) should be before A ({}) after move",
        ord_c_after,
        ord_a_after
    );
}

#[tokio::test]
async fn move_after_last_task_via_dispatch() {
    let (_temp, ctx, id_a, _id_b, id_c) = setup_board_with_tasks().await;

    // Move A after C
    dispatch_move(
        &ctx,
        json!({
            "op": "move task",
            "id": id_a,
            "column": "todo",
            "after_id": id_c
        }),
    )
    .await;

    let ord_a = ordinal(&ctx, &id_a).await;
    let ord_c = ordinal(&ctx, &id_c).await;
    assert!(
        ord_a > ord_c,
        "A ({}) should be after C ({}) after move",
        ord_a,
        ord_c
    );
}

#[tokio::test]
async fn move_between_tasks_via_dispatch() {
    let (_temp, ctx, id_a, id_b, id_c) = setup_board_with_tasks().await;

    // Move C between A and B (before B)
    dispatch_move(
        &ctx,
        json!({
            "op": "move task",
            "id": id_c,
            "column": "todo",
            "before_id": id_b
        }),
    )
    .await;

    let ord_a = ordinal(&ctx, &id_a).await;
    let ord_c = ordinal(&ctx, &id_c).await;
    let ord_b = ordinal(&ctx, &id_b).await;
    assert!(ord_a < ord_c, "A ({}) < C ({})", ord_a, ord_c);
    assert!(ord_c < ord_b, "C ({}) < B ({})", ord_c, ord_b);
}

#[tokio::test]
async fn move_without_placement_appends() {
    let (_temp, ctx, id_a, _id_b, _id_c) = setup_board_with_tasks().await;

    // Move A to "doing" without placement — should append
    dispatch_move(
        &ctx,
        json!({
            "op": "move task",
            "id": id_a,
            "column": "doing"
        }),
    )
    .await;

    let ectx = ctx.entity_context().await.unwrap();
    let entity = ectx.read("task", &id_a).await.unwrap();
    assert_eq!(entity.get_str("position_column"), Some("doing"));
}

#[tokio::test]
async fn before_nonexistent_task_appends() {
    let (_temp, ctx, id_a, _id_b, _id_c) = setup_board_with_tasks().await;

    let _ord_before = ordinal(&ctx, &id_a).await;

    // before_id references a task that doesn't exist — should fall through to append
    dispatch_move(
        &ctx,
        json!({
            "op": "move task",
            "id": id_a,
            "column": "todo",
            "before_id": "nonexistent-task-id"
        }),
    )
    .await;

    // Should still be in todo (didn't error out)
    let ectx = ctx.entity_context().await.unwrap();
    let entity = ectx.read("task", &id_a).await.unwrap();
    assert_eq!(entity.get_str("position_column"), Some("todo"));
}

/// Reproduce the exact production board state: 8 cards in "todo" with their
/// real ordinals. Then move "Fix same-board drag" (ordinal "8780") to be
/// first — before "Switch AppConfig" (ordinal "80").
///
/// This is the exact scenario that failed via the MCP tool in production.
#[tokio::test]
async fn move_to_first_in_production_board_state() {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);

    InitBoard::new("Test")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    // Create all 8 cards matching production
    struct Card {
        title: &'static str,
        ordinal: &'static str,
    }
    let cards = [
        Card {
            title: "Switch AppConfig persistence from JSON to YAML",
            ordinal: "80",
        },
        Card {
            title: "Unify per-window state into windows map",
            ordinal: "8180",
        },
        Card {
            title: "Update Tauri commands to use per-window state",
            ordinal: "8280",
        },
        Card {
            title: "Frontend: strip localStorage/refs",
            ordinal: "8380",
        },
        Card {
            title: "Wire view switching through command dispatch",
            ordinal: "8480",
        },
        Card {
            title: "Remove grey opacity on blocked cards",
            ordinal: "8580",
        },
        Card {
            title: "Show dependency pills in card header after tags",
            ordinal: "8680",
        },
        Card {
            title: "Fix same-board drag",
            ordinal: "8780",
        },
    ];

    let ectx = ctx.entity_context().await.unwrap();
    let mut ids: Vec<String> = Vec::new();

    for card in &cards {
        let result = AddTask::new(card.title)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id = result["id"].as_str().unwrap().to_string();

        // Force the production ordinal
        let mut entity = ectx.read("task", &id).await.unwrap();
        entity.set("position_ordinal", json!(card.ordinal));
        ectx.write(&entity).await.unwrap();

        ids.push(id);
    }

    // ids[0] = "Switch AppConfig" (ordinal "80")  — currently first
    // ids[7] = "Fix same-board drag" (ordinal "8780") — currently last

    // Verify initial sort order: "80" < "8180" < ... < "8780"
    let mut prev_ord = String::new();
    for (i, id) in ids.iter().enumerate() {
        let ord = ordinal(&ctx, id).await;
        if i > 0 {
            assert!(
                prev_ord < ord,
                "Card {} ({}) should sort after card {} ({})",
                i,
                ord,
                i - 1,
                prev_ord
            );
        }
        prev_ord = ord;
    }

    // THE FAILING SCENARIO: move "Fix same-board drag" before "Switch AppConfig"
    // i.e., move the last card to be first
    dispatch_move(
        &ctx,
        json!({
            "op": "move task",
            "id": ids[7],
            "column": "todo",
            "before_id": ids[0]
        }),
    )
    .await;

    // "Fix same-board drag" should now sort before "Switch AppConfig"
    let ord_drag = ordinal(&ctx, &ids[7]).await;
    let ord_yaml = ordinal(&ctx, &ids[0]).await;
    assert!(
        ord_drag < ord_yaml,
        "Fix-drag ({}) should be before Switch-AppConfig ({}) after move",
        ord_drag,
        ord_yaml
    );

    // Verify full order: drag, yaml, unify, commands, frontend, wire, grey, pills
    let mut all_ords: Vec<(usize, String)> = Vec::new();
    for (i, id) in ids.iter().enumerate() {
        all_ords.push((i, ordinal(&ctx, id).await));
    }
    all_ords.sort_by(|a, b| a.1.cmp(&b.1));

    let expected_order = [7, 0, 1, 2, 3, 4, 5, 6]; // drag first, then original order
    let actual_order: Vec<usize> = all_ords.iter().map(|(i, _)| *i).collect();
    assert_eq!(
        actual_order,
        expected_order,
        "Full board order wrong. Got indices {:?}, ordinals: {:?}",
        actual_order,
        all_ords
            .iter()
            .map(|(i, o)| (cards[*i].title, o.as_str()))
            .collect::<Vec<_>>()
    );
}

/// Move the 3rd card to the 2nd position using before_id.
///
/// This exercises the exact drop-zone scenario from the frontend:
/// [A, B, C] → user drops C on zone "before-B" → C lands between A and B.
///
/// Cross-reference: the notification flow after this move emits
/// `entity-field-changed` with updated `position_ordinal`, tested in
/// `kanban-app/src/watcher.rs`
/// (`bridge_end_to_end_second_write_emits_field_changed_payload`).
#[tokio::test]
async fn move_third_to_second_position_via_dispatch() {
    let (_temp, ctx, id_a, id_b, id_c) = setup_board_with_tasks().await;

    // Move C before B (3rd card to 2nd position)
    dispatch_move(
        &ctx,
        json!({
            "op": "move task",
            "id": id_c,
            "column": "todo",
            "before_id": id_b
        }),
    )
    .await;

    let ord_a = ordinal(&ctx, &id_a).await;
    let ord_c = ordinal(&ctx, &id_c).await;
    let ord_b = ordinal(&ctx, &id_b).await;
    assert!(ord_a < ord_c, "A ({}) < C ({})", ord_a, ord_c);
    assert!(ord_c < ord_b, "C ({}) < B ({})", ord_c, ord_b);
}

/// Same production state, but move a card between two others:
/// Move "Wire view switching" (ordinal "8480") before "Unify per-window" (ordinal "8180").
#[tokio::test]
async fn move_between_in_production_board_state() {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);

    InitBoard::new("Test")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    struct Card {
        title: &'static str,
        ordinal: &'static str,
    }
    let cards = [
        Card {
            title: "Switch AppConfig",
            ordinal: "80",
        },
        Card {
            title: "Unify per-window",
            ordinal: "8180",
        },
        Card {
            title: "Update Tauri",
            ordinal: "8280",
        },
        Card {
            title: "Frontend strip",
            ordinal: "8380",
        },
        Card {
            title: "Wire view",
            ordinal: "8480",
        },
        Card {
            title: "Remove grey",
            ordinal: "8580",
        },
        Card {
            title: "Show pills",
            ordinal: "8680",
        },
        Card {
            title: "Fix drag",
            ordinal: "8780",
        },
    ];

    let ectx = ctx.entity_context().await.unwrap();
    let mut ids: Vec<String> = Vec::new();

    for card in &cards {
        let result = AddTask::new(card.title)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id = result["id"].as_str().unwrap().to_string();
        let mut entity = ectx.read("task", &id).await.unwrap();
        entity.set("position_ordinal", json!(card.ordinal));
        ectx.write(&entity).await.unwrap();
        ids.push(id);
    }

    // Move "Wire view" (index 4, ordinal "8480") before "Unify per-window" (index 1, ordinal "8180")
    // It should land between "Switch AppConfig" ("80") and "Unify per-window" ("8180")
    dispatch_move(
        &ctx,
        json!({
            "op": "move task",
            "id": ids[4],
            "column": "todo",
            "before_id": ids[1]
        }),
    )
    .await;

    let ord_appconfig = ordinal(&ctx, &ids[0]).await;
    let ord_wire = ordinal(&ctx, &ids[4]).await;
    let ord_unify = ordinal(&ctx, &ids[1]).await;

    assert!(
        ord_appconfig < ord_wire,
        "AppConfig ({}) < Wire ({})",
        ord_appconfig,
        ord_wire
    );
    assert!(
        ord_wire < ord_unify,
        "Wire ({}) < Unify ({})",
        ord_wire,
        ord_unify
    );
}
