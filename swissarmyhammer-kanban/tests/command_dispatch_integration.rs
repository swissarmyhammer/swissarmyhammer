//! Integration tests for the unified command dispatch system.
//!
//! These tests exercise the full dispatch cycle (registry lookup, availability
//! check, context construction, execution) without Tauri or React. A `TestEngine`
//! struct manages a temp board, shared UIState, and the command map.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer_commands::{
    builtin_yaml_sources, Command, CommandContext, CommandError, CommandsRegistry, UIState,
};
use swissarmyhammer_kanban::commands::register_commands;
use swissarmyhammer_kanban::{
    board::InitBoard, KanbanContext, KanbanOperationProcessor, OperationProcessor,
};
use tempfile::TempDir;

// ===========================================================================
// TestEngine
// ===========================================================================

/// Lightweight harness that wires up a temp board, command registry, command
/// implementations, and shared UIState for integration tests.
struct TestEngine {
    _temp: TempDir,
    kanban: Arc<KanbanContext>,
    commands: HashMap<String, Arc<dyn Command>>,
    _registry: CommandsRegistry,
    ui_state: Arc<UIState>,
}

impl TestEngine {
    /// Create a new test engine with an initialized board (todo, doing, done columns).
    async fn new() -> Self {
        let temp = TempDir::new().expect("failed to create temp dir");
        let kanban_dir = temp.path().join(".kanban");
        let kanban = KanbanContext::new(&kanban_dir);

        // Initialize the board (creates directories, board entity, default columns)
        let processor = KanbanOperationProcessor::new();
        processor
            .process(&InitBoard::new("Test Board"), &kanban)
            .await
            .expect("board init failed");

        let kanban = Arc::new(kanban);
        let registry = CommandsRegistry::from_yaml_sources(&builtin_yaml_sources());
        let commands = register_commands();
        let ui_state = Arc::new(UIState::new());
        Self {
            _temp: temp,
            kanban,
            commands,
            _registry: registry,
            ui_state,
        }
    }

    /// Dispatch a command by ID through the full availability + execute cycle.
    ///
    /// Builds a `CommandContext` with the given scope chain, target, and args,
    /// attaches the shared UIState and KanbanContext extension, checks
    /// availability, then executes.
    async fn dispatch(
        &self,
        cmd_id: &str,
        scope: &[&str],
        target: Option<&str>,
        args: HashMap<String, Value>,
    ) -> swissarmyhammer_commands::Result<Value> {
        let cmd = self
            .commands
            .get(cmd_id)
            .ok_or_else(|| CommandError::ExecutionFailed(format!("unknown command: {}", cmd_id)))?;

        let mut ctx = CommandContext::new(
            cmd_id,
            scope.iter().map(|s| s.to_string()).collect(),
            target.map(|s| s.to_string()),
            args,
        );
        ctx.ui_state = Some(Arc::clone(&self.ui_state));
        ctx.set_extension(Arc::clone(&self.kanban));
        // Set EntityContext extension for entity-layer commands (undo/redo)
        if let Ok(ectx_arc) = self.kanban.entity_context().await {
            ctx.set_extension(ectx_arc);
        }

        if !cmd.available(&ctx) {
            return Err(CommandError::ExecutionFailed(format!(
                "command '{}' not available in this context",
                cmd_id
            )));
        }

        let result = cmd.execute(&ctx).await?;

        // Undo stack push is handled automatically inside EntityContext::write()/delete()
        Ok(result)
    }

    /// Convenience: dispatch with no args.
    async fn dispatch_simple(
        &self,
        cmd_id: &str,
        scope: &[&str],
        target: Option<&str>,
    ) -> swissarmyhammer_commands::Result<Value> {
        self.dispatch(cmd_id, scope, target, HashMap::new()).await
    }
}

// ===========================================================================
// Command dispatch tests
// ===========================================================================

#[tokio::test]
async fn task_add_creates_task() {
    let engine = TestEngine::new().await;

    let result = engine
        .dispatch_simple("task.add", &["column:todo"], None)
        .await
        .expect("task.add should succeed");

    // Should return a JSON object with an operation_id and task id
    assert!(
        result.get("operation_id").is_some(),
        "result should contain operation_id"
    );
    assert!(result.get("id").is_some(), "result should contain task id");

    // Verify the task actually exists on disk
    let task_id = result["id"].as_str().unwrap();
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .expect("task should exist after add");
    assert!(task.get_str("title").is_some());
}

