//! Application-level command implementations: undo, redo, quit, keymap mode.

use crate::context::KanbanContext;
use async_trait::async_trait;
use serde_json::{json, Value};
use swissarmyhammer_commands::{Command, CommandContext, CommandError};

/// Undo the last operation by transaction ID.
///
/// Always available. Required arg: `id` (transaction ULID).
pub struct UndoCmd;

#[async_trait]
impl Command for UndoCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let id = ctx.require_arg_str("id")?;

        let ectx = kanban
            .entity_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        let result_ulid = ectx
            .undo(id)
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        Ok(json!({ "undone": id, "operation_id": result_ulid }))
    }
}

/// Redo a previously undone operation by transaction ID.
///
/// Always available. Required arg: `id` (transaction ULID).
pub struct RedoCmd;

#[async_trait]
impl Command for RedoCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let id = ctx.require_arg_str("id")?;

        let ectx = kanban
            .entity_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        let result_ulid = ectx
            .redo(id)
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        Ok(json!({ "redone": id, "operation_id": result_ulid }))
    }
}

/// Set the keymap mode to a fixed value (vim, cua, emacs).
///
/// Each keymap mode has its own command instance with the mode baked in.
/// Always available.
pub struct SetKeymapModeCmd(pub &'static str);

#[async_trait]
impl Command for SetKeymapModeCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        let change = ui.set_keymap_mode(self.0);
        Ok(serde_json::to_value(change).unwrap_or(Value::Null))
    }
}

/// Quit the application.
///
/// Always available. Execution is a no-op on the backend — the frontend
/// (Tauri layer) handles the actual window close / process exit when it
/// receives the command result.
pub struct QuitCmd;

#[async_trait]
impl Command for QuitCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, _ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        Ok(json!({ "quit": true }))
    }
}
