//! Application-level command implementations: quit, keymap mode, about, help,
//! reset windows, dismiss, command palette, search palette.

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

/// About — no-op placeholder.
///
/// Always available. Returns a no-op result.
pub struct AboutCmd;

#[async_trait]
impl Command for AboutCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, _ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        Ok(json!({ "about": true }))
    }
}

/// Help — no-op placeholder.
///
/// Always available. Returns a no-op result.
pub struct HelpCmd;

#[async_trait]
impl Command for HelpCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, _ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        Ok(json!({ "help": true }))
    }
}

/// Open the command palette in "command" mode.
///
/// Always available. Sets `palette_open = true` and `palette_mode = "command"`.
pub struct CommandPaletteCmd;

#[async_trait]
impl Command for CommandPaletteCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        let window_label = ctx.window_label_from_scope().unwrap_or("main");
        let change = ui.set_palette_open_with_mode(window_label, true, "command");
        Ok(serde_json::to_value(change).unwrap_or(Value::Null))
    }
}

/// Open the command palette in "search" mode.
///
/// Always available. Sets `palette_open = true` and `palette_mode = "search"`
/// for the invoking window only.
pub struct SearchPaletteCmd;

#[async_trait]
impl Command for SearchPaletteCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        let window_label = ctx.window_label_from_scope().unwrap_or("main");
        let change = ui.set_palette_open_with_mode(window_label, true, "search");
        Ok(serde_json::to_value(change).unwrap_or(Value::Null))
    }
}

/// Dismiss — layered close: palette first, then topmost inspector.
///
/// Always available. Closes the palette if open in the invoking window,
/// otherwise pops the inspector stack. Returns a UIStateChange so the
/// frontend stays in sync.
pub struct DismissCmd;

#[async_trait]
impl Command for DismissCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        let window_label = ctx.window_label_from_scope().unwrap_or("main");

        // Layer 1: close palette if open in this window
        if ui.palette_open(window_label) {
            let change = ui.set_palette_open(window_label, false);
            return Ok(serde_json::to_value(change).unwrap_or(Value::Null));
        }

        // Layer 2: pop topmost inspector
        let inspector_stack = ui.inspector_stack(window_label);
        if !inspector_stack.is_empty() {
            let change = ui.inspector_close(window_label);
            return Ok(serde_json::to_value(change).unwrap_or(Value::Null));
        }

        // Nothing to dismiss
        Ok(Value::Null)
    }
}
