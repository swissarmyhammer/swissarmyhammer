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
use swissarmyhammer_entity::EntityTypeStore;
use swissarmyhammer_kanban::clipboard::{
    ClipboardProvider, ClipboardProviderExt, InMemoryClipboard,
};
use swissarmyhammer_kanban::commands::register_commands;
use swissarmyhammer_kanban::defaults::builtin_view_definitions;
use swissarmyhammer_kanban::scope_commands::{commands_for_scope, DynamicSources, ViewInfo};
use swissarmyhammer_kanban::{
    board::InitBoard, KanbanContext, KanbanOperationProcessor, OperationProcessor,
};
use swissarmyhammer_store::{StoreContext, StoreHandle};
use swissarmyhammer_views::{ViewKind, ViewsContext};
use tempfile::TempDir;

// ===========================================================================
// TestEngine
// ===========================================================================

/// Lightweight harness that wires up a temp board, command registry, command
/// implementations, shared UIState, and in-memory clipboard for integration tests.
struct TestEngine {
    _temp: TempDir,
    kanban: Arc<KanbanContext>,
    commands: HashMap<String, Arc<dyn Command>>,
    _registry: CommandsRegistry,
    ui_state: Arc<UIState>,
    clipboard: Arc<InMemoryClipboard>,
    /// Optional StoreContext — present when created via `with_store_context()`.
    store_context: Option<Arc<StoreContext>>,
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
        let clipboard = Arc::new(InMemoryClipboard::new());

        Self {
            _temp: temp,
            kanban,
            commands,
            _registry: registry,
            ui_state,
            clipboard,
            store_context: None,
        }
    }

    /// Create a new test engine with store handles registered (production-like).
    ///
    /// This mirrors how `kanban-app/src/state.rs` sets up the EntityContext:
    /// every entity type gets a StoreHandle registered so writes go through
    /// the undo-capable store path instead of the legacy io path.
    async fn with_store_handles() -> Self {
        let engine = Self::new().await;

        // Register StoreHandle for every entity type, just like production
        let ectx = engine
            .kanban
            .entity_context()
            .await
            .expect("entity_context should be available");

        let fields_ctx = ectx.fields();
        for entity_def in fields_ctx.all_entities() {
            let entity_type = entity_def.name.as_str();
            let field_defs = fields_ctx.fields_for_entity(entity_type);
            let owned_defs: Vec<_> = field_defs.into_iter().cloned().collect();
            let entity_type_store = EntityTypeStore::new(
                ectx.entity_dir(entity_type),
                entity_type,
                std::sync::Arc::new(entity_def.clone()),
                std::sync::Arc::new(owned_defs),
            );
            let handle =
                std::sync::Arc::new(StoreHandle::new(std::sync::Arc::new(entity_type_store)));
            ectx.register_store(entity_type, handle).await;
        }

        engine
    }

    /// Create a test engine with both StoreHandles AND a StoreContext.
    ///
    /// This is the full production-like setup: every entity type store is
    /// registered with both EntityContext (for write delegation) and
    /// StoreContext (for flush_all / undo). Use this when testing the
    /// event pipeline end-to-end.
    async fn with_store_context() -> Self {
        let mut engine = Self::new().await;

        let store_context = Arc::new(StoreContext::new(engine.kanban.root().to_path_buf()));

        let ectx = engine
            .kanban
            .entity_context()
            .await
            .expect("entity_context should be available");

        // Wire StoreContext into EntityContext for undo stack integration
        ectx.set_store_context(Arc::clone(&store_context));

        let fields_ctx = ectx.fields();
        for entity_def in fields_ctx.all_entities() {
            let entity_type = entity_def.name.as_str();
            let field_defs = fields_ctx.fields_for_entity(entity_type);
            let owned_defs: Vec<_> = field_defs.into_iter().cloned().collect();
            let entity_type_store = EntityTypeStore::new(
                ectx.entity_dir(entity_type),
                entity_type,
                std::sync::Arc::new(entity_def.clone()),
                std::sync::Arc::new(owned_defs),
            );
            let handle =
                std::sync::Arc::new(StoreHandle::new(std::sync::Arc::new(entity_type_store)));
            ectx.register_store(entity_type, handle.clone()).await;
            store_context.register(handle).await;
        }

        engine.store_context = Some(store_context);
        engine
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
        // Inject EntityContext so undo/redo commands can access it
        let ectx = self
            .kanban
            .entity_context()
            .await
            .expect("entity_context should be available");
        ctx.set_extension(ectx);
        let clipboard_ext = ClipboardProviderExt(Arc::clone(&self.clipboard)
            as Arc<dyn swissarmyhammer_kanban::clipboard::ClipboardProvider>);
        ctx.set_extension(Arc::new(clipboard_ext));

        if !cmd.available(&ctx) {
            return Err(CommandError::ExecutionFailed(format!(
                "command '{}' not available in this context",
                cmd_id
            )));
        }

        let result = cmd.execute(&ctx).await;

        // Undo/redo state sync is now handled through StoreContext in the
        // Tauri layer. In tests without a StoreContext, this is a no-op.

        result
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

    /// Create a task via the unified `entity.add` command with
    /// `entity_type: task` and an optional column override. This is the
    /// single production creation path — tests that need a fixture task
    /// must use this helper so any regression in the `entity.add` pipeline
    /// is caught by setup, not by dispatch tests further down.
    async fn add_task(
        &self,
        column: Option<&str>,
        extra_args: HashMap<String, Value>,
    ) -> swissarmyhammer_commands::Result<Value> {
        let mut args = extra_args;
        args.insert("entity_type".to_string(), json!("task"));
        if let Some(c) = column {
            args.insert("column".to_string(), json!(c));
        }
        self.dispatch("entity.add", &[], None, args).await
    }
}

