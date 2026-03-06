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

        let change = ui.inspect(moniker);
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

        let change = ui.inspector_close();
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

        let change = ui.inspector_close_all();
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

        let change = ui.set_palette_open(true);
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

        let change = ui.set_palette_open(false);
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
        let change = ui.set_active_view(view_id);
        Ok(serde_json::to_value(change).unwrap_or(Value::Null))
    }
}
