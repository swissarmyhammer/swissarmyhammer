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
pub mod perspective_commands;
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
    map.insert(
        "task.doThisNext".into(),
        Arc::new(task_commands::DoThisNextCmd),
    );
    map.insert("task.delete".into(), Arc::new(task_commands::DeleteTaskCmd));

    // Clipboard commands
    map.insert(
        "entity.copy".into(),
        Arc::new(clipboard_commands::CopyTaskCmd),
    );
    map.insert(
        "entity.cut".into(),
        Arc::new(clipboard_commands::CutTaskCmd),
    );
    map.insert(
        "entity.paste".into(),
        Arc::new(clipboard_commands::PasteTaskCmd),
    );

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

    // Tag commands
    map.insert("tag.update".into(), Arc::new(entity_commands::TagUpdateCmd));

    // Attachment file commands
    map.insert(
        "attachment.open".into(),
        Arc::new(entity_commands::AttachmentOpenCmd),
    );
    map.insert(
        "attachment.reveal".into(),
        Arc::new(entity_commands::AttachmentRevealCmd),
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
    map.insert(
        "ui.perspective.set".into(),
        Arc::new(ui_commands::SetActivePerspectiveCmd),
    );
    map.insert("ui.setFocus".into(), Arc::new(ui_commands::SetFocusCmd));
    map.insert("ui.mode.set".into(), Arc::new(ui_commands::SetAppModeCmd));

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
    map.insert("file.newBoard".into(), Arc::new(file_commands::NewBoardCmd));
    map.insert(
        "file.openBoard".into(),
        Arc::new(file_commands::OpenBoardCmd),
    );
    map.insert("window.new".into(), Arc::new(file_commands::NewWindowCmd));

    // Perspective commands
    map.insert(
        "perspective.load".into(),
        Arc::new(perspective_commands::LoadPerspectiveCmd),
    );
    map.insert(
        "perspective.save".into(),
        Arc::new(perspective_commands::SavePerspectiveCmd),
    );
    map.insert(
        "perspective.delete".into(),
        Arc::new(perspective_commands::DeletePerspectiveCmd),
    );
    map.insert(
        "perspective.filter".into(),
        Arc::new(perspective_commands::SetFilterCmd),
    );
    map.insert(
        "perspective.clearFilter".into(),
        Arc::new(perspective_commands::ClearFilterCmd),
    );
    map.insert(
        "perspective.group".into(),
        Arc::new(perspective_commands::SetGroupCmd),
    );
    map.insert(
        "perspective.clearGroup".into(),
        Arc::new(perspective_commands::ClearGroupCmd),
    );
    map.insert(
        "perspective.list".into(),
        Arc::new(perspective_commands::ListPerspectivesCmd),
    );
    map.insert(
        "perspective.sort.set".into(),
        Arc::new(perspective_commands::SetSortCmd),
    );
    map.insert(
        "perspective.sort.clear".into(),
        Arc::new(perspective_commands::ClearSortCmd),
    );
    map.insert(
        "perspective.sort.toggle".into(),
        Arc::new(perspective_commands::ToggleSortCmd),
    );

    // App commands
    map.insert("app.quit".into(), Arc::new(app_commands::QuitCmd));
    map.insert("app.about".into(), Arc::new(app_commands::AboutCmd));
    map.insert("app.help".into(), Arc::new(app_commands::HelpCmd));
    map.insert(
        "app.command".into(),
        Arc::new(app_commands::CommandPaletteCmd),
    );
    // app.palette is an alias for app.command — both open the command palette.
    map.insert(
        "app.palette".into(),
        Arc::new(app_commands::CommandPaletteCmd),
    );
    map.insert(
        "app.search".into(),
        Arc::new(app_commands::SearchPaletteCmd),
    );
    map.insert("app.dismiss".into(), Arc::new(app_commands::DismissCmd));
    map.insert("app.undo".into(), Arc::new(swissarmyhammer_entity::UndoCmd));
    map.insert("app.redo".into(), Arc::new(swissarmyhammer_entity::RedoCmd));
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
        // 5 task (add, move, untag, doThisNext, delete) + 3 clipboard
        // + 4 entity + 1 tag + 1 column + 8 UI
        // + 12 app (quit, about, help, command, palette, search,
        //          dismiss, undo, redo, keymap.vim, keymap.cua, keymap.emacs)
        // + 5 file (switchBoard, closeBoard, newBoard, openBoard, window.new)
        // + 3 drag + 11 perspective (8 + 3 sort) + 2 attachment (open, reveal) = 55
        // + 1 ui.mode.set = 56
        // Note: clipboard entries are duplicated in the source but HashMap deduplicates.
        assert_eq!(cmds.len(), 56);
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
    fn do_this_next_available_with_task_in_scope() {
        let cmds = register_commands();
        let cmd = cmds.get("task.doThisNext").unwrap();
        let ctx = ctx_scope(&["task:01ABC", "column:doing"]);
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn do_this_next_not_available_without_task() {
        let cmds = register_commands();
        let cmd = cmds.get("task.doThisNext").unwrap();
        let ctx = ctx_scope(&["column:doing"]);
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

    fn ui_with_task_clipboard() -> Arc<UIState> {
        let ui = Arc::new(UIState::new());
        ui.set_clipboard_entity_type("task");
        ui
    }

    fn ui_with_tag_clipboard() -> Arc<UIState> {
        let ui = Arc::new(UIState::new());
        ui.set_clipboard_entity_type("tag");
        ui
    }

    #[test]
    fn paste_task_available_with_column() {
        let cmds = register_commands();
        let cmd = cmds.get("entity.paste").unwrap();
        let ui = ui_with_task_clipboard();
        let ctx = ctx_with(&["column:todo"], None, Some(ui));
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn paste_task_available_with_board() {
        let cmds = register_commands();
        let cmd = cmds.get("entity.paste").unwrap();
        let ui = ui_with_task_clipboard();
        let ctx = ctx_with(&["board:my-board"], None, Some(ui));
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn paste_tag_available_with_task() {
        let cmds = register_commands();
        let cmd = cmds.get("entity.paste").unwrap();
        let ui = ui_with_tag_clipboard();
        let ctx = ctx_with(&["task:01X", "column:todo"], None, Some(ui));
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn paste_tag_not_available_on_column() {
        let cmds = register_commands();
        let cmd = cmds.get("entity.paste").unwrap();
        let ui = ui_with_tag_clipboard();
        let ctx = ctx_with(&["column:todo"], None, Some(ui));
        assert!(!cmd.available(&ctx));
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
    fn paste_not_available_without_scope() {
        let cmds = register_commands();
        let cmd = cmds.get("entity.paste").unwrap();
        let ui = ui_with_task_clipboard();
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
        assert!(!ui.palette_open("main"));

        let ctx = ctx_with(&[], None, Some(Arc::clone(&ui)));
        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        assert!(ui.palette_open("main"));
    }

    #[tokio::test]
    async fn palette_close_executes() {
        let cmds = register_commands();
        let cmd = cmds.get("ui.palette.close").unwrap();
        let ui = Arc::new(UIState::new());
        ui.set_palette_open("main", true);

        let ctx = ctx_with(&[], None, Some(Arc::clone(&ui)));
        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        assert!(!ui.palette_open("main"));
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
    // About, Help, ResetWindows, Palette, Search, Dismiss command tests
    // =========================================================================

    #[test]
    fn about_always_available() {
        let cmds = register_commands();
        let cmd = cmds.get("app.about").unwrap();
        assert!(cmd.available(&ctx_scope(&[])));
    }

    #[tokio::test]
    async fn about_returns_marker() {
        let cmds = register_commands();
        let cmd = cmds.get("app.about").unwrap();
        let result = cmd.execute(&ctx_scope(&[])).await.unwrap();
        assert_eq!(result["about"], true);
    }

    #[test]
    fn help_always_available() {
        let cmds = register_commands();
        let cmd = cmds.get("app.help").unwrap();
        assert!(cmd.available(&ctx_scope(&[])));
    }

    #[tokio::test]
    async fn help_returns_marker() {
        let cmds = register_commands();
        let cmd = cmds.get("app.help").unwrap();
        let result = cmd.execute(&ctx_scope(&[])).await.unwrap();
        assert_eq!(result["help"], true);
    }

    #[test]
    fn command_palette_always_available() {
        let cmds = register_commands();
        let cmd = cmds.get("app.command").unwrap();
        assert!(cmd.available(&ctx_scope(&[])));
    }

    #[tokio::test]
    async fn command_palette_opens_palette_in_command_mode() {
        let cmds = register_commands();
        let cmd = cmds.get("app.command").unwrap();
        let ui = Arc::new(UIState::new());
        assert!(!ui.palette_open("main"));

        let ctx = ctx_with(&[], None, Some(Arc::clone(&ui)));
        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        assert!(ui.palette_open("main"));
        assert_eq!(ui.palette_mode("main"), "command");
    }

    #[tokio::test]
    async fn search_palette_opens_palette_in_search_mode() {
        let cmds = register_commands();
        let cmd = cmds.get("app.search").unwrap();
        let ui = Arc::new(UIState::new());
        assert!(!ui.palette_open("main"));

        let ctx = ctx_with(&[], None, Some(Arc::clone(&ui)));
        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        assert!(ui.palette_open("main"));
        assert_eq!(ui.palette_mode("main"), "search");
    }

    #[tokio::test]
    async fn command_palette_targets_invoking_window_only() {
        let cmds = register_commands();
        let cmd = cmds.get("app.command").unwrap();
        let ui = Arc::new(UIState::new());

        // Execute with scope chain containing window:secondary
        let ctx = ctx_with(&["window:secondary"], None, Some(Arc::clone(&ui)));
        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        // Only secondary window should have palette open
        assert!(ui.palette_open("secondary"));
        assert!(!ui.palette_open("main"));
    }

    #[test]
    fn dismiss_always_available() {
        let cmds = register_commands();
        let cmd = cmds.get("app.dismiss").unwrap();
        assert!(cmd.available(&ctx_scope(&[])));
    }

    #[tokio::test]
    async fn dismiss_closes_palette_when_open() {
        let cmds = register_commands();
        let cmd = cmds.get("app.dismiss").unwrap();
        let ui = Arc::new(UIState::new());
        ui.set_palette_open("main", true);

        let ctx = ctx_with(&[], None, Some(Arc::clone(&ui)));
        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        assert!(!ui.palette_open("main"));
    }

    #[tokio::test]
    async fn dismiss_closes_inspector_when_palette_closed() {
        let cmds = register_commands();
        let cmd = cmds.get("app.dismiss").unwrap();
        let ui = Arc::new(UIState::new());
        ui.inspect("main", "task:01XYZ");
        assert!(!ui.palette_open("main"));
        assert_eq!(ui.inspector_stack("main").len(), 1);

        let ctx = ctx_with(&[], None, Some(Arc::clone(&ui)));
        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        assert!(ui.inspector_stack("main").is_empty());
    }

    #[tokio::test]
    async fn dismiss_returns_null_when_nothing_to_dismiss() {
        let cmds = register_commands();
        let cmd = cmds.get("app.dismiss").unwrap();
        let ui = Arc::new(UIState::new());

        let ctx = ctx_with(&[], None, Some(Arc::clone(&ui)));
        let result = cmd.execute(&ctx).await.unwrap();
        assert!(result.is_null());
    }

    // =========================================================================
    // Undo/Redo availability tests
    // =========================================================================

    #[test]
    fn undo_unavailable_without_ui_state() {
        let cmds = register_commands();
        let cmd = cmds.get("app.undo").unwrap();
        // No UIState on context — undo should not be available
        assert!(!cmd.available(&ctx_scope(&[])));
    }

    #[test]
    fn redo_unavailable_without_ui_state() {
        let cmds = register_commands();
        let cmd = cmds.get("app.redo").unwrap();
        // No UIState on context — redo should not be available
        assert!(!cmd.available(&ctx_scope(&[])));
    }

    #[test]
    fn undo_unavailable_when_stack_empty() {
        let cmds = register_commands();
        let cmd = cmds.get("app.undo").unwrap();
        let ui = Arc::new(UIState::new());
        // UIState present but can_undo defaults to false
        assert!(!cmd.available(&ctx_with(&[], None, Some(ui))));
    }

    #[test]
    fn undo_available_when_can_undo_set() {
        let cmds = register_commands();
        let cmd = cmds.get("app.undo").unwrap();
        let ui = Arc::new(UIState::new());
        ui.set_undo_redo_state(true, false);
        assert!(cmd.available(&ctx_with(&[], None, Some(ui))));
    }

    #[test]
    fn redo_available_when_can_redo_set() {
        let cmds = register_commands();
        let cmd = cmds.get("app.redo").unwrap();
        let ui = Arc::new(UIState::new());
        ui.set_undo_redo_state(false, true);
        assert!(cmd.available(&ctx_with(&[], None, Some(ui))));
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