#[tokio::test]
async fn task_move_to_column() {
    let engine = TestEngine::new().await;

    // Add a task in todo
    let add_result = engine
        .dispatch_simple("task.add", &["column:todo"], None)
        .await
        .unwrap();
    let task_id = add_result["id"].as_str().unwrap();

    // Move to doing via args
    let mut args = HashMap::new();
    args.insert("column".to_string(), json!("doing"));

    let move_result = engine
        .dispatch("task.move", &[&format!("task:{}", task_id)], None, args)
        .await
        .expect("task.move should succeed");

    assert!(move_result.get("operation_id").is_some());

    // Verify the task is now in the "doing" column
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .unwrap();
    assert_eq!(task.get_str("position_column"), Some("doing"));
}

#[tokio::test]
async fn task_untag_removes_tag() {
    let engine = TestEngine::new().await;

    // Add a task with a tag via the lower-level API
    let processor = KanbanOperationProcessor::new();

    let add_result = processor
        .process(
            &swissarmyhammer_kanban::task::AddTask::new("Tagged task"),
            &engine.kanban,
        )
        .await
        .unwrap();
    let task_id = add_result["id"].as_str().unwrap();

    // Create a tag
    processor
        .process(
            &swissarmyhammer_kanban::tag::AddTag::new("bug"),
            &engine.kanban,
        )
        .await
        .unwrap();

    // Tag the task
    processor
        .process(
            &swissarmyhammer_kanban::task::TagTask::new(task_id, "bug"),
            &engine.kanban,
        )
        .await
        .unwrap();

    // Verify the tag is on the task
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .unwrap();
    let tags = task
        .get("tags")
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    assert!(
        tags.iter().any(|t| t.as_str() == Some("bug")),
        "task should have 'bug' tag before untag"
    );

    // Now dispatch task.untag via the command system
    let result = engine
        .dispatch_simple(
            "task.untag",
            &["tag:bug", &format!("task:{}", task_id)],
            None,
        )
        .await
        .expect("task.untag should succeed");

    assert!(result.get("operation_id").is_some());

    // Verify the tag was removed
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .unwrap();
    let tags = task
        .get("tags")
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    assert!(
        !tags.iter().any(|t| t.as_str() == Some("bug")),
        "task should no longer have 'bug' tag after untag"
    );
}

#[tokio::test]
async fn entity_update_field() {
    let engine = TestEngine::new().await;

    // Add a task
    let add_result = engine
        .dispatch_simple("task.add", &["column:todo"], None)
        .await
        .unwrap();
    let task_id = add_result["id"].as_str().unwrap();

    // Update the title via entity.update_field
    let mut args = HashMap::new();
    args.insert("entity_type".to_string(), json!("task"));
    args.insert("id".to_string(), json!(task_id));
    args.insert("field_name".to_string(), json!("title"));
    args.insert("value".to_string(), json!("New Title"));

    let result = engine
        .dispatch("entity.update_field", &[], None, args)
        .await
        .expect("entity.update_field should succeed");

    assert!(result.get("operation_id").is_some());

    // Verify the title was updated
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .unwrap();
    assert_eq!(task.get_str("title"), Some("New Title"));
}

// ===========================================================================
// Availability tests
// ===========================================================================

#[tokio::test]
async fn task_add_unavailable_without_column() {
    let engine = TestEngine::new().await;

    let result = engine.dispatch_simple("task.add", &[], None).await;

    assert!(
        result.is_err(),
        "task.add should fail without column in scope"
    );
}

#[tokio::test]
async fn task_untag_unavailable_without_tag() {
    let engine = TestEngine::new().await;

    let result = engine
        .dispatch_simple("task.untag", &["task:01FAKE"], None)
        .await;

    assert!(
        result.is_err(),
        "task.untag should fail without tag in scope"
    );
}

#[tokio::test]
async fn quit_always_available() {
    let engine = TestEngine::new().await;

    let result = engine
        .dispatch_simple("app.quit", &[], None)
        .await
        .expect("app.quit should succeed with empty scope");

    assert_eq!(result["quit"], true);
}

// ===========================================================================
// UI state tests
// ===========================================================================

#[tokio::test]
async fn inspect_updates_ui_state() {
    let engine = TestEngine::new().await;

    engine
        .dispatch_simple("ui.inspect", &[], Some("task:01XYZ"))
        .await
        .expect("ui.inspect should succeed");

    // dispatch_simple doesn't set window_label, so falls back to "main"
    assert_eq!(engine.ui_state.inspector_stack("main"), vec!["task:01XYZ"]);
}