// ===========================================================================
// Command dispatch tests
// ===========================================================================

#[tokio::test]
async fn task_add_creates_task() {
    let engine = TestEngine::new().await;

    let result = engine
        .add_task(Some("todo"), HashMap::new())
        .await
        .expect("entity.add:task should succeed");

    // Should return a JSON object with a task id
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
    let add_result = engine.add_task(Some("todo"), HashMap::new()).await.unwrap();
    let task_id = add_result["id"].as_str().unwrap();

    // Move to doing via args
    let mut args = HashMap::new();
    args.insert("column".to_string(), json!("doing"));

    engine
        .dispatch("task.move", &[&format!("task:{}", task_id)], None, args)
        .await
        .expect("task.move should succeed");

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
    let _result = engine
        .dispatch_simple(
            "task.untag",
            &["tag:bug", &format!("task:{}", task_id)],
            None,
        )
        .await
        .expect("task.untag should succeed");

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
    let add_result = engine.add_task(Some("todo"), HashMap::new()).await.unwrap();
    let task_id = add_result["id"].as_str().unwrap();

    // Update the title via entity.update_field
    let mut args = HashMap::new();
    args.insert("entity_type".to_string(), json!("task"));
    args.insert("id".to_string(), json!(task_id));
    args.insert("field_name".to_string(), json!("title"));
    args.insert("value".to_string(), json!("New Title"));

    engine
        .dispatch("entity.update_field", &[], None, args)
        .await
        .expect("entity.update_field should succeed");

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

/// The legacy `task.add` command is retired — creation flows through
/// dynamic `entity.add:task`. This regression guard proves the registry
/// no longer exposes `task.add` so a future refactor can't silently
/// re-introduce the duplicate "New Task" palette item (and its slug-id
/// collision on repeated creates) by re-registering the command.
#[tokio::test]
async fn task_add_retired_registry_rejects_it() {
    let engine = TestEngine::new().await;

    let result = engine
        .dispatch_simple("task.add", &["column:todo"], None)
        .await;

    let err = result.expect_err("task.add must be rejected — use entity.add:task");
    let msg = err.to_string();
    assert!(
        msg.contains("unknown command: task.add"),
        "expected `unknown command: task.add`, got: {msg}"
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
async fn inspect_uses_window_from_scope_chain() {
    let engine = TestEngine::new().await;

    // Dispatch inspect with a scope chain that includes window:board-2
    // This simulates pressing (i) in a secondary window.
    engine
        .dispatch_simple(
            "ui.inspect",
            &["task:01XYZ", "column:todo", "window:board-2"],
            Some("task:01XYZ"),
        )
        .await
        .expect("ui.inspect should succeed");

    // The inspector stack should be on window "board-2", NOT "main"
    assert_eq!(
        engine.ui_state.inspector_stack("board-2"),
        vec!["task:01XYZ"],
        "inspector should open in the window specified by scope chain"
    );
    assert!(
        engine.ui_state.inspector_stack("main").is_empty(),
        "main window inspector should be unaffected"
    );
}

#[tokio::test]
async fn inspector_close_uses_window_from_scope_chain() {
    let engine = TestEngine::new().await;

    // Open inspector in board-2
    engine
        .dispatch_simple(
            "ui.inspect",
            &["task:01XYZ", "window:board-2"],
            Some("task:01XYZ"),
        )
        .await
        .unwrap();

    // Close inspector in board-2 (scope chain carries the window)
    engine
        .dispatch_simple(
            "ui.inspector.close",
            &["task:01XYZ", "window:board-2"],
            None,
        )
        .await
        .expect("ui.inspector.close should succeed");

    assert!(
        engine.ui_state.inspector_stack("board-2").is_empty(),
        "board-2 inspector should be closed"
    );
}

#[tokio::test]
async fn inspect_without_window_in_scope_falls_back_to_main() {
    let engine = TestEngine::new().await;

    // No window: moniker in scope chain — should fall back to "main"
    engine
        .dispatch_simple(
            "ui.inspect",
            &["task:01XYZ", "column:todo"],
            Some("task:01XYZ"),
        )
        .await
        .expect("ui.inspect should succeed");

    assert_eq!(
        engine.ui_state.inspector_stack("main"),
        vec!["task:01XYZ"],
        "should fall back to main when no window in scope"
    );
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
async fn full_session_add_move_update() {
    let engine = TestEngine::new().await;

    // 1. Add a task in todo
    let add_result = engine
        .add_task(Some("todo"), HashMap::new())
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
            .add_task(Some("todo"), args)
            .await
            .expect("entity.add:task should succeed");
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

/// The YAML registry must mark task.move as undoable — this is the gate that
/// lets the write-through `EntityCache` emit `entity-field-changed` events on
/// commit.
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
        "task.move must be marked undoable so the write-through cache emits entity-field-changed events on commit"
    );
}

/// `entity.add` must be declared in the YAML registry.
///
/// The dynamic `entity.add:{type}` palette / context-menu command is rewritten
/// by `kanban-app::rewrite_dynamic_prefix` to the canonical command id
/// `entity.add`. `dispatch_command_internal` then calls `lookup_undoable` which
/// requires a registry entry — without it, every runtime dispatch of
/// `entity.add:task` / `entity.add:tag` / `entity.add:project` fails with
/// `"Unknown command: entity.add"`. This test is the regression guard for that
/// class of bug.
///
/// The canonical `entity.add` is `visible: false` — the palette entries come
/// from the dynamic `entity.add:{type}` synthesised by `emit_dynamic_commands`,
/// not from the static registry.
#[tokio::test]
async fn entity_add_is_registered_undoable_and_hidden() {
    let registry = CommandsRegistry::from_yaml_sources(&builtin_yaml_sources());
    let cmd_def = registry
        .get("entity.add")
        .expect("entity.add must exist in the YAML registry — the dynamic entity.add:{type} dispatch rewrites to this canonical id");

    assert!(
        cmd_def.undoable,
        "entity.add must be marked undoable — creation goes through the undo stack"
    );
    assert!(
        !cmd_def.visible,
        "entity.add must be visible: false — palette entries are synthesised dynamically as entity.add:{{type}}"
    );
}

/// End-to-end: dispatching `entity.add` with `entity_type: task` in the arg
/// bag (the shape produced by `rewrite_dynamic_prefix` after stripping the
/// `entity.add:task` prefix) must create a task entity in the lowest-order
/// column and return its id.
///
/// This mirrors the production path: frontend palette/context-menu fires
/// `entity.add:task`, `rewrite_dynamic_prefix` rewrites to `entity.add` with
/// `entity_type: "task"` merged into args, then dispatch flows through the
/// registry → `AddEntityCmd` → `AddEntity` operation. Prior to adding the
/// YAML registry entry, this dispatch failed at `lookup_undoable` with
/// "Unknown command: entity.add" before the impl was ever reached.
#[tokio::test]
async fn dispatch_entity_add_task_creates_task_in_lowest_order_column() {
    let engine = TestEngine::new().await;

    let mut args = HashMap::new();
    args.insert("entity_type".to_string(), json!("task"));

    let result = engine
        .dispatch("entity.add", &[], None, args)
        .await
        .expect("entity.add should succeed after registry entry exists");

    // Returned payload is the serialized entity
    let task_id = result["id"].as_str().expect("entity.add must return an id");
    assert_eq!(
        result["position_column"], "todo",
        "task must land in the lowest-order column when no override is given"
    );

    // Verify the task was actually persisted
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .expect("task should exist on disk after dispatch");
    assert_eq!(task.get_str("position_column"), Some("todo"));
    assert_eq!(task.get_str("title"), Some("Untitled"));
}

/// Isolated per-entity-type dispatch guard for `task`.
///
/// The task-specific name `dispatch_entity_add_task_creates_task` is deliberate:
/// one named, isolated guard per entity type so a future regression on a
/// single type cannot hide behind a parameterised pass. Read + write through
/// the real `AddEntity` op with the real `FieldsContext`, asserting a file on
/// disk under `.kanban/tasks/` with the schema-default title populated.
#[tokio::test]
async fn dispatch_entity_add_task_creates_task() {
    let engine = TestEngine::new().await;

    let mut args = HashMap::new();
    args.insert("entity_type".to_string(), json!("task"));

    let result = engine
        .dispatch("entity.add", &[], None, args)
        .await
        .expect("entity.add:task must succeed end-to-end through the registry");

    let task_id = result["id"]
        .as_str()
        .expect("entity.add:task must return an id");
    assert!(!task_id.is_empty(), "id must be non-empty");

    // Schema defaults: title comes from title.yaml's `default:` value.
    // A missing default here would silently poison every new task.
    assert_eq!(
        result["title"], "Untitled",
        "task.title must be populated from the schema default"
    );
    // Task-shaped entities must get position resolution applied.
    assert_eq!(
        result["position_column"], "todo",
        "task must land in lowest-order column"
    );
    assert!(
        result.get("position_ordinal").is_some(),
        "task must have a resolved position_ordinal"
    );

    // On-disk verification — dispatch is not complete until the file lands.
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .expect("task file must exist on disk after dispatch");
    assert_eq!(task.get_str("title"), Some("Untitled"));
    assert_eq!(task.get_str("position_column"), Some("todo"));
}

/// Isolated per-entity-type dispatch guard for `tag`.
///
/// Mirrors `dispatch_entity_add_task_creates_task` but for the tag entity
/// type. Surfaces any `tag_name.yaml` default-drift as a single-named
/// failure instead of hiding behind a parameterised pass.
#[tokio::test]
async fn dispatch_entity_add_tag_creates_tag() {
    let engine = TestEngine::new().await;

    let mut args = HashMap::new();
    args.insert("entity_type".to_string(), json!("tag"));

    let result = engine
        .dispatch("entity.add", &[], None, args)
        .await
        .expect("entity.add:tag must succeed end-to-end through the registry");

    let tag_id = result["id"]
        .as_str()
        .expect("entity.add:tag must return an id");
    assert!(!tag_id.is_empty(), "id must be non-empty");

    // Schema default from tag_name.yaml. If this regresses, every new tag
    // starts life nameless — the UI creates but the user sees nothing.
    assert_eq!(
        result["tag_name"], "new-tag",
        "tag.tag_name must be populated from the schema default"
    );

    // On-disk verification.
    let tag = engine
        .kanban
        .read_entity_generic("tag", tag_id)
        .await
        .expect("tag file must exist on disk after dispatch");
    assert_eq!(tag.get_str("tag_name"), Some("new-tag"));
}

/// Isolated per-entity-type dispatch guard for `project`.
///
/// Mirrors `dispatch_entity_add_task_creates_task` but for the project
/// entity type. Catches any drift in `name.yaml`'s `default:` independently
/// of task/tag coverage — the 2026-04 "New Project does nothing" regression
/// looked identical from the parameterised test POV.
#[tokio::test]
async fn dispatch_entity_add_project_creates_project() {
    let engine = TestEngine::new().await;

    let mut args = HashMap::new();
    args.insert("entity_type".to_string(), json!("project"));

    let result = engine
        .dispatch("entity.add", &[], None, args)
        .await
        .expect("entity.add:project must succeed end-to-end through the registry");

    let project_id = result["id"]
        .as_str()
        .expect("entity.add:project must return an id");
    assert!(!project_id.is_empty(), "id must be non-empty");

    // Schema default from name.yaml.
    assert_eq!(
        result["name"], "New item",
        "project.name must be populated from the schema default"
    );

    // On-disk verification.
    let project = engine
        .kanban
        .read_entity_generic("project", project_id)
        .await
        .expect("project file must exist on disk after dispatch");
    assert_eq!(project.get_str("name"), Some("New item"));
}

/// End-to-end: dispatching `entity.add` with `entity_type: tag` must create a
/// tag entity populated with the schema-declared `default` for `tag_name`.
#[tokio::test]
async fn dispatch_entity_add_tag_creates_tag_with_defaults() {
    let engine = TestEngine::new().await;

    let mut args = HashMap::new();
    args.insert("entity_type".to_string(), json!("tag"));

    let result = engine
        .dispatch("entity.add", &[], None, args)
        .await
        .expect("entity.add should succeed for tag");

    assert_eq!(
        result["tag_name"], "new-tag",
        "tag must be created with the schema default `tag_name` value"
    );
    assert!(result["id"].as_str().is_some_and(|s| !s.is_empty()));
}

/// End-to-end: dispatching `entity.add` with `entity_type: project` must create
/// a project entity populated with the schema-declared `default` for `name`.
///
/// Mirrors `dispatch_entity_add_tag_creates_tag_with_defaults` — this is the
/// regression guard proving the projects grid's `+` button creates a persisted
/// project file exactly the same way the tags grid's `+` button creates a tag.
/// Before this test existed, `entity.add:project` went through the dynamic
/// prefix-rewrite and canonical-`entity.add` registry entry but had no
/// coverage — the tags grid worked, and everyone assumed the other grids did
/// too.
#[tokio::test]
async fn dispatch_entity_add_project_creates_project_with_defaults() {
    let engine = TestEngine::new().await;

    let mut args = HashMap::new();
    args.insert("entity_type".to_string(), json!("project"));

    let result = engine
        .dispatch("entity.add", &[], None, args)
        .await
        .expect("entity.add should succeed for project");

    assert_eq!(
        result["name"], "New item",
        "project must be created with the schema default `name` value"
    );
    let project_id = result["id"]
        .as_str()
        .expect("entity.add must return an id for project");
    assert!(!project_id.is_empty());

    // Verify the project was actually persisted to disk under .kanban/projects/
    let project = engine
        .kanban
        .read_entity_generic("project", project_id)
        .await
        .expect("project should exist on disk after dispatch");
    assert_eq!(project.get_str("name"), Some("New item"));
}

/// End-to-end: explicit `column` arg in the dispatch bag must override the
/// lowest-order auto-resolution, proving the generic-override pipeline from
/// `CommandContext.args` → `AddEntityCmd` → `AddEntity.overrides` is wired up.
#[tokio::test]
async fn dispatch_entity_add_task_honors_explicit_column_override() {
    let engine = TestEngine::new().await;

    let mut args = HashMap::new();
    args.insert("entity_type".to_string(), json!("task"));
    args.insert("column".to_string(), json!("doing"));

    let result = engine
        .dispatch("entity.add", &[], None, args)
        .await
        .expect("entity.add with column override should succeed");

    assert_eq!(
        result["position_column"], "doing",
        "explicit `column` arg must override lowest-order auto-placement"
    );
}

/// Unified creation path: the same `entity.add` dispatch powers every UI
/// entry point (board column (+), grid (+), palette, context menu) across
/// every entity type (task, tag, project). This test exercises all three
/// in one pass so the "one true creation path" invariant holds for the full
/// set — a regression in any of them shows up as a single failing test
/// rather than hiding behind per-type coverage.
///
/// For `task`, the `column` override mirrors what the board's column (+)
/// button sends; `tag` and `project` mirror the grid (+) flows that need no
/// per-type context.
#[tokio::test]
async fn dispatch_entity_add_unified_path_for_task_tag_project() {
    let engine = TestEngine::new().await;

    // Task: column (+) and board.newTask path — `column` override present.
    let mut task_args = HashMap::new();
    task_args.insert("entity_type".to_string(), json!("task"));
    task_args.insert("column".to_string(), json!("doing"));
    let task_result = engine
        .dispatch("entity.add", &[], None, task_args)
        .await
        .expect("entity.add:task (column-override path) must succeed");
    assert_eq!(
        task_result["position_column"], "doing",
        "task created via the unified path must honour the column override",
    );
    assert_eq!(
        task_result["title"], "Untitled",
        "task gets the schema-default title — the UI passes no title override",
    );

    // Tag: tags-grid (+) path.
    let mut tag_args = HashMap::new();
    tag_args.insert("entity_type".to_string(), json!("tag"));
    let tag_result = engine
        .dispatch("entity.add", &[], None, tag_args)
        .await
        .expect("entity.add:tag must succeed");
    assert_eq!(
        tag_result["tag_name"], "new-tag",
        "tag must land with the schema-declared `tag_name` default",
    );

    // Project: projects-grid (+) path. Before the Projects view was
    // registered end-to-end, this dispatch succeeded in isolation but the
    // user could never trigger it from the UI.
    let mut project_args = HashMap::new();
    project_args.insert("entity_type".to_string(), json!("project"));
    let project_result = engine
        .dispatch("entity.add", &[], None, project_args)
        .await
        .expect("entity.add:project must succeed");
    assert_eq!(
        project_result["name"], "New item",
        "project must land with the schema-declared `name` default",
    );

    // Persistence round-trip for all three — the dispatch isn't enough,
    // the entity must actually live on disk once the command returns.
    for (entity_type, result) in [
        ("task", &task_result),
        ("tag", &tag_result),
        ("project", &project_result),
    ] {
        let id = result["id"]
            .as_str()
            .unwrap_or_else(|| panic!("entity.add:{entity_type} must return an id in result"));
        engine
            .kanban
            .read_entity_generic(entity_type, id)
            .await
            .unwrap_or_else(|e| {
                panic!("{entity_type} {id} should be persisted after entity.add: {e}")
            });
    }
}

/// `entity.add` without an `entity_type` arg must be unavailable — the impl
/// returns `false` from `available()` so dispatch refuses to execute. This
/// mirrors the "you need a view with entity_type in scope" invariant on the
/// palette side at the command-impl level.
#[tokio::test]
async fn dispatch_entity_add_unavailable_without_entity_type_arg() {
    let engine = TestEngine::new().await;

    let result = engine
        .dispatch("entity.add", &[], None, HashMap::new())
        .await;

    assert!(
        result.is_err(),
        "entity.add must fail when entity_type is missing from args"
    );
}

/// After task.move, the task's .md file on disk must have the updated position_ordinal.
/// This is the precondition for the cache diff to detect the change and fire events.
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
        .add_task(Some("todo"), HashMap::new())
        .await
        .expect("entity.add:task should succeed");
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
    engine
        .dispatch_simple("task.delete", &[&format!("task:{}", task_id)], None)
        .await
        .expect("task.delete should succeed");

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
// (task.tag command was removed — tagging is tested via paste-tag and unit tests)

// ===========================================================================
// Card 01KMT7Z4560S3CSHNQVR7GQ7PY — DragCompleteCmd same-board
// ===========================================================================

#[tokio::test]
async fn drag_complete_same_board_moves_task() {
    let engine = TestEngine::new().await;

    // Add a task in todo
    let add_result = engine
        .add_task(Some("todo"), HashMap::new())
        .await
        .expect("entity.add:task should succeed");
    let task_id = add_result["id"].as_str().unwrap();

    // Start a drag session — board path derived from scope chain's store: moniker
    let board_path = engine.kanban.root().to_string_lossy().to_string();
    let store_moniker = format!("store:{}", board_path);
    let mut start_args = HashMap::new();
    start_args.insert("taskId".to_string(), json!(task_id));

    engine
        .dispatch("drag.start", &[store_moniker.as_str()], None, start_args)
        .await
        .expect("drag.start should succeed");

    // Complete the drag on the same board, targeting "doing" column
    // Target board path also from scope chain
    let mut complete_args = HashMap::new();
    complete_args.insert("targetColumn".to_string(), json!("doing"));

    let result = engine
        .dispatch(
            "drag.complete",
            &[store_moniker.as_str()],
            None,
            complete_args,
        )
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
        .add_task(Some("todo"), HashMap::new())
        .await
        .expect("entity.add:task should succeed");
    let task_id = add_result["id"].as_str().unwrap();

    // Start a drag session — source board path from scope chain store: moniker
    let source_board_path = "/boards/source/.kanban";
    let source_store = format!("store:{}", source_board_path);
    let mut start_args = HashMap::new();
    start_args.insert("taskId".to_string(), json!(task_id));

    engine
        .dispatch("drag.start", &[source_store.as_str()], None, start_args)
        .await
        .expect("drag.start should succeed");

    // Complete the drag targeting a DIFFERENT board path via scope chain
    let target_board_path = "/boards/target/.kanban";
    let target_store = format!("store:{}", target_board_path);
    let mut complete_args = HashMap::new();
    complete_args.insert("targetColumn".to_string(), json!("done"));
    complete_args.insert("dropIndex".to_string(), json!(0));

    let result = engine
        .dispatch(
            "drag.complete",
            &[target_store.as_str()],
            None,
            complete_args,
        )
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

// ===========================================================================
// Clipboard command integration tests
// ===========================================================================

#[tokio::test]
async fn entity_copy_copies_task_to_clipboard() {
    let engine = TestEngine::new().await;

    // Create a task with a known title
    let mut args = HashMap::new();
    args.insert("title".to_string(), json!("Clipboard test task"));
    let add_result = engine
        .add_task(Some("todo"), args)
        .await
        .expect("entity.add:task should succeed");
    let task_id = add_result["id"].as_str().unwrap();

    // Dispatch entity.copy with task as target (cross-cutting command:
    // params: [{name: moniker, from: target}])
    let target = format!("task:{}", task_id);
    let result = engine
        .dispatch_simple("entity.copy", &[], Some(&target))
        .await
        .expect("entity.copy should succeed");

    assert_eq!(result["id"].as_str(), Some(task_id));
    assert_eq!(result["copied"].as_bool(), Some(true));

    // Verify the InMemoryClipboard has the task's fields as JSON (wrapped format)
    let clipboard_text = engine
        .clipboard
        .read_text()
        .await
        .unwrap()
        .expect("clipboard should have data");
    let clipboard_json: Value =
        serde_json::from_str(&clipboard_text).expect("should be valid JSON");
    let content = &clipboard_json["swissarmyhammer_clipboard"];
    assert_eq!(content["entity_id"].as_str(), Some(task_id));
    assert_eq!(content["entity_type"].as_str(), Some("task"));

    // Verify task still exists (copy is non-destructive)
    let task = engine
        .kanban
        .read_entity_generic("task", task_id)
        .await
        .expect("task should still exist after copy");
    assert_eq!(task.get_str("title"), Some("Clipboard test task"));

    // Verify has_clipboard flag was set
    assert!(
        engine.ui_state.has_clipboard(),
        "has_clipboard should be true after copy"
    );
}

// ===========================================================================
// StoreHandle integration — reproduces untag bug with production-like setup
// ===========================================================================

/// Untag through the StoreHandle path (production-like setup).
///
/// This test reproduces the "untag not working after StoreHandle migration" bug
/// by registering StoreHandle for all entity types, then exercising the full
/// tag → untag cycle.
#[tokio::test]
async fn task_untag_removes_tag_with_store_handles() {
    let engine = TestEngine::with_store_handles().await;

    let processor = KanbanOperationProcessor::new();

    // Create a task
    let add_result = processor
        .process(
            &swissarmyhammer_kanban::task::AddTask::new("Tagged task"),
            &engine.kanban,
        )
        .await
        .unwrap();
    let task_id = add_result["id"].as_str().unwrap();

    // Create tag definition
    processor
        .process(
            &swissarmyhammer_kanban::tag::AddTag::new("bug"),
            &engine.kanban,
        )
        .await
        .unwrap();

    // Tag the task (adds #bug to body)
    processor
        .process(
            &swissarmyhammer_kanban::task::TagTask::new(task_id, "bug"),
            &engine.kanban,
        )
        .await
        .unwrap();

    // Verify tag is present
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
        "task should have 'bug' tag before untag, got tags: {:?}",
        tags
    );
    let body_before = task.get_str("body").unwrap_or("").to_string();
    assert!(
        body_before.contains("#bug"),
        "body should contain '#bug' before untag, got: {}",
        body_before
    );

    // Untag via dispatch (same path as production)
    engine
        .dispatch_simple(
            "task.untag",
            &["tag:bug", &format!("task:{}", task_id)],
            None,
        )
        .await
        .expect("task.untag should succeed");

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
        "task should no longer have 'bug' tag after untag, got tags: {:?}",
        tags
    );
    let body_after = task.get_str("body").unwrap_or("").to_string();
    assert!(
        !body_after.contains("#bug"),
        "body should no longer contain '#bug' after untag, got: {}",
        body_after
    );
}

/// Tag and untag with legacy-written task files (migration scenario).
///
/// Simulates the production case where task files were written by the legacy
/// io path (before StoreHandle migration), then StoreHandle is registered and
/// untag is performed. The legacy path includes computed fields in frontmatter;
/// the StoreHandle path strips them.
#[tokio::test]
async fn task_untag_with_legacy_written_files() {
    // Phase 1: Create board and tasks WITHOUT store handles (legacy path)
    let engine = TestEngine::new().await;
    let processor = KanbanOperationProcessor::new();

    let add_result = processor
        .process(
            &swissarmyhammer_kanban::task::AddTask::new("Legacy task"),
            &engine.kanban,
        )
        .await
        .unwrap();
    let task_id = add_result["id"].as_str().unwrap().to_string();

    processor
        .process(
            &swissarmyhammer_kanban::tag::AddTag::new("bug"),
            &engine.kanban,
        )
        .await
        .unwrap();

    processor
        .process(
            &swissarmyhammer_kanban::task::TagTask::new(task_id.as_str(), "bug"),
            &engine.kanban,
        )
        .await
        .unwrap();

    // Verify tag is on the task (written by legacy path)
    let task = engine
        .kanban
        .read_entity_generic("task", &task_id)
        .await
        .unwrap();
    assert!(
        task.get_str("body").unwrap_or("").contains("#bug"),
        "body should contain #bug after tagging via legacy path"
    );

    // Phase 2: Register store handles (simulating app upgrade/restart)
    let ectx = engine
        .kanban
        .entity_context()
        .await
        .expect("entity_context should be available");
    let fields_ctx = ectx.fields();
    for entity_def in fields_ctx.all_entities() {
        let entity_type = entity_def.name.as_str();
        let field_defs = fields_ctx.fields_for_entity(entity_type);
        let owned_defs: Vec<_> = field_defs.into_iter().cloned().collect();
        let entity_type_store = EntityTypeStore::new(
            ectx.entity_dir(entity_type),
            entity_type,
            std::sync::Arc::new(entity_def.clone()),
            std::sync::Arc::new(owned_defs),
        );
        let handle = std::sync::Arc::new(StoreHandle::new(std::sync::Arc::new(entity_type_store)));
        ectx.register_store(entity_type, handle).await;
    }

    // Phase 3: Untag via StoreHandle path
    engine
        .dispatch_simple(
            "task.untag",
            &["tag:bug", &format!("task:{}", task_id)],
            None,
        )
        .await
        .expect("task.untag should succeed with store handles on legacy files");

    // Verify the tag was removed
    let task = engine
        .kanban
        .read_entity_generic("task", &task_id)
        .await
        .unwrap();
    let tags = task
        .get("tags")
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    assert!(
        !tags.iter().any(|t| t.as_str() == Some("bug")),
        "task should no longer have 'bug' tag after untag on legacy file, got tags: {:?}",
        tags
    );
    let body_after = task.get_str("body").unwrap_or("").to_string();
    assert!(
        !body_after.contains("#bug"),
        "body should no longer contain '#bug' after untag on legacy file, got: {}",
        body_after
    );
}

/// After task.move, `StoreContext.flush_all()` must return at least one
/// "item-changed" event for the moved task.  This is the mechanism the
/// write-through `EntityCache` relies on to emit `entity-field-changed`
/// events to the frontend.
#[tokio::test]
async fn task_move_produces_store_event_via_flush_all() {
    let engine = TestEngine::with_store_context().await;
    let ids = add_tasks(&engine, &["A", "B"]).await;

    let store_context = engine
        .store_context
        .as_ref()
        .expect("with_store_context should provide a StoreContext");

    // Drain any events from the initial add operations
    store_context.flush_all().await;

    // Move task A to done column
    let mut args = HashMap::new();
    args.insert("task".to_string(), json!(ids[0]));
    args.insert("column".to_string(), json!("done"));
    engine
        .dispatch("task.move", &[&format!("task:{}", ids[0])], None, args)
        .await
        .expect("task.move should succeed");

    // flush_all must return at least one event
    let events = store_context.flush_all().await;
    assert!(
        !events.is_empty(),
        "flush_all() must return events after task.move — got none"
    );

    // The event must be an "item-changed" for the moved task
    let task_event = events.iter().find(|e| {
        e.event_name() == "item-changed"
            && e.payload().get("store").and_then(|v| v.as_str()) == Some("task")
            && e.payload().get("id").and_then(|v| v.as_str()) == Some(ids[0].as_str())
    });
    assert!(
        task_event.is_some(),
        "flush_all() must contain an item-changed event for the moved task; got: {:?}",
        events
            .iter()
            .map(|e| format!("{}({})", e.event_name(), e.payload()))
            .collect::<Vec<_>>()
    );
}

// ===========================================================================
// Cross-cutting: `list_commands_for_scope` emits `entity.add:{type}` for every
// `kind: grid` builtin view (section-5 regression guard).
//
// Iterates the REAL builtin view registry (not a hand-constructed list) and
// asserts — for every view whose `kind == grid` and whose `entity_type` is
// present — that `commands_for_scope` returns `entity.add:{entity_type}` in
// BOTH the palette path (`context_menu_only = false`) and the context-menu
// path (`context_menu_only = true`). Catches new grid views shipping without
// an `entity_type`, YAML drift that silently drops the type, or changes to
// `commands_for_scope` / `dedupe_by_id` / `check_available` that filter out
// the dynamic emission.
// ===========================================================================

/// Load the real builtin view registry (same path production uses) and
/// project the loaded `ViewDef`s onto the `ViewInfo` shape `gather_views`
/// assembles. Skips local views — we test the builtin invariant only.
fn load_builtin_view_infos() -> Vec<ViewInfo> {
    let builtin = builtin_view_definitions();
    let temp = TempDir::new().expect("tempdir");
    let vctx = ViewsContext::from_yaml_sources(temp.path().to_path_buf(), &builtin)
        .expect("builtin views must parse");
    vctx.all_views()
        .iter()
        .map(|v| ViewInfo {
            id: v.id.clone(),
            name: v.name.clone(),
            entity_type: v.entity_type.clone(),
        })
        .collect()
}

/// Same loader as `load_builtin_view_infos` but also returns each view's
/// declared `kind` so tests can filter to `kind: grid` only.
fn load_builtin_views_with_kind() -> Vec<(ViewInfo, ViewKind)> {
    let builtin = builtin_view_definitions();
    let temp = TempDir::new().expect("tempdir");
    let vctx = ViewsContext::from_yaml_sources(temp.path().to_path_buf(), &builtin)
        .expect("builtin views must parse");
    vctx.all_views()
        .iter()
        .map(|v| {
            (
                ViewInfo {
                    id: v.id.clone(),
                    name: v.name.clone(),
                    entity_type: v.entity_type.clone(),
                },
                v.kind.clone(),
            )
        })
        .collect()
}

/// Every builtin grid view that declares an `entity_type` must surface
/// `entity.add:{entity_type}` through `commands_for_scope`, in both the
/// palette (`context_menu_only = false`) and the context menu
/// (`context_menu_only = true`).
///
/// This is the regression guard the task calls out as MANDATORY in
/// section 5 — a new grid view shipped without `entity_type`, or any
/// future change that silently drops the `entity.add:*` emission, fails
/// this test as a single named failure.
#[tokio::test]
async fn list_commands_for_scope_emits_entity_add_for_every_grid_view() {
    let engine = TestEngine::new().await;
    let registry = CommandsRegistry::from_yaml_sources(&builtin_yaml_sources());
    let fields = engine.kanban.entity_context().await.unwrap();
    let views_with_kind = load_builtin_views_with_kind();
    let views: Vec<ViewInfo> = views_with_kind.iter().map(|(v, _)| v.clone()).collect();

    // Filter to kind: grid views that declare an entity_type. The task's
    // requirement is explicitly scoped to grids — the board view is
    // separately guarded above. If this list is ever empty the test fails
    // fast to prevent a "0 of 0 passed" vacuous green.
    let grids: Vec<&ViewInfo> = views_with_kind
        .iter()
        .filter(|(_, kind)| *kind == ViewKind::Grid)
        .filter(|(v, _)| v.entity_type.as_deref().is_some_and(|s| !s.is_empty()))
        .map(|(v, _)| v)
        .collect();
    assert!(
        !grids.is_empty(),
        "expected at least one builtin grid view with entity_type; got none. \
         Loaded views: {:?}",
        views
            .iter()
            .map(|v| (&v.name, &v.entity_type))
            .collect::<Vec<_>>()
    );

    for view in grids {
        let entity_type = view.entity_type.as_deref().unwrap();
        let expected_id = format!("entity.add:{entity_type}");
        let scope = vec![format!("view:{}", view.id), "board:my-board".into()];
        let dynamic = DynamicSources {
            views: views.clone(),
            ..Default::default()
        };

        // Palette path
        let palette = commands_for_scope(
            &scope,
            &registry,
            &engine.commands,
            Some(fields.fields()),
            &engine.ui_state,
            false,
            Some(&dynamic),
        );
        let palette_add = palette.iter().find(|c| c.id == expected_id);
        assert!(
            palette_add.is_some_and(|c| c.available),
            "palette (context_menu_only=false) must surface {expected_id} for grid view '{}' \
             (id={}); got command ids: {:?}",
            view.name,
            view.id,
            palette.iter().map(|c| &c.id).collect::<Vec<_>>()
        );

        // Context menu path
        let ctx_menu = commands_for_scope(
            &scope,
            &registry,
            &engine.commands,
            Some(fields.fields()),
            &engine.ui_state,
            true,
            Some(&dynamic),
        );
        let ctx_add = ctx_menu.iter().find(|c| c.id == expected_id);
        assert!(
            ctx_add.is_some_and(|c| c.available && c.context_menu),
            "context menu (context_menu_only=true) must surface {expected_id} for grid view \
             '{}' (id={}); got command ids: {:?}",
            view.name,
            view.id,
            ctx_menu.iter().map(|c| &c.id).collect::<Vec<_>>()
        );
    }
}

/// Reference: the helper that production's `kanban-app::gather_views`
/// projects on is `ViewDef` → `ViewInfo`. If the field set ever drifts
/// (e.g. `entity_type` is renamed), `load_builtin_view_infos` will fail
/// to compile — which is the signal the projection needs to be fixed in
/// lockstep with `gather_views`.
#[test]
fn load_builtin_view_infos_projects_entity_type() {
    let views = load_builtin_view_infos();
    let with_type: Vec<&ViewInfo> = views
        .iter()
        .filter(|v| v.entity_type.as_deref().is_some_and(|s| !s.is_empty()))
        .collect();
    assert!(
        with_type.len() >= 3,
        "expected at least 3 builtin views to declare entity_type; got {}: {:?}",
        with_type.len(),
        with_type
            .iter()
            .map(|v| (&v.name, &v.entity_type))
            .collect::<Vec<_>>()
    );
}
