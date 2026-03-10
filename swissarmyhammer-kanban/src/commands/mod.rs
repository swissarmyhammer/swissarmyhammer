//! Command trait implementations for kanban domain operations.
//!
//! Each submodule implements `Command` for a group of related operations.
//! The `register_commands()` function returns a map of command IDs to
//! trait objects, ready to be inserted into a `CommandsRegistry`.

pub mod app_commands;
pub mod column_commands;
pub mod entity_commands;
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
    map.insert("task.tag".into(), Arc::new(task_commands::TagTaskCmd));
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

    // Tag commands
    map.insert("tag.update".into(), Arc::new(entity_commands::TagUpdateCmd));

    // Attachment commands
    map.insert(
        "attachment.delete".into(),
        Arc::new(entity_commands::AttachmentDeleteCmd),
    );

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

    // App commands
    map.insert("app.quit".into(), Arc::new(app_commands::QuitCmd));
    map.insert("app.undo".into(), Arc::new(app_commands::UndoCmd));
    map.insert("app.redo".into(), Arc::new(app_commands::RedoCmd));
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
        // 5 task + 2 entity + 1 tag + 1 attachment + 1 column + 6 UI + 6 app = 22
        assert_eq!(cmds.len(), 22);
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
    fn tag_available_with_tag_and_task() {
        let cmds = register_commands();
        let cmd = cmds.get("task.tag").unwrap();
        let ctx = ctx_scope(&["tag:bug", "task:01ABC", "column:todo"]);
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn tag_not_available_without_tag() {
        let cmds = register_commands();
        let cmd = cmds.get("task.tag").unwrap();
        let ctx = ctx_scope(&["task:01ABC", "column:todo"]);
        assert!(!cmd.available(&ctx));
    }

    #[test]
    fn tag_not_available_without_task() {
        let cmds = register_commands();
        let cmd = cmds.get("task.tag").unwrap();
        let ctx = ctx_scope(&["tag:bug", "column:todo"]);
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
        assert_eq!(ui.inspector_stack(), vec!["task:01XYZ"]);
    }

    #[tokio::test]
    async fn inspector_close_executes() {
        let cmds = register_commands();
        let cmd = cmds.get("ui.inspector.close").unwrap();
        let ui = Arc::new(UIState::new());
        ui.inspect("task:01XYZ");
        ui.inspect("tag:01TAG");

        let ctx = ctx_with(&[], None, Some(Arc::clone(&ui)));
        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        assert_eq!(ui.inspector_stack(), vec!["task:01XYZ"]);
    }

    #[tokio::test]
    async fn inspector_close_all_executes() {
        let cmds = register_commands();
        let cmd = cmds.get("ui.inspector.close_all").unwrap();
        let ui = Arc::new(UIState::new());
        ui.inspect("task:01XYZ");
        ui.inspect("tag:01TAG");

        let ctx = ctx_with(&[], None, Some(Arc::clone(&ui)));
        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        assert!(ui.inspector_stack().is_empty());
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
        assert_eq!(ui.active_view_id(), "my-view");
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
        assert_eq!(ui.inspector_stack(), vec!["task:01ABC"]);

        // Dispatch ui.inspector.close
        let cmd = cmds.get("ui.inspector.close").unwrap();
        let ctx = ctx_with(&[], None, Some(Arc::clone(&ui)));
        assert!(cmd.available(&ctx));
        cmd.execute(&ctx).await.unwrap();
        assert!(ui.inspector_stack().is_empty());

        // Dispatch settings.keymap.vim
        let cmd = cmds.get("settings.keymap.vim").unwrap();
        let ctx = ctx_with(&[], None, Some(Arc::clone(&ui)));
        assert!(cmd.available(&ctx));
        cmd.execute(&ctx).await.unwrap();
        assert_eq!(ui.keymap_mode(), "vim");
    }
}