#[tokio::test]
async fn inspect_secondary_pushes() {
    let engine = TestEngine::new().await;

    // First inspect a task (primary type — replaces stack)
    engine
        .dispatch_simple("ui.inspect", &[], Some("task:01XYZ"))
        .await
        .unwrap();

    // Then inspect a tag (secondary type — pushes onto stack)
    engine
        .dispatch_simple("ui.inspect", &[], Some("tag:01ABC"))
        .await
        .unwrap();

    assert_eq!(
        engine.ui_state.inspector_stack("main"),
        vec!["task:01XYZ", "tag:01ABC"]
    );
}

#[tokio::test]
async fn inspector_close_pops() {
    let engine = TestEngine::new().await;

    // Push two entries
    engine
        .dispatch_simple("ui.inspect", &[], Some("task:01XYZ"))
        .await
        .unwrap();
    engine
        .dispatch_simple("ui.inspect", &[], Some("tag:01ABC"))
        .await
        .unwrap();

    // Close top
    engine
        .dispatch_simple("ui.inspector.close", &[], None)
        .await
        .expect("ui.inspector.close should succeed");

    assert_eq!(engine.ui_state.inspector_stack("main"), vec!["task:01XYZ"]);
}

#[tokio::test]
async fn inspector_close_all() {
    let engine = TestEngine::new().await;

    // Push entries
    engine
        .dispatch_simple("ui.inspect", &[], Some("task:01XYZ"))
        .await
        .unwrap();
    engine
        .dispatch_simple("ui.inspect", &[], Some("tag:01ABC"))
        .await
        .unwrap();

    // Close all
    engine
        .dispatch_simple("ui.inspector.close_all", &[], None)
        .await
        .expect("ui.inspector.close_all should succeed");

    assert!(engine.ui_state.inspector_stack("main").is_empty());
}

#[tokio::test]
async fn keymap_mode_change() {
    let engine = TestEngine::new().await;

    // Default is "cua"
    assert_eq!(engine.ui_state.keymap_mode(), "cua");

    engine
        .dispatch_simple("settings.keymap.vim", &[], None)
        .await
        .expect("settings.keymap.vim should succeed");

    assert_eq!(engine.ui_state.keymap_mode(), "vim");
}

// ===========================================================================
// Undo / Redo tests
// ===========================================================================

#[tokio::test]
async fn undo_reverts_task_add() {
    let engine = TestEngine::new().await;

    // Add a task
    let add_result = engine
        .dispatch_simple("task.add", &["column:todo"], None)
        .await
        .expect("task.add should succeed");
    let task_id = add_result["id"].as_str().unwrap();
    let operation_id = add_result["operation_id"].as_str().unwrap();

    // Verify the task exists
    assert!(
        engine
            .kanban
            .read_entity_generic("task", task_id)
            .await
            .is_ok(),
        "task should exist after add"
    );

    // Undo the add (stack-based — no explicit id needed)
    let undo_result = engine
        .dispatch_simple("app.undo", &[], None)
        .await
        .expect("app.undo should succeed");

    assert_eq!(undo_result["undone"].as_str(), Some(operation_id));

    // Task should be gone (trashed)
    assert!(
        engine
            .kanban
            .read_entity_generic("task", task_id)
            .await
            .is_err(),
        "task should not exist after undo"
    );

    // Redo the add (stack-based — no explicit id needed)
    let redo_result = engine
        .dispatch_simple("app.redo", &[], None)
        .await
        .expect("app.redo should succeed");

    assert!(redo_result.get("redone").is_some());

    // Task should be back
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .expect("task should exist after redo");
    assert!(task.get_str("title").is_some());
}

#[tokio::test]
async fn undo_reverts_field_update() {
    let engine = TestEngine::new().await;

    // Add a task
    let add_result = engine
        .dispatch_simple("task.add", &["column:todo"], None)
        .await
        .unwrap();
    let task_id = add_result["id"].as_str().unwrap();

    // Get the original title
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .unwrap();
    let original_title = task.get_str("title").unwrap_or("").to_string();

    // Update the title
    let mut update_args = HashMap::new();
    update_args.insert("entity_type".to_string(), json!("task"));
    update_args.insert("id".to_string(), json!(task_id));
    update_args.insert("field_name".to_string(), json!("title"));
    update_args.insert("value".to_string(), json!("Changed Title"));
    let update_result = engine
        .dispatch("entity.update_field", &[], None, update_args)
        .await
        .expect("update should succeed");
    // Verify operation_id is present (used by undo stack internally)
    assert!(update_result.get("operation_id").is_some());

    // Verify title changed
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .unwrap();
    assert_eq!(task.get_str("title"), Some("Changed Title"));

    // Undo the update (stack-based — no explicit id needed)
    engine
        .dispatch_simple("app.undo", &[], None)
        .await
        .expect("undo should succeed");

    // Title should be back to original
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .unwrap();
    assert_eq!(
        task.get_str("title"),
        Some(original_title.as_str()),
        "title should revert after undo"
    );
}

