//! UI command implementations: inspector, palette, active view.

use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_commands::{Command, CommandContext, CommandError};

/// Open the inspector for a target entity.
///
/// Available when a target moniker or an inspectable scope chain entry is present.
/// Inspectable types: task, tag, column, board, actor.
pub struct InspectCmd;

/// Entity types that are meaningful to inspect.
const INSPECTABLE_TYPES: &[&str] = &["task", "tag", "column", "board", "actor"];

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

/// Enter inline rename mode for the active perspective tab.
///
/// No-op on the backend — exists in the registry so the command palette can
/// discover it.  The frontend resolves the local `execute` handler registered
/// in `AppShell`'s global commands, so this never actually runs.
///
/// Available only when the current window has an active perspective. This
/// makes the command view-aware: switching to a view kind with no perspective
/// selected (or a fresh window with none set) hides the palette entry so
/// users do not see a command that has nothing to rename.
pub struct StartRenamePerspectiveCmd;

#[async_trait]
impl Command for StartRenamePerspectiveCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        // No UIState means we cannot check — fail closed (unavailable) to
        // avoid showing a non-functional palette entry.
        let Some(ui) = ctx.ui_state.as_ref() else {
            return false;
        };
        let window_label = ctx.window_label_from_scope().unwrap_or("main");
        !ui.active_perspective_id(window_label).is_empty()
    }

    async fn execute(&self, _ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        // Intentional no-op — the frontend intercepts this command before it
        // reaches the backend.  Return null so the caller sees success.
        Ok(Value::Null)
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

        // Keep the backend scope_chain consistent with the newly active view.
        //
        // The command palette and right-click menu both read `scope_chain` from
        // UIState to ask the backend which commands are available. Dynamic
        // commands like `entity.add:{type}` fan out from the `view:{id}` moniker
        // in that chain. If we only update `active_view` here without touching
        // `scope_chain`, the palette keeps emitting commands for whichever view
        // happened to be in scope last (commonly the board the user launched
        // from) even after they switch to a different view — so "New Tag" and
        // "New Project" never appear on their respective grids.
        //
        // Rewrite every `view:*` element in the current chain to point at the
        // new active view. When the user later focuses a FocusScope inside the
        // new view, `ui.setFocus` will rebuild the full chain from scratch —
        // this bridge makes the palette work in the interim.
        let mut chain = ui.scope_chain();
        let mut mutated = false;
        for moniker in &mut chain {
            if moniker.starts_with("view:") {
                let new_moniker = format!("view:{view_id}");
                if *moniker != new_moniker {
                    *moniker = new_moniker;
                    mutated = true;
                }
            }
        }
        if mutated {
            ui.set_scope_chain(chain);
        }

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

    #[test]
    fn first_inspectable_skips_field_monikers() {
        // "field:task:abc.title" has entity_type "field", which is not in
        // INSPECTABLE_TYPES, so it should be skipped.
        let scope = vec![
            "field:task:abc.title".to_string(),
            "task:abc".to_string(),
            "column:todo".to_string(),
        ];
        let result = first_inspectable(&scope);
        assert_eq!(
            result,
            Some("task:abc"),
            "should skip field moniker and find task"
        );
    }

    #[test]
    fn first_inspectable_returns_none_for_only_field_monikers() {
        let scope = vec!["field:task:abc.title".to_string()];
        assert!(first_inspectable(&scope).is_none());
    }

    /// When the active view changes, the `view:{id}` moniker in the current
    /// scope_chain must be rewritten to point at the new view. This is the
    /// regression guard for the user-visible bug where switching from the
    /// Board to the Tags/Projects grid left the backend scope_chain pointing
    /// at the Board's view id, so the command palette kept offering
    /// "New Task" instead of "New Tag" / "New Project".
    #[tokio::test]
    async fn set_active_view_rewrites_view_moniker_in_scope_chain() {
        let ui = Arc::new(UIState::new());
        // Simulate the user having focused a task card on the board, which
        // landed this chain in UIState via a prior ui.setFocus dispatch.
        ui.set_scope_chain(vec![
            "task:01ABC".to_string(),
            "column:todo".to_string(),
            "board:board".to_string(),
            "view:01JMVIEW0000000000BOARD0".to_string(),
            "window:main".to_string(),
            "engine".to_string(),
        ]);

        let mut args = HashMap::new();
        args.insert(
            "view_id".to_string(),
            serde_json::json!("01JMVIEW0000000000TGGRD0"),
        );
        let ctx = CommandContext::new("ui.view.set", vec!["window:main".to_string()], None, args)
            .with_ui_state(Arc::clone(&ui));

        SetActiveViewCmd.execute(&ctx).await.unwrap();

        let chain = ui.scope_chain();
        assert!(
            chain.contains(&"view:01JMVIEW0000000000TGGRD0".to_string()),
            "scope_chain must now reference the NEW active view, got: {chain:?}"
        );
        assert!(
            !chain.contains(&"view:01JMVIEW0000000000BOARD0".to_string()),
            "scope_chain must not still reference the OLD view, got: {chain:?}"
        );
    }

    /// If no `view:*` moniker is in the current scope_chain, changing the
    /// active view must not synthesise one — the scope_chain stays untouched
    /// and the next ui.setFocus rebuild populates it. This guards against
    /// spurious scope changes when the user hasn't focused anything yet.
    #[tokio::test]
    async fn set_active_view_leaves_scope_chain_alone_when_no_view_moniker() {
        let ui = Arc::new(UIState::new());
        ui.set_scope_chain(vec!["window:main".to_string(), "engine".to_string()]);

        let mut args = HashMap::new();
        args.insert(
            "view_id".to_string(),
            serde_json::json!("01JMVIEW0000000000TGGRD0"),
        );
        let ctx = CommandContext::new("ui.view.set", vec!["window:main".to_string()], None, args)
            .with_ui_state(Arc::clone(&ui));

        SetActiveViewCmd.execute(&ctx).await.unwrap();

        assert_eq!(
            ui.scope_chain(),
            vec!["window:main".to_string(), "engine".to_string()],
            "scope_chain must be untouched when it has no view:* moniker"
        );
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

    #[test]
    fn start_rename_perspective_available_requires_active_perspective() {
        // With no active perspective set for the window, the command should
        // not be available — it has nothing to rename.
        let ui = Arc::new(UIState::new());
        let ctx = CommandContext::new(
            "ui.perspective.startRename",
            vec!["window:main".to_string()],
            None,
            HashMap::new(),
        )
        .with_ui_state(Arc::clone(&ui));

        let cmd = StartRenamePerspectiveCmd;
        assert!(
            !cmd.available(&ctx),
            "should be unavailable when no active perspective"
        );

        // After setting an active perspective for the main window, the
        // command becomes available.
        ui.set_active_perspective("main", "p1");
        assert!(
            cmd.available(&ctx),
            "should be available when an active perspective exists"
        );
    }

    #[test]
    fn start_rename_perspective_available_per_window() {
        // The availability check is scoped to the window label resolved from
        // the scope chain — an active perspective on window A must not make
        // the command available for window B.
        let ui = Arc::new(UIState::new());
        ui.set_active_perspective("main", "p1");

        let ctx_secondary = CommandContext::new(
            "ui.perspective.startRename",
            vec!["window:secondary".to_string()],
            None,
            HashMap::new(),
        )
        .with_ui_state(Arc::clone(&ui));

        let cmd = StartRenamePerspectiveCmd;
        assert!(
            !cmd.available(&ctx_secondary),
            "active perspective on main should not affect secondary window"
        );
    }
}
