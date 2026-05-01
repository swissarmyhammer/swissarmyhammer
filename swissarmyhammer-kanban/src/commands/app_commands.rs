//! Application-level command implementations: quit, keymap mode, about, help,
//! reset windows, dismiss, command palette, search palette, undo, redo.

use async_trait::async_trait;
use serde_json::{json, Value};
use swissarmyhammer_commands::{Command, CommandContext, CommandError};
use swissarmyhammer_entity::EntityContext;
use swissarmyhammer_perspectives::PERSPECTIVE_STORE_NAME;
use swissarmyhammer_store::{StoreContext, StoreError, UndoOutcome};

use crate::context::KanbanContext;

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

/// Kanban-aware undo command.
///
/// This is a **parallel implementation** of the generic
/// `swissarmyhammer_entity::UndoCmd` flow — not a wrapper. The body
/// duplicates the `StoreContext::undo` + error-matching + success-value
/// sequence because it needs the `UndoOutcome` to dispatch to both
/// reconciliation hooks (entity cache and perspective cache), and the
/// generic command does not expose the outcome to its callers. Delegating
/// would require either returning the outcome from `UndoCmd::execute`
/// (a breaking change to its public API) or re-running `StoreContext::undo`
/// which would be racy.
///
/// The control flow mirrors the entity-layer command:
///
///   1. `StoreContext::undo()` finds the owning store and rewrites bytes
///      on disk.
///   2. For entity-backed stores, the attached `EntityContext` cache is
///      refreshed via `sync_entity_cache_from_disk`.
///   3. **New** — when the undo target was a perspective, the
///      `PerspectiveContext` cache is refreshed via `reload_from_disk`,
///      which also emits `PerspectiveEvent::PerspectiveChanged` so the
///      Tauri bridge forwards the refresh to the frontend.
///
/// Both reconciliation steps are orthogonal: entity-only boards still
/// work (the perspective branch is a no-op when no perspective extension
/// is attached), and perspective-only mutations still work (the entity
/// branch is a no-op when the store isn't entity-backed).
pub struct KanbanUndoCmd;

#[async_trait]
impl Command for KanbanUndoCmd {
    /// Returns `true` only when the undo stack has entries to undo.
    ///
    /// Delegates to the generic `UndoCmd::available` so the availability
    /// contract stays consistent across crates.
    fn available(&self, ctx: &CommandContext) -> bool {
        swissarmyhammer_entity::UndoCmd.available(ctx)
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let store_ctx = ctx.require_extension::<StoreContext>()?;

        match store_ctx.undo().await {
            Ok(outcome) => {
                reconcile_post_undo_caches(ctx, &outcome).await;
                Ok(json!({ "undone": true }))
            }
            Err(StoreError::NotFound(_)) => Ok(json!({ "noop": true })),
            Err(e) => Err(CommandError::ExecutionFailed(e.to_string())),
        }
    }
}

/// Kanban-aware redo command.
///
/// See [`KanbanUndoCmd`] for the rationale. Structurally identical —
/// a parallel implementation (not a wrapper) of
/// `swissarmyhammer_entity::RedoCmd`: calls `StoreContext::redo()` and
/// runs the same post-redo reconciliation over the returned
/// `UndoOutcome`.
pub struct KanbanRedoCmd;

#[async_trait]
impl Command for KanbanRedoCmd {
    /// Returns `true` only when the undo stack has entries to redo.
    fn available(&self, ctx: &CommandContext) -> bool {
        swissarmyhammer_entity::RedoCmd.available(ctx)
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let store_ctx = ctx.require_extension::<StoreContext>()?;

        match store_ctx.redo().await {
            Ok(outcome) => {
                reconcile_post_undo_caches(ctx, &outcome).await;
                Ok(json!({ "redone": true }))
            }
            Err(StoreError::NotFound(_)) => Ok(json!({ "noop": true })),
            Err(e) => Err(CommandError::ExecutionFailed(e.to_string())),
        }
    }
}

/// Reconcile every in-memory cache that might shadow the on-disk state
/// the store layer just rewrote.
///
/// Called after `StoreContext::undo` / `redo` succeeds. Two caches may
/// need syncing, keyed by `outcome.store_name`:
///
///   - **Entity-backed stores** (`task`, `tag`, `column`, `actor`,
///     `board`, `project`, `attachment`): the [`EntityContext`] cache
///     holds parsed `Entity` values. `sync_entity_cache_from_disk`
///     refreshes or evicts the entry so the next read sees the reversed
///     state without waiting for the file-watcher round trip. No-op when
///     no `EntityContext` extension is attached.
///
///   - **Perspective store** (`perspective`): the [`PerspectiveContext`]
///     cache holds parsed `Perspective` values and is accessible via the
///     [`KanbanContext`] extension. `reload_from_disk` refreshes or
///     evicts the entry *and* emits a `PerspectiveChanged` /
///     `PerspectiveDeleted` broadcast event. The Tauri bridge forwards
///     that event to the frontend as `entity-field-changed` /
///     `entity-removed` with `entity_type = "perspective"`, which drives
///     the perspective-list re-fetch in `perspective-context.tsx`.
///     No-op when no `KanbanContext` extension is attached or the
///     perspective sub-context hasn't been initialized yet.
///
/// The two branches are independent: failure of one does not affect the
/// other. Errors are logged at warn-level and otherwise swallowed — the
/// undo/redo itself already succeeded on disk, so surfacing a cache-sync
/// failure as a command error would misrepresent what happened.
async fn reconcile_post_undo_caches(ctx: &CommandContext, outcome: &UndoOutcome) {
    // Entity-layer reconciliation — orthogonal to perspective reconciliation.
    if let Some(ectx) = ctx.extension::<EntityContext>() {
        ectx.sync_entity_cache_from_disk(&outcome.store_name, outcome.item_id.as_str())
            .await;
    }

    // Perspective-layer reconciliation — only fires when the undo target
    // was a perspective. Guarded by `store_name` because there's no sense
    // reading the perspective directory when the reversed mutation was a
    // task or tag edit. The `PERSPECTIVE_STORE_NAME` constant is the same
    // string `PerspectiveStore::store_name()` returns — if either side
    // moves, compilation fails rather than silently falling through.
    if outcome.store_name == PERSPECTIVE_STORE_NAME {
        let Some(kanban) = ctx.extension::<KanbanContext>() else {
            return;
        };
        // Use the non-initializing accessor: if the perspective subcontext
        // has not been touched yet, there is nothing to reconcile. Lazy
        // initialization here would trigger a fresh load_all on a cold
        // context, which is wasteful and can mask bugs in the caller.
        let Some(pctx) = kanban.perspective_context_if_ready() else {
            return;
        };
        let mut pctx = pctx.write().await;
        if let Err(e) = pctx.reload_from_disk(outcome.item_id.as_str()).await {
            tracing::warn!(
                id = %outcome.item_id.as_str(),
                error = %e,
                "perspective reload_from_disk after undo/redo failed"
            );
        }
    }
}