#[tokio::test]
async fn undo_redo_task_move() {
    let engine = TestEngine::new().await;

    // 1. Add a task in "todo"
    let add_result = engine
        .dispatch_simple("task.add", &["column:todo"], None)
        .await
        .expect("task.add should succeed");
    let task_id = add_result["id"].as_str().unwrap();

    // Verify task starts in "todo"
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .unwrap();
    assert_eq!(
        task.get_str("position_column"),
        Some("todo"),
        "task should start in todo"
    );

    // 2. Move it to "doing" via task.move
    let mut move_args = HashMap::new();
    move_args.insert("column".to_string(), json!("doing"));
    let move_result = engine
        .dispatch(
            "task.move",
            &[&format!("task:{}", task_id), "column:todo"],
            None,
            move_args,
        )
        .await
        .expect("task.move should succeed");
    let move_op_id = move_result["operation_id"].as_str().unwrap();

    // 3. Verify task is in "doing"
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .unwrap();
    assert_eq!(
        task.get_str("position_column"),
        Some("doing"),
        "task should be in doing after move"
    );

    // 4. Undo — verify task is back in "todo"
    let mut undo_args = HashMap::new();
    undo_args.insert("id".to_string(), json!(move_op_id));
    let undo_result = engine
        .dispatch("app.undo", &[], None, undo_args)
        .await
        .expect("app.undo should succeed");

    assert_eq!(undo_result["undone"].as_str(), Some(move_op_id));
    let undo_op_id = undo_result["operation_id"]
        .as_str()
        .expect("undo should return an operation_id for redo");

    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .unwrap();
    assert_eq!(
        task.get_str("position_column"),
        Some("todo"),
        "task should be back in todo after undo"
    );

    // 5. Redo — verify task is in "doing" again
    let mut redo_args = HashMap::new();
    redo_args.insert("id".to_string(), json!(undo_op_id));
    let redo_result = engine
        .dispatch("app.redo", &[], None, redo_args)
        .await
        .expect("app.redo should succeed");

    assert_eq!(redo_result["redone"].as_str(), Some(undo_op_id));

    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .unwrap();
    assert_eq!(
        task.get_str("position_column"),
        Some("doing"),
        "task should be in doing again after redo"
    );
}

// ===========================================================================
// Full session test
// ===========================================================================

#[tokio::test]
async fn full_session_add_move_update() {
    let engine = TestEngine::new().await;

    // 1. Add a task in todo
    let add_result = engine
        .dispatch_simple("task.add", &["column:todo"], None)
        .await
        .expect("add should succeed");
    let task_id = add_result["id"].as_str().unwrap();

    // Verify task is in todo
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .unwrap();
    assert_eq!(task.get_str("position_column"), Some("todo"));

    // 2. Move task to doing
    let mut move_args = HashMap::new();
    move_args.insert("column".to_string(), json!("doing"));
    engine
        .dispatch(
            "task.move",
            &[&format!("task:{}", task_id)],
            None,
            move_args,
        )
        .await
        .expect("move should succeed");

    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .unwrap();
    assert_eq!(task.get_str("position_column"), Some("doing"));

    // 3. Update the title
    let mut update_args = HashMap::new();
    update_args.insert("entity_type".to_string(), json!("task"));
    update_args.insert("id".to_string(), json!(task_id));
    update_args.insert("field_name".to_string(), json!("title"));
    update_args.insert("value".to_string(), json!("Updated Title"));
    engine
        .dispatch("entity.update_field", &[], None, update_args)
        .await
        .expect("update should succeed");

    // 4. Verify final state
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .unwrap();
    assert_eq!(task.get_str("position_column"), Some("doing"));
    assert_eq!(task.get_str("title"), Some("Updated Title"));
}

// ===========================================================================
// Task reorder integration tests — before_id / after_id placement
// ===========================================================================

/// Helper: add N tasks to "todo" column with distinct ordinals, return IDs in creation order.
async fn add_tasks(engine: &TestEngine, titles: &[&str]) -> Vec<String> {
    let mut ids = Vec::new();
    for (i, title) in titles.iter().enumerate() {
        let mut args = HashMap::new();
        args.insert("title".into(), json!(title));
        let result = engine
            .dispatch("task.add", &["column:todo"], None, args)
            .await
            .expect("task.add should succeed");
        let id = result["id"].as_str().unwrap().to_string();

        // Set distinct ordinals using Ordinal::after chain so tasks have a
        // defined sort order. We pass the string form of valid FractionalIndex
        // ordinals. Build them: first(), after(first), after(after(first)), ...
        {
            use swissarmyhammer_kanban::types::Ordinal;
            let mut ord = Ordinal::first();
            for _ in 0..i {
                ord = Ordinal::after(&ord);
            }
            let mut move_args = HashMap::new();
            move_args.insert("id".into(), json!(&id));
            move_args.insert("column".into(), json!("todo"));
            move_args.insert("ordinal".into(), json!(ord.as_str()));
            engine
                .dispatch("task.move", &[], None, move_args)
                .await
                .expect("task.move to set ordinal should succeed");
        }

        ids.push(id);
    }
    ids
}

