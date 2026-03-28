//! Application-level command implementations: quit, keymap mode.

use async_trait::async_trait;
use serde_json::{json, Value};
use swissarmyhammer_commands::{Command, CommandContext, CommandError};

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
