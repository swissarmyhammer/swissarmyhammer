//! UI command implementations: inspector, palette, active view.

use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_commands::{Command, CommandContext, CommandError};

/// Open the inspector for a target entity.
///
/// Available when a target moniker or an inspectable scope chain entry is present.
/// Inspectable types: task, tag, column, board, swimlane, actor.
pub struct InspectCmd;

/// Entity types that are meaningful to inspect.
const INSPECTABLE_TYPES: &[&str] = &["task", "tag", "column", "board", "swimlane", "actor"];

/// Find the first inspectable moniker in the scope chain.
fn first_inspectable(scope_chain: &[String]) -> Option<&str> {
    scope_chain.iter().find_map(|m| {
        let (entity_type, _) = swissarmyhammer_commands::parse_moniker(m)?;
        if INSPECTABLE_TYPES.contains(&entity_type) {
            Some(m.as_str())
        } else {
            None
        }
    })
}

#[async_trait]
impl Command for InspectCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.target.is_some() || first_inspectable(&ctx.scope_chain).is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        let moniker = ctx
            .target
            .as_deref()
            .or_else(|| first_inspectable(&ctx.scope_chain))
            .ok_or_else(|| CommandError::MissingArg("target".into()))?;

        let window_label = ctx.window_label_from_scope().unwrap_or("main");
        let change = ui.inspect(window_label, moniker);
        Ok(serde_json::to_value(change).unwrap_or(Value::Null))
    }
}

/// Close the topmost inspector entry.
///
/// Always available.
pub struct InspectorCloseCmd;

#[async_trait]
impl Command for InspectorCloseCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        let window_label = ctx.window_label_from_scope().unwrap_or("main");
        let change = ui.inspector_close(window_label);
        Ok(serde_json::to_value(change).unwrap_or(Value::Null))
    }
}

/// Close all inspector entries.
///
/// Always available.
pub struct InspectorCloseAllCmd;

#[async_trait]
impl Command for InspectorCloseAllCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        let window_label = ctx.window_label_from_scope().unwrap_or("main");
        let change = ui.inspector_close_all(window_label);
        Ok(serde_json::to_value(change).unwrap_or(Value::Null))
    }
}

/// Open the command palette.
///
/// Always available.
pub struct PaletteOpenCmd;

#[async_trait]
impl Command for PaletteOpenCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        let window_label = ctx.window_label_from_scope().unwrap_or("main");
        let change = ui.set_palette_open(window_label, true);
        Ok(serde_json::to_value(change).unwrap_or(Value::Null))
    }
}

/// Close the command palette.
///
/// Always available.
pub struct PaletteCloseCmd;

#[async_trait]
impl Command for PaletteCloseCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        let window_label = ctx.window_label_from_scope().unwrap_or("main");
        let change = ui.set_palette_open(window_label, false);
        Ok(serde_json::to_value(change).unwrap_or(Value::Null))
    }
}

/// Set the focus scope chain.
///
/// Always available. Required arg: `scope_chain` (array of strings).
/// This replaces the standalone `set_focus` Tauri command, routing through
/// the unified command dispatch pipeline.
pub struct SetFocusCmd;

#[async_trait]
impl Command for SetFocusCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        let scope_chain: Vec<String> = ctx
            .args
            .get("scope_chain")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let change = ui.set_scope_chain(scope_chain);
        Ok(serde_json::to_value(change).unwrap_or(Value::Null))
    }
}

/// Set the active perspective by ID.
///
/// Always available. Required arg: `perspective_id`.
pub struct SetActivePerspectiveCmd;

#[async_trait]
impl Command for SetActivePerspectiveCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        let perspective_id = ctx.require_arg_str("perspective_id")?;
        let window_label = ctx.window_label_from_scope().unwrap_or("main");
        let change = ui.set_active_perspective(window_label, perspective_id);
        Ok(serde_json::to_value(change).unwrap_or(Value::Null))
    }
}

/// Set the application interaction mode (normal, command, search).
///
/// Always available. Required arg: `mode`.
pub struct SetAppModeCmd;

#[async_trait]
impl Command for SetAppModeCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        let mode = ctx.require_arg_str("mode")?;
        let window_label = ctx.window_label_from_scope().unwrap_or("main");
        let change = ui.set_app_mode(window_label, mode);
        Ok(serde_json::to_value(change).unwrap_or(Value::Null))
    }
}

/// Set the active view by ID.
///
/// Always available. Required arg: `view_id`.
pub struct SetActiveViewCmd;

#[async_trait]
impl Command for SetActiveViewCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        let view_id = ctx.require_arg_str("view_id")?;
        let window_label = ctx.window_label_from_scope().unwrap_or("main");
        let change = ui.set_active_view(window_label, view_id);
        Ok(serde_json::to_value(change).unwrap_or(Value::Null))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_commands::{CommandContext, UIState};

    /// Helper to build a CommandContext with UIState and a window scope chain.
    fn ctx_with_mode_arg(mode: &str) -> CommandContext {
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("mode".to_string(), serde_json::json!(mode));
        CommandContext::new("ui.mode.set", vec!["window:main".to_string()], None, args)
            .with_ui_state(ui)
    }

    #[tokio::test]
    async fn set_app_mode_changes_ui_state() {
        let ctx = ctx_with_mode_arg("command");
        let cmd = SetAppModeCmd;

        assert!(cmd.available(&ctx));

        let result = cmd.execute(&ctx).await.unwrap();
        // Should return the AppMode change
        assert!(!result.is_null());

        // Verify state was updated
        let ui = ctx.ui_state.as_ref().unwrap();
        assert_eq!(ui.app_mode("main"), "command");
    }

    #[tokio::test]
    async fn set_app_mode_noop_returns_null() {
        let ctx = ctx_with_mode_arg("normal");
        let cmd = SetAppModeCmd;

        // "normal" is the default — should be a no-op
        let result = cmd.execute(&ctx).await.unwrap();
        assert!(result.is_null());
    }

    #[tokio::test]
    async fn set_app_mode_uses_window_from_scope() {
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("mode".to_string(), serde_json::json!("search"));
        let ctx = CommandContext::new(
            "ui.mode.set",
            vec!["window:secondary".to_string()],
            None,
            args,
        )
        .with_ui_state(ui.clone());

        let cmd = SetAppModeCmd;
        cmd.execute(&ctx).await.unwrap();

        assert_eq!(ui.app_mode("secondary"), "search");
        // Main window should still be "normal"
        assert_eq!(ui.app_mode("main"), "normal");
    }
}