/// Helper: read tasks in "todo" column, sorted by ordinal, return IDs in order.
async fn todo_order(engine: &TestEngine) -> Vec<String> {
    let ectx = engine.kanban.entity_context().await.unwrap();
    let all = ectx.list("task").await.unwrap();
    let mut col_tasks: Vec<_> = all
        .into_iter()
        .filter(|t| t.get_str("position_column") == Some("todo"))
        .collect();
    col_tasks.sort_by(|a, b| {
        let oa = a.get_str("position_ordinal").unwrap_or("a0");
        let ob = b.get_str("position_ordinal").unwrap_or("a0");
        oa.cmp(ob)
    });
    col_tasks.iter().map(|t| t.id.to_string()).collect()
}

/// Helper: move task with before_id (place before reference task).
async fn move_before(engine: &TestEngine, task_id: &str, before_id: &str) {
    let mut args = HashMap::new();
    args.insert("id".into(), json!(task_id));
    args.insert("column".into(), json!("todo"));
    args.insert("before_id".into(), json!(before_id));
    engine
        .dispatch("task.move", &[], None, args)
        .await
        .expect("task.move before should succeed");
}

/// Helper: move task with after_id (place after reference task).
async fn move_after(engine: &TestEngine, task_id: &str, after_id: &str) {
    let mut args = HashMap::new();
    args.insert("id".into(), json!(task_id));
    args.insert("column".into(), json!("todo"));
    args.insert("after_id".into(), json!(after_id));
    engine
        .dispatch("task.move", &[], None, args)
        .await
        .expect("task.move after should succeed");
}

#[tokio::test]
async fn reorder_move_last_to_first() {
    let engine = TestEngine::new().await;
    let ids = add_tasks(&engine, &["A", "B", "C"]).await;

    // Move C before A → order should be [C, A, B]
    move_before(&engine, &ids[2], &ids[0]).await;
    let order = todo_order(&engine).await;
    assert_eq!(
        order,
        vec![ids[2].clone(), ids[0].clone(), ids[1].clone()],
        "C should be first after moving before A"
    );
}

#[tokio::test]
async fn reorder_move_first_to_last() {
    let engine = TestEngine::new().await;
    let ids = add_tasks(&engine, &["A", "B", "C"]).await;

    // Move A after C → order should be [B, C, A]
    move_after(&engine, &ids[0], &ids[2]).await;
    let order = todo_order(&engine).await;
    assert_eq!(
        order,
        vec![ids[1].clone(), ids[2].clone(), ids[0].clone()],
        "A should be last after moving after C"
    );
}

#[tokio::test]
async fn reorder_pairwise_swap() {
    let engine = TestEngine::new().await;
    let ids = add_tasks(&engine, &["A", "B", "C", "D"]).await;

    // Swap A and B: move A after B → [B, A, C, D]
    move_after(&engine, &ids[0], &ids[1]).await;
    let order = todo_order(&engine).await;
    assert_eq!(
        order,
        vec![
            ids[1].clone(),
            ids[0].clone(),
            ids[2].clone(),
            ids[3].clone()
        ]
    );

    // Swap C and D: move C after D → [B, A, D, C]
    move_after(&engine, &ids[2], &ids[3]).await;
    let order = todo_order(&engine).await;
    assert_eq!(
        order,
        vec![
            ids[1].clone(),
            ids[0].clone(),
            ids[3].clone(),
            ids[2].clone()
        ]
    );

    // Swap back: move B after A → [A, B, D, C]
    move_after(&engine, &ids[1], &ids[0]).await;
    let order = todo_order(&engine).await;
    assert_eq!(
        order,
        vec![
            ids[0].clone(),
            ids[1].clone(),
            ids[3].clone(),
            ids[2].clone()
        ]
    );
}

