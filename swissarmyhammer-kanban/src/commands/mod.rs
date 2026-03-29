//! Command trait implementations for kanban domain operations.
//!
//! Each submodule implements `Command` for a group of related operations.
//! The `register_commands()` function returns a map of command IDs to
//! trait objects, ready to be inserted into a `CommandsRegistry`.

pub mod app_commands;
pub mod clipboard_commands;
pub mod column_commands;
pub mod drag_commands;
pub mod entity_commands;
pub mod file_commands;
pub mod task_commands;
pub mod ui_commands;

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::KanbanOperationProcessor;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer_commands::{Command, CommandError};
use swissarmyhammer_operations::{Execute, OperationProcessor};

/// Run a kanban operation through the processor, mapping errors to `CommandError`.
pub(crate) async fn run_op<T>(
    op: &T,
    kanban: &KanbanContext,
) -> swissarmyhammer_commands::Result<Value>
where
    T: Execute<KanbanContext, KanbanError> + Send + Sync,
{
    KanbanOperationProcessor::new()
        .process(op, kanban)
        .await
        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))
}

/// Build the full map of kanban command implementations.
///
/// Returns command ID -> trait object pairs for all kanban domain commands.
pub fn register_commands() -> HashMap<String, Arc<dyn Command>> {
    let mut map: HashMap<String, Arc<dyn Command>> = HashMap::new();

    // Task commands
    map.insert("task.add".into(), Arc::new(task_commands::AddTaskCmd));
    map.insert("task.move".into(), Arc::new(task_commands::MoveTaskCmd));
    map.insert("task.untag".into(), Arc::new(task_commands::UntagTaskCmd));
    map.insert("task.delete".into(), Arc::new(task_commands::DeleteTaskCmd));

    // Entity commands
    map.insert(
        "entity.update_field".into(),
        Arc::new(entity_commands::UpdateEntityFieldCmd),
    );
    map.insert(
        "entity.delete".into(),
        Arc::new(entity_commands::DeleteEntityCmd),
    );
    map.insert(
        "entity.archive".into(),
        Arc::new(entity_commands::ArchiveEntityCmd),
    );
    map.insert(
        "entity.unarchive".into(),
        Arc::new(entity_commands::UnarchiveEntityCmd),
    );

    // Clipboard commands
    map.insert(
        "entity.paste".into(),
        Arc::new(entity_commands::PasteCmd),
    );

    // Tag commands
    map.insert("tag.update".into(), Arc::new(entity_commands::TagUpdateCmd));

    // Attachment commands
    map.insert(
        "attachment.delete".into(),
        Arc::new(entity_commands::AttachmentDeleteCmd),
    );

    // Clipboard commands
    map.insert(
        "entity.copy".into(),
        Arc::new(clipboard_commands::CopyCmd),
    );
    map.insert("entity.cut".into(), Arc::new(clipboard_commands::CutCmd));

    // Column commands
    map.insert(
        "column.reorder".into(),
        Arc::new(column_commands::ColumnReorderCmd),
    );

    // UI commands
    map.insert("ui.inspect".into(), Arc::new(ui_commands::InspectCmd));
    map.insert(
        "ui.inspector.close".into(),
        Arc::new(ui_commands::InspectorCloseCmd),
    );
    map.insert(
        "ui.inspector.close_all".into(),
        Arc::new(ui_commands::InspectorCloseAllCmd),
    );
    map.insert(
        "ui.palette.open".into(),
        Arc::new(ui_commands::PaletteOpenCmd),
    );
    map.insert(
        "ui.palette.close".into(),
        Arc::new(ui_commands::PaletteCloseCmd),
    );
    map.insert(
        "ui.view.set".into(),
        Arc::new(ui_commands::SetActiveViewCmd),
    );
    map.insert("ui.setFocus".into(), Arc::new(ui_commands::SetFocusCmd));

    // Drag session commands
    map.insert("drag.start".into(), Arc::new(drag_commands::DragStartCmd));
    map.insert("drag.cancel".into(), Arc::new(drag_commands::DragCancelCmd));
    map.insert(
        "drag.complete".into(),
        Arc::new(drag_commands::DragCompleteCmd),
    );

    // File / board management commands
    map.insert(
        "file.switchBoard".into(),
        Arc::new(file_commands::SwitchBoardCmd),
    );
    map.insert(
        "file.closeBoard".into(),
        Arc::new(file_commands::CloseBoardCmd),
    );

    // App commands
    map.insert("app.quit".into(), Arc::new(app_commands::QuitCmd));
    map.insert(
        "app.undo".into(),
        Arc::new(swissarmyhammer_entity::UndoCmd),
    );
    map.insert(
        "app.redo".into(),
        Arc::new(swissarmyhammer_entity::RedoCmd),
    );
    map.insert(
        "settings.keymap.vim".into(),
        Arc::new(app_commands::SetKeymapModeCmd("vim")),
    );
    map.insert(
        "settings.keymap.cua".into(),
        Arc::new(app_commands::SetKeymapModeCmd("cua")),
    );
    map.insert(
        "settings.keymap.emacs".into(),
        Arc::new(app_commands::SetKeymapModeCmd("emacs")),
    );

    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::sync::Arc;
    use swissarmyhammer_commands::{CommandContext, UIState};

    /// Build a CommandContext with the given scope chain, target, and optional UIState.
    fn ctx_with(scope: &[&str], target: Option<&str>, ui: Option<Arc<UIState>>) -> CommandContext {
        let mut ctx = CommandContext::new(
            "test",
            scope.iter().map(|s| s.to_string()).collect(),
            target.map(|s| s.to_string()),
            std::collections::HashMap::new(),
        );
        if let Some(ui) = ui {
            ctx.ui_state = Some(ui);
        }
        ctx
    }

    fn ctx_scope(scope: &[&str]) -> CommandContext {
        ctx_with(scope, None, None)
    }

    // =========================================================================
    // Registration sanity check
    // =========================================================================

    #[test]
    fn register_commands_returns_expected_count() {
        let cmds = register_commands();
        // 4 task + 4 entity + 3 clipboard + 1 tag + 1 attachment + 1 column + 7 UI + 6 app + 2 file + 3 drag = 32
        assert_eq!(cmds.len(), 32);
    }

    // =========================================================================
    // Availability tests — no disk I/O needed
    // =========================================================================

    #[test]
    fn add_task_available_with_column_in_scope() {
        let cmds = register_commands();
        let cmd = cmds.get("task.add").unwrap();
        let ctx = ctx_scope(&["column:todo", "board:board"]);
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn add_task_not_available_without_column() {
        let cmds = register_commands();
        let cmd = cmds.get("task.add").unwrap();
        let ctx = ctx_scope(&["board:board"]);
        assert!(!cmd.available(&ctx));
    }

    #[test]
    fn move_task_available_with_task_in_scope() {
        let cmds = register_commands();
        let cmd = cmds.get("task.move").unwrap();
        let ctx = ctx_scope(&["task:01ABC", "column:todo"]);
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn move_task_not_available_without_task() {
        let cmds = register_commands();
        let cmd = cmds.get("task.move").unwrap();
        let ctx = ctx_scope(&["column:todo"]);
        assert!(!cmd.available(&ctx));
    }

    #[test]
    fn untag_available_with_tag_and_task() {
        let cmds = register_commands();
        let cmd = cmds.get("task.untag").unwrap();
        let ctx = ctx_scope(&["tag:bug", "task:01ABC", "column:todo"]);
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn untag_not_available_without_tag() {
        let cmds = register_commands();
        let cmd = cmds.get("task.untag").unwrap();
        let ctx = ctx_scope(&["task:01ABC", "column:todo"]);
        assert!(!cmd.available(&ctx));
    }

    #[test]
    fn untag_not_available_without_task() {
        let cmds = register_commands();
        let cmd = cmds.get("task.untag").unwrap();
        let ctx = ctx_scope(&["tag:bug", "column:todo"]);
        assert!(!cmd.available(&ctx));
    }

    #[test]
    fn delete_task_available_with_task_in_scope() {
        let cmds = register_commands();
        let cmd = cmds.get("task.delete").unwrap();
        let ctx = ctx_scope(&["task:01ABC"]);
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn copy_available_with_task_in_scope() {
        let cmds = register_commands();
        let cmd = cmds.get("entity.copy").unwrap();
        let ctx = ctx_scope(&["task:01ABC", "column:todo"]);
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn copy_not_available_without_task() {
        let cmds = register_commands();
        let cmd = cmds.get("entity.copy").unwrap();
        let ctx = ctx_scope(&["column:todo"]);
        assert!(!cmd.available(&ctx));
    }

    #[test]
    fn cut_available_with_task_in_scope() {
        let cmds = register_commands();
        let cmd = cmds.get("entity.cut").unwrap();
        let ctx = ctx_scope(&["task:01ABC", "column:todo"]);
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn cut_not_available_without_task() {
        let cmds = register_commands();
        let cmd = cmds.get("entity.cut").unwrap();
        let ctx = ctx_scope(&["column:todo"]);
        assert!(!cmd.available(&ctx));
    }

    #[test]
    fn entity_delete_available_with_target() {
        let cmds = register_commands();
        let cmd = cmds.get("entity.delete").unwrap();
        let ctx = ctx_with(&[], Some("tag:01ABC"), None);
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn entity_delete_not_available_without_target() {
        let cmds = register_commands();
        let cmd = cmds.get("entity.delete").unwrap();
        let ctx = ctx_scope(&[]);
        assert!(!cmd.available(&ctx));
    }

    #[test]
    fn archive_entity_available_with_target() {
        let cmds = register_commands();
        let cmd = cmds.get("entity.archive").unwrap();
        let ctx = ctx_with(&[], Some("task:01ABC"), None);
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn archive_entity_not_available_without_target() {
        let cmds = register_commands();
        let cmd = cmds.get("entity.archive").unwrap();
        let ctx = ctx_scope(&[]);
        assert!(!cmd.available(&ctx));
    }

    #[test]
    fn unarchive_entity_available_with_target() {
        let cmds = register_commands();
        let cmd = cmds.get("entity.unarchive").unwrap();
        let ctx = ctx_with(&[], Some("task:01ABC"), None);
        assert!(cmd.available(&ctx));
    }

    // =========================================================================
    // Paste command availability tests
    // =========================================================================

    fn ui_with_clipboard() -> Arc<UIState> {
        let ui = Arc::new(UIState::new());
        ui.set_clipboard(swissarmyhammer_commands::ClipboardState {
            mode: swissarmyhammer_commands::ClipboardMode::Copy,
            entity_type: "task".into(),
            entity_id: "01TASK".into(),
            fields: serde_json::json!({"title": "Copied task"}),
        });
        ui
    }

    #[test]
    fn paste_available_with_clipboard_and_column() {
        let cmds = register_commands();
        let cmd = cmds.get("entity.paste").unwrap();
        let ui = ui_with_clipboard();
        let ctx = ctx_with(&["column:todo"], None, Some(ui));
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn paste_available_with_clipboard_and_board() {
        let cmds = register_commands();
        let cmd = cmds.get("entity.paste").unwrap();
        let ui = ui_with_clipboard();
        let ctx = ctx_with(&["board:my-board"], None, Some(ui));
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn paste_not_available_without_clipboard() {
        let cmds = register_commands();
        let cmd = cmds.get("entity.paste").unwrap();
        let ui = Arc::new(UIState::new());
        let ctx = ctx_with(&["column:todo"], None, Some(ui));
        assert!(!cmd.available(&ctx));
    }

    #[test]
    fn paste_not_available_without_column_or_board() {
        let cmds = register_commands();
        let cmd = cmds.get("entity.paste").unwrap();
        let ui = ui_with_clipboard();
        let ctx = ctx_with(&[], None, Some(ui));
        assert!(!cmd.available(&ctx));
    }

    // =========================================================================
    // UI command tests — use in-memory UIState, no disk I/O
    // =========================================================================

    #[test]
    fn inspect_available_with_target() {
        let cmds = register_commands();
        let cmd = cmds.get("ui.inspect").unwrap();
        let ctx = ctx_with(&[], Some("task:01ABC"), None);
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn inspect_available_with_scope() {
        let cmds = register_commands();
        let cmd = cmds.get("ui.inspect").unwrap();
        let ctx = ctx_scope(&["task:01ABC"]);
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn inspect_not_available_without_target_or_scope() {
        let cmds = register_commands();
        let cmd = cmds.get("ui.inspect").unwrap();
        let ctx = ctx_scope(&[]);
        assert!(!cmd.available(&ctx));
    }

    #[tokio::test]
    async fn inspect_executes_updates_ui_state() {
        let cmds = register_commands();
        let cmd = cmds.get("ui.inspect").unwrap();
        let ui = Arc::new(UIState::new());
        let ctx = ctx_with(&[], Some("task:01XYZ"), Some(Arc::clone(&ui)));

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        // ctx has no window_label set, so falls back to "main"
        assert_eq!(ui.inspector_stack("main"), vec!["task:01XYZ"]);
    }

    #[tokio::test]
    async fn inspector_close_executes() {
        let cmds = register_commands();
        let cmd = cmds.get("ui.inspector.close").unwrap();
        let ui = Arc::new(UIState::new());
        ui.inspect("main", "task:01XYZ");
        ui.inspect("main", "tag:01TAG");

        let ctx = ctx_with(&[], None, Some(Arc::clone(&ui)));
        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        assert_eq!(ui.inspector_stack("main"), vec!["task:01XYZ"]);
    }

    #[tokio::test]
    async fn inspector_close_all_executes() {
        let cmds = register_commands();
        let cmd = cmds.get("ui.inspector.close_all").unwrap();
        let ui = Arc::new(UIState::new());
        ui.inspect("main", "task:01XYZ");
        ui.inspect("main", "tag:01TAG");

        let ctx = ctx_with(&[], None, Some(Arc::clone(&ui)));
        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        assert!(ui.inspector_stack("main").is_empty());
    }

    #[tokio::test]
    async fn palette_open_executes() {
        let cmds = register_commands();
        let cmd = cmds.get("ui.palette.open").unwrap();
        let ui = Arc::new(UIState::new());
        assert!(!ui.palette_open());

        let ctx = ctx_with(&[], None, Some(Arc::clone(&ui)));
        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        assert!(ui.palette_open());
    }

    #[tokio::test]
    async fn palette_close_executes() {
        let cmds = register_commands();
        let cmd = cmds.get("ui.palette.close").unwrap();
        let ui = Arc::new(UIState::new());
        ui.set_palette_open(true);

        let ctx = ctx_with(&[], None, Some(Arc::clone(&ui)));
        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        assert!(!ui.palette_open());
    }

    #[tokio::test]
    async fn set_keymap_mode_executes() {
        let cmds = register_commands();
        let cmd = cmds.get("settings.keymap.vim").unwrap();
        let ui = Arc::new(UIState::new());
        assert_eq!(ui.keymap_mode(), "cua");

        let ctx = ctx_with(&[], None, Some(Arc::clone(&ui)));
        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        assert_eq!(ui.keymap_mode(), "vim");
    }

    #[tokio::test]
    async fn set_focus_cmd_sets_scope_chain() {
        let cmds = register_commands();
        let cmd = cmds.get("ui.setFocus").unwrap();
        let ui = Arc::new(UIState::new());
        assert!(ui.scope_chain().is_empty());

        let mut args = std::collections::HashMap::new();
        args.insert(
            "scope_chain".to_string(),
            serde_json::json!(["task:01XYZ", "column:todo"]),
        );
        let mut ctx = CommandContext::new("ui.setFocus", vec![], None, args);
        ctx.ui_state = Some(Arc::clone(&ui));

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        assert_eq!(ui.scope_chain(), vec!["task:01XYZ", "column:todo"]);
    }

    #[tokio::test]
    async fn set_focus_cmd_clears_scope_chain_with_empty_arg() {
        let cmds = register_commands();
        let cmd = cmds.get("ui.setFocus").unwrap();
        let ui = Arc::new(UIState::new());
        ui.set_scope_chain(vec!["task:01XYZ".to_string()]);

        let mut args = std::collections::HashMap::new();
        args.insert("scope_chain".to_string(), serde_json::json!([]));
        let mut ctx = CommandContext::new("ui.setFocus", vec![], None, args);
        ctx.ui_state = Some(Arc::clone(&ui));

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        assert!(ui.scope_chain().is_empty());
    }

    #[tokio::test]
    async fn set_focus_cmd_defaults_to_empty_when_no_arg() {
        let cmds = register_commands();
        let cmd = cmds.get("ui.setFocus").unwrap();
        let ui = Arc::new(UIState::new());
        ui.set_scope_chain(vec!["task:01XYZ".to_string()]);

        // No scope_chain arg — should default to empty
        let ctx_empty: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
        let mut ctx = CommandContext::new("ui.setFocus", vec![], None, ctx_empty);
        ctx.ui_state = Some(Arc::clone(&ui));

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        assert!(ui.scope_chain().is_empty());
    }

    #[tokio::test]
    async fn set_active_view_executes() {
        let cmds = register_commands();
        let cmd = cmds.get("ui.view.set").unwrap();
        let ui = Arc::new(UIState::new());

        let mut args = std::collections::HashMap::new();
        args.insert("view_id".to_string(), Value::String("my-view".into()));
        let mut ctx = CommandContext::new("test", vec![], None, args);
        ctx.ui_state = Some(Arc::clone(&ui));

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        // No window_label in ctx — defaults to "main"
        assert_eq!(ui.active_view_id("main"), "my-view");
    }

    // =========================================================================
    // Quit command tests
    // =========================================================================

    #[test]
    fn quit_always_available() {
        let cmds = register_commands();
        let cmd = cmds.get("app.quit").unwrap();
        // Available with empty scope
        assert!(cmd.available(&ctx_scope(&[])));
        // Available with any scope
        assert!(cmd.available(&ctx_scope(&["task:01ABC", "column:todo"])));
    }

    #[tokio::test]
    async fn quit_executes_returns_quit_true() {
        let cmds = register_commands();
        let cmd = cmds.get("app.quit").unwrap();
        let ctx = ctx_scope(&[]);
        let result = cmd.execute(&ctx).await.unwrap();
        assert_eq!(result["quit"], true);
    }

    // =========================================================================
    // Undo/Redo availability tests
    // =========================================================================

    #[test]
    fn undo_always_available() {
        let cmds = register_commands();
        let cmd = cmds.get("app.undo").unwrap();
        assert!(cmd.available(&ctx_scope(&[])));
    }

    #[test]
    fn redo_always_available() {
        let cmds = register_commands();
        let cmd = cmds.get("app.redo").unwrap();
        assert!(cmd.available(&ctx_scope(&[])));
    }

    // =========================================================================
    // Integration test: dispatch through registry
    // =========================================================================

    #[tokio::test]
    async fn integration_registry_dispatch_ui_command() {
        // Simulate the full dispatch path: lookup command by ID, check available,
        // then execute. This verifies that register_commands() produces working
        // trait objects that the dispatcher can drive.
        let cmds = register_commands();
        let ui = Arc::new(UIState::new());

        // Dispatch ui.inspect with a target
        let cmd = cmds.get("ui.inspect").unwrap();
        let ctx = ctx_with(&["task:01ABC"], Some("task:01ABC"), Some(Arc::clone(&ui)));

        assert!(cmd.available(&ctx), "inspect should be available");
        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok(), "inspect should succeed");
        // ctx has no window_label set, so falls back to "main"
        assert_eq!(ui.inspector_stack("main"), vec!["task:01ABC"]);

        // Dispatch ui.inspector.close
        let cmd = cmds.get("ui.inspector.close").unwrap();
        let ctx = ctx_with(&[], None, Some(Arc::clone(&ui)));
        assert!(cmd.available(&ctx));
        cmd.execute(&ctx).await.unwrap();
        assert!(ui.inspector_stack("main").is_empty());

        // Dispatch settings.keymap.vim
        let cmd = cmds.get("settings.keymap.vim").unwrap();
        let ctx = ctx_with(&[], None, Some(Arc::clone(&ui)));
        assert!(cmd.available(&ctx));
        cmd.execute(&ctx).await.unwrap();
        assert_eq!(ui.keymap_mode(), "vim");
    }

    // =========================================================================
    // Drag command tests
    // =========================================================================

    #[tokio::test]
    async fn drag_start_cmd_stores_session() {
        let cmds = register_commands();
        let cmd = cmds.get("drag.start").unwrap();
        let ui = Arc::new(UIState::new());
        assert!(ui.drag_session().is_none());

        let mut args = std::collections::HashMap::new();
        args.insert("boardPath".into(), serde_json::json!("/boards/a/.kanban"));
        args.insert("taskId".into(), serde_json::json!("task-123"));
        args.insert("taskFields".into(), serde_json::json!({"title": "My Task"}));
        let mut ctx = CommandContext::new("drag.start", vec![], None, args);
        ctx.ui_state = Some(Arc::clone(&ui));

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok(), "drag.start should succeed: {:?}", result);

        let session = ui.drag_session().expect("session should be stored");
        assert_eq!(session.task_id, "task-123");
        assert_eq!(session.source_board_path, "/boards/a/.kanban");
        assert_eq!(session.source_window_label, "main");
        assert!(!session.copy_mode);
    }

    #[tokio::test]
    async fn drag_start_cmd_returns_drag_start_result() {
        let cmds = register_commands();
        let cmd = cmds.get("drag.start").unwrap();
        let ui = Arc::new(UIState::new());

        let mut args = std::collections::HashMap::new();
        args.insert("boardPath".into(), serde_json::json!("/boards/b/.kanban"));
        args.insert("taskId".into(), serde_json::json!("task-456"));
        let mut ctx = CommandContext::new("drag.start", vec![], None, args);
        ctx.ui_state = Some(Arc::clone(&ui));

        let result = cmd.execute(&ctx).await.unwrap();
        assert!(
            result.get("DragStart").is_some(),
            "result must have DragStart key"
        );
        let drag_start = result.get("DragStart").unwrap();
        assert_eq!(drag_start["task_id"].as_str().unwrap(), "task-456");
        assert_eq!(
            drag_start["source_board_path"].as_str().unwrap(),
            "/boards/b/.kanban"
        );
    }

    #[tokio::test]
    async fn drag_start_cmd_replaces_existing_session() {
        let cmds = register_commands();
        let cmd = cmds.get("drag.start").unwrap();
        let ui = Arc::new(UIState::new());

        // Start first session
        let mut args1 = std::collections::HashMap::new();
        args1.insert("boardPath".into(), serde_json::json!("/boards/a"));
        args1.insert("taskId".into(), serde_json::json!("task-1"));
        let mut ctx1 = CommandContext::new("drag.start", vec![], None, args1);
        ctx1.ui_state = Some(Arc::clone(&ui));
        cmd.execute(&ctx1).await.unwrap();

        // Start second session — should replace
        let mut args2 = std::collections::HashMap::new();
        args2.insert("boardPath".into(), serde_json::json!("/boards/b"));
        args2.insert("taskId".into(), serde_json::json!("task-2"));
        let mut ctx2 = CommandContext::new("drag.start", vec![], None, args2);
        ctx2.ui_state = Some(Arc::clone(&ui));
        cmd.execute(&ctx2).await.unwrap();

        let session = ui.drag_session().unwrap();
        assert_eq!(session.task_id, "task-2");
        assert_eq!(session.source_board_path, "/boards/b");
    }

    #[tokio::test]
    async fn drag_start_cmd_missing_task_id_returns_error() {
        let cmds = register_commands();
        let cmd = cmds.get("drag.start").unwrap();
        let ui = Arc::new(UIState::new());

        let mut args = std::collections::HashMap::new();
        args.insert("boardPath".into(), serde_json::json!("/boards/a"));
        // taskId intentionally omitted
        let mut ctx = CommandContext::new("drag.start", vec![], None, args);
        ctx.ui_state = Some(Arc::clone(&ui));

        let result = cmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without taskId");
    }

    #[tokio::test]
    async fn drag_start_cmd_copy_mode_defaults_to_false() {
        let cmds = register_commands();
        let cmd = cmds.get("drag.start").unwrap();
        let ui = Arc::new(UIState::new());

        let mut args = std::collections::HashMap::new();
        args.insert("boardPath".into(), serde_json::json!("/boards/a"));
        args.insert("taskId".into(), serde_json::json!("task-1"));
        // copyMode not provided
        let mut ctx = CommandContext::new("drag.start", vec![], None, args);
        ctx.ui_state = Some(Arc::clone(&ui));

        cmd.execute(&ctx).await.unwrap();
        let session = ui.drag_session().unwrap();
        assert!(!session.copy_mode);
    }

    // =========================================================================
    // Drag cancel command tests
    // =========================================================================

    #[tokio::test]
    async fn drag_cancel_cmd_clears_session() {
        let cmds = register_commands();
        let cmd = cmds.get("drag.cancel").unwrap();
        let ui = Arc::new(UIState::new());

        // Start a session first via drag.start
        let start_cmd = cmds.get("drag.start").unwrap();
        let mut start_args = std::collections::HashMap::new();
        start_args.insert("boardPath".into(), serde_json::json!("/boards/a/.kanban"));
        start_args.insert("taskId".into(), serde_json::json!("task-999"));
        let mut start_ctx = CommandContext::new("drag.start", vec![], None, start_args);
        start_ctx.ui_state = Some(Arc::clone(&ui));
        start_cmd.execute(&start_ctx).await.unwrap();
        assert!(ui.drag_session().is_some(), "session should be active");

        // Now cancel it
        let mut ctx = CommandContext::new(
            "drag.cancel",
            vec![],
            None,
            std::collections::HashMap::new(),
        );
        ctx.ui_state = Some(Arc::clone(&ui));
        let result = cmd.execute(&ctx).await.unwrap();

        assert!(ui.drag_session().is_none(), "session should be cleared");
        assert!(
            result.get("DragCancel").is_some(),
            "result must have DragCancel key"
        );
        let drag_cancel = result.get("DragCancel").unwrap();
        assert!(
            drag_cancel.get("session_id").is_some(),
            "DragCancel must contain session_id"
        );
    }

    #[tokio::test]
    async fn drag_cancel_cmd_no_session_returns_null() {
        let cmds = register_commands();
        let cmd = cmds.get("drag.cancel").unwrap();
        let ui = Arc::new(UIState::new());
        assert!(ui.drag_session().is_none(), "no session should be active");

        let mut ctx = CommandContext::new(
            "drag.cancel",
            vec![],
            None,
            std::collections::HashMap::new(),
        );
        ctx.ui_state = Some(Arc::clone(&ui));
        let result = cmd.execute(&ctx).await.unwrap();

        assert!(
            result.is_null(),
            "should return null when no session active"
        );
    }

    #[tokio::test]
    async fn drag_start_cmd_copy_mode_can_be_set() {
        let cmds = register_commands();
        let cmd = cmds.get("drag.start").unwrap();
        let ui = Arc::new(UIState::new());

        let mut args = std::collections::HashMap::new();
        args.insert("boardPath".into(), serde_json::json!("/boards/a"));
        args.insert("taskId".into(), serde_json::json!("task-1"));
        args.insert("copyMode".into(), serde_json::json!(true));
        let mut ctx = CommandContext::new("drag.start", vec![], None, args);
        ctx.ui_state = Some(Arc::clone(&ui));

        cmd.execute(&ctx).await.unwrap();
        let session = ui.drag_session().unwrap();
        assert!(session.copy_mode);
    }
}
