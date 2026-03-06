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

        if !cmd.available(&ctx) {
            return Err(CommandError::ExecutionFailed(format!(
                "command '{}' not available in this context",
                cmd_id
            )));
        }

        cmd.execute(&ctx).await
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
    assert!(
        result.get("id").is_some(),
        "result should contain task id"
    );

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
        .dispatch(
            "task.move",
            &[&format!("task:{}", task_id)],
            None,
            args,
        )
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
            &[&format!("tag:bug"), &format!("task:{}", task_id)],
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

    let result = engine
        .dispatch_simple("task.add", &[], None)
        .await;

    assert!(result.is_err(), "task.add should fail without column in scope");
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

    assert_eq!(engine.ui_state.inspector_stack(), vec!["task:01XYZ"]);
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
        engine.ui_state.inspector_stack(),
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

    assert_eq!(engine.ui_state.inspector_stack(), vec!["task:01XYZ"]);
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

    assert!(engine.ui_state.inspector_stack().is_empty());
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

    // Undo the add
    let mut undo_args = HashMap::new();
    undo_args.insert("id".to_string(), json!(operation_id));
    let undo_result = engine
        .dispatch("app.undo", &[], None, undo_args)
        .await
        .expect("app.undo should succeed");

    assert_eq!(undo_result["undone"].as_str(), Some(operation_id));
    let undo_op_id = undo_result["operation_id"].as_str();

    // Task should be gone (trashed)
    assert!(
        engine
            .kanban
            .read_entity_generic("task", task_id)
            .await
            .is_err(),
        "task should not exist after undo"
    );

    // Redo the add (undo the undo) — use the undo operation_id
    if let Some(redo_id) = undo_op_id {
        let mut redo_args = HashMap::new();
        redo_args.insert("id".to_string(), json!(redo_id));
        let redo_result = engine
            .dispatch("app.redo", &[], None, redo_args)
            .await
            .expect("app.redo should succeed");

        assert_eq!(redo_result["redone"].as_str(), Some(redo_id));

        // Task should be back
        let task = engine
            .kanban
            .read_entity_generic("task", task_id)
            .await
            .expect("task should exist after redo");
        assert!(task.get_str("title").is_some());
    }
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
    let update_op_id = update_result["operation_id"].as_str().unwrap();

    // Verify title changed
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .unwrap();
    assert_eq!(task.get_str("title"), Some("Changed Title"));

    // Undo the update
    let mut undo_args = HashMap::new();
    undo_args.insert("id".to_string(), json!(update_op_id));
    engine
        .dispatch("app.undo", &[], None, undo_args)
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