#[tokio::test]
async fn reorder_reverse_list_by_dragging_end_to_beginning() {
    let engine = TestEngine::new().await;
    let ids = add_tasks(&engine, &["A", "B", "C", "D", "E"]).await;

    // Reverse by repeatedly moving last to first:
    // [A,B,C,D,E] → move E before A → [E,A,B,C,D]
    move_before(&engine, &ids[4], &ids[0]).await;
    // → move D before E → [D,E,A,B,C]
    move_before(&engine, &ids[3], &ids[4]).await;
    // → move C before D → [C,D,E,A,B]
    move_before(&engine, &ids[2], &ids[3]).await;
    // → move B before C → [B,C,D,E,A]
    move_before(&engine, &ids[1], &ids[2]).await;

    let order = todo_order(&engine).await;
    assert_eq!(
        order,
        vec![
            ids[1].clone(),
            ids[2].clone(),
            ids[3].clone(),
            ids[4].clone(),
            ids[0].clone()
        ],
        "list should be [B,C,D,E,A] after reversing"
    );
}

#[tokio::test]
async fn reorder_reverse_list_by_dragging_beginning_to_end() {
    let engine = TestEngine::new().await;
    let ids = add_tasks(&engine, &["A", "B", "C", "D", "E"]).await;

    // Reverse by repeatedly moving first to last:
    // [A,B,C,D,E] → move A after E → [B,C,D,E,A]
    move_after(&engine, &ids[0], &ids[4]).await;
    // → move B after A → [C,D,E,A,B]
    move_after(&engine, &ids[1], &ids[0]).await;
    // → move C after B → [D,E,A,B,C]
    move_after(&engine, &ids[2], &ids[1]).await;
    // → move D after C → [E,A,B,C,D]
    move_after(&engine, &ids[3], &ids[2]).await;

    let order = todo_order(&engine).await;
    assert_eq!(
        order,
        vec![
            ids[4].clone(),
            ids[0].clone(),
            ids[1].clone(),
            ids[2].clone(),
            ids[3].clone()
        ],
        "list should be [E,A,B,C,D] after reversing"
    );
}

#[tokio::test]
async fn reorder_move_to_middle() {
    let engine = TestEngine::new().await;
    let ids = add_tasks(&engine, &["A", "B", "C", "D", "E"]).await;

    // Move E before C → [A, B, E, C, D]
    move_before(&engine, &ids[4], &ids[2]).await;
    let order = todo_order(&engine).await;
    assert_eq!(
        order,
        vec![
            ids[0].clone(),
            ids[1].clone(),
            ids[4].clone(),
            ids[2].clone(),
            ids[3].clone()
        ],
    );

    // Move A after E → [B, E, A, C, D]
    move_after(&engine, &ids[0], &ids[4]).await;
    let order = todo_order(&engine).await;
    assert_eq!(
        order,
        vec![
            ids[1].clone(),
            ids[4].clone(),
            ids[0].clone(),
            ids[2].clone(),
            ids[3].clone()
        ],
    );
}

/// The YAML registry must mark task.move as undoable — this is the gate for
/// flush_and_emit_for_handle to run and emit entity-field-changed events.
#[tokio::test]
async fn task_move_is_undoable_in_registry() {
    let registry = CommandsRegistry::from_yaml_sources(&builtin_yaml_sources());
    let cmd_def = registry.get("task.move");
    assert!(
        cmd_def.is_some(),
        "task.move must exist in the YAML registry"
    );
    assert!(
        cmd_def.unwrap().undoable,
        "task.move must be marked undoable so flush_and_emit fires events"
    );
}

/// After task.move, the task's .md file on disk must have the updated position_ordinal.
/// This is the precondition for flush_and_emit to detect the change and fire events.
#[tokio::test]
async fn task_move_writes_new_ordinal_to_disk() {
    let engine = TestEngine::new().await;
    let ids = add_tasks(&engine, &["A", "B", "C", "D"]).await;

    // Read C's ordinal before move
    let task_dir = engine.kanban.root().join("tasks");
    let c_md_path = task_dir.join(format!("{}.md", &ids[2]));
    let before_content = std::fs::read_to_string(&c_md_path).expect("should read task C .md");
    let before_ordinal = before_content
        .lines()
        .find(|l| l.starts_with("position_ordinal:"))
        .expect("should have position_ordinal")
        .to_string();

    // Move C before B
    move_before(&engine, &ids[2], &ids[1]).await;

    // Read C's ordinal after move
    let after_content =
        std::fs::read_to_string(&c_md_path).expect("should read task C .md after move");
    let after_ordinal = after_content
        .lines()
        .find(|l| l.starts_with("position_ordinal:"))
        .expect("should have position_ordinal after move")
        .to_string();

    assert_ne!(
        before_ordinal, after_ordinal,
        "position_ordinal in .md file must change after task.move; before={}, after={}",
        before_ordinal, after_ordinal
    );
}

/// Reproduces the exact bug: 4 cards [A, B, C, D], drag C (3rd) to position 2 (before B).
/// This is the "move 3rd card to 2nd position" scenario that fails in the UI.
#[tokio::test]
async fn reorder_move_third_before_second() {
    let engine = TestEngine::new().await;
    let ids = add_tasks(&engine, &["A", "B", "C", "D"]).await;

    // Verify initial order: [A, B, C, D]
    let order = todo_order(&engine).await;
    assert_eq!(
        order,
        vec![
            ids[0].clone(),
            ids[1].clone(),
            ids[2].clone(),
            ids[3].clone()
        ]
    );

    // Move C before B → expected [A, C, B, D]
    move_before(&engine, &ids[2], &ids[1]).await;
    let order = todo_order(&engine).await;
    assert_eq!(
        order,
        vec![
            ids[0].clone(),
            ids[2].clone(),
            ids[1].clone(),
            ids[3].clone()
        ],
        "C should be before B after move_before(C, B)"
    );
}

/// Verify repeated same-column reorder: move card back and forth.
#[tokio::test]
async fn reorder_move_third_before_second_then_back() {
    let engine = TestEngine::new().await;
    let ids = add_tasks(&engine, &["A", "B", "C", "D"]).await;

    // Move C before B → [A, C, B, D]
    move_before(&engine, &ids[2], &ids[1]).await;
    let order = todo_order(&engine).await;
    assert_eq!(
        order,
        vec![
            ids[0].clone(),
            ids[2].clone(),
            ids[1].clone(),
            ids[3].clone()
        ],
    );

    // Move C back after B → [A, B, C, D] (original order)
    move_after(&engine, &ids[2], &ids[1]).await;
    let order = todo_order(&engine).await;
    assert_eq!(
        order,
        vec![
            ids[0].clone(),
            ids[1].clone(),
            ids[2].clone(),
            ids[3].clone()
        ],
        "should return to original order after moving C back after B"
    );
}

// ===========================================================================
// Card 01KMT7ZF59AYKGRA62DBTR9Y6E — DeleteTaskCmd::execute
// ===========================================================================

#[tokio::test]
async fn task_delete_removes_task() {
    let engine = TestEngine::new().await;

    // Add a task
    let add_result = engine
        .dispatch_simple("task.add", &["column:todo"], None)
        .await
        .expect("task.add should succeed");
    let task_id = add_result["id"].as_str().unwrap();

    // Verify the task exists
    assert!(
        engine
            .kanban
            .read_entity_generic("task", task_id)
            .await
            .is_ok(),
        "task should exist after add"
    );

    // Dispatch task.delete through the command harness
    let delete_result = engine
        .dispatch_simple(
            "task.delete",
            &[&format!("task:{}", task_id)],
            None,
        )
        .await
        .expect("task.delete should succeed");

    assert!(
        delete_result.get("operation_id").is_some(),
        "result should contain operation_id"
    );

    // Verify the task is gone
    assert!(
        engine
            .kanban
            .read_entity_generic("task", task_id)
            .await
            .is_err(),
        "task should not exist after delete"
    );
}

// ===========================================================================
// Card 01KMT7ZAFSH0BW898EV3MF3M5E — MoveTaskCmd swimlane arg
// ===========================================================================

#[tokio::test]
async fn task_move_with_swimlane_arg() {
    let engine = TestEngine::new().await;

    // Create a swimlane via the lower-level API
    let processor = KanbanOperationProcessor::new();
    processor
        .process(
            &swissarmyhammer_kanban::swimlane::AddSwimlane::new("urgent", "Urgent"),
            &engine.kanban,
        )
        .await
        .expect("swimlane creation should succeed");

    // Add a task in todo
    let add_result = engine
        .dispatch_simple("task.add", &["column:todo"], None)
        .await
        .expect("task.add should succeed");
    let task_id = add_result["id"].as_str().unwrap();

    // Verify task has no swimlane initially
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .unwrap();
    assert!(
        task.get_str("position_swimlane").is_none()
            || task.get_str("position_swimlane") == Some(""),
        "task should have no swimlane initially"
    );

    // Move task to doing with swimlane arg
    let mut args = HashMap::new();
    args.insert("column".to_string(), json!("doing"));
    args.insert("swimlane".to_string(), json!("urgent"));

    let move_result = engine
        .dispatch(
            "task.move",
            &[&format!("task:{}", task_id)],
            None,
            args,
        )
        .await
        .expect("task.move with swimlane should succeed");

    assert!(move_result.get("operation_id").is_some());

    // Verify the task is in the correct column and swimlane
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .unwrap();
    assert_eq!(
        task.get_str("position_column"),
        Some("doing"),
        "task should be in doing column"
    );
    assert_eq!(
        task.get_str("position_swimlane"),
        Some("urgent"),
        "task should be in urgent swimlane"
    );
}

// ===========================================================================
// ===========================================================================
// Card 01KMT7Z4560S3CSHNQVR7GQ7PY — DragCompleteCmd same-board
// ===========================================================================

#[tokio::test]
async fn drag_complete_same_board_moves_task() {
    let engine = TestEngine::new().await;

    // Add a task in todo
    let add_result = engine
        .dispatch_simple("task.add", &["column:todo"], None)
        .await
        .expect("task.add should succeed");
    let task_id = add_result["id"].as_str().unwrap();

    // Start a drag session
    let board_path = engine.kanban.root().to_string_lossy().to_string();
    let mut start_args = HashMap::new();
    start_args.insert("taskId".to_string(), json!(task_id));
    start_args.insert("boardPath".to_string(), json!(&board_path));

    engine
        .dispatch("drag.start", &[], None, start_args)
        .await
        .expect("drag.start should succeed");

    // Complete the drag on the same board, targeting "doing" column
    let mut complete_args = HashMap::new();
    complete_args.insert("targetBoardPath".to_string(), json!(&board_path));
    complete_args.insert("targetColumn".to_string(), json!("doing"));

    let result = engine
        .dispatch("drag.complete", &[], None, complete_args)
        .await
        .expect("drag.complete should succeed");

    // Verify the result indicates same-board completion
    let drag_complete = result
        .get("DragComplete")
        .expect("result should have DragComplete key");
    assert_eq!(
        drag_complete["same_board"].as_bool(),
        Some(true),
        "should be same-board drag"
    );
    assert_eq!(
        drag_complete["task_id"].as_str(),
        Some(task_id),
        "task_id should match"
    );
    assert_eq!(
        drag_complete["target_column"].as_str(),
        Some("doing"),
        "target column should be doing"
    );

    // Verify the task actually moved to "doing"
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .unwrap();
    assert_eq!(
        task.get_str("position_column"),
        Some("doing"),
        "task should be in doing column after drag complete"
    );
}

// ===========================================================================
// Card 01KMT7Z7N9FD6ZJ7K48FPDQFM4 — DragCompleteCmd cross-board
// ===========================================================================

#[tokio::test]
async fn drag_complete_cross_board_returns_transfer_params() {
    let engine = TestEngine::new().await;

    // Add a task in todo
    let add_result = engine
        .dispatch_simple("task.add", &["column:todo"], None)
        .await
        .expect("task.add should succeed");
    let task_id = add_result["id"].as_str().unwrap();

    // Start a drag session with source board path
    let source_board_path = "/boards/source/.kanban";
    let mut start_args = HashMap::new();
    start_args.insert("taskId".to_string(), json!(task_id));
    start_args.insert("boardPath".to_string(), json!(source_board_path));

    engine
        .dispatch("drag.start", &[], None, start_args)
        .await
        .expect("drag.start should succeed");

    // Complete the drag targeting a DIFFERENT board path
    let target_board_path = "/boards/target/.kanban";
    let mut complete_args = HashMap::new();
    complete_args.insert("targetBoardPath".to_string(), json!(target_board_path));
    complete_args.insert("targetColumn".to_string(), json!("done"));
    complete_args.insert("dropIndex".to_string(), json!(0));

    let result = engine
        .dispatch("drag.complete", &[], None, complete_args)
        .await
        .expect("drag.complete should succeed");

    // Verify the result indicates cross-board completion
    let drag_complete = result
        .get("DragComplete")
        .expect("result should have DragComplete key");
    assert_eq!(
        drag_complete["same_board"].as_bool(),
        Some(false),
        "should not be same-board"
    );
    assert_eq!(
        drag_complete["cross_board"].as_bool(),
        Some(true),
        "should be cross-board drag"
    );
    assert_eq!(
        drag_complete["source_board_path"].as_str(),
        Some(source_board_path),
        "source board path should match"
    );
    assert_eq!(
        drag_complete["target_board_path"].as_str(),
        Some(target_board_path),
        "target board path should match"
    );
    assert_eq!(
        drag_complete["task_id"].as_str(),
        Some(task_id),
        "task_id should match"
    );
    assert_eq!(
        drag_complete["target_column"].as_str(),
        Some("done"),
        "target column should be done"
    );
    assert_eq!(
        drag_complete["drop_index"].as_u64(),
        Some(0),
        "drop_index should be passed through"
    );
    assert_eq!(
        drag_complete["copy_mode"].as_bool(),
        Some(false),
        "copy_mode should default to false"
    );
}
