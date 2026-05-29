//! Application-level command implementations: quit, keymap mode, about, help,
//! reset windows, dismiss, command palette, search palette, undo, redo.

use async_trait::async_trait;
use serde_json::{json, Value};
use swissarmyhammer_commands::{Command, CommandContext, CommandError};
use swissarmyhammer_entity::EntityContext;
use swissarmyhammer_perspectives::{PerspectiveContext, PERSPECTIVE_STORE_NAME};
use swissarmyhammer_store::{EventProvenance, StoreContext, StoreError, UndoEntryId, UndoOutcome};
use swissarmyhammer_views::{ViewsContext, VIEW_STORE_NAME};
use tokio::sync::RwLock;

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
                reconcile_post_undo_caches(ctx, &outcome, "undo").await;
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
                reconcile_post_undo_caches(ctx, &outcome, "redo").await;
                Ok(json!({ "redone": true }))
            }
            Err(StoreError::NotFound(_)) => Ok(json!({ "noop": true })),
            Err(e) => Err(CommandError::ExecutionFailed(e.to_string())),
        }
    }
}

/// The category of cache that owns a store, used to dispatch one uniform
/// reconcile per `(store, item)` without a bespoke per-store branch.
///
/// Resolved from the store name alone, so adding a new store of an existing
/// category (e.g. a new entity type) needs no code change here — it falls
/// into [`StoreCategory::Entity`] automatically. A genuinely new *kind* of
/// cache (not entity / perspective / view) is the only thing that would add
/// a variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoreCategory {
    /// An entity-backed store (`task`, `tag`, `column`, `actor`, `board`,
    /// `project`, …): reconciled through the [`EntityContext`] cache.
    Entity,
    /// The perspective store: reconciled through the [`PerspectiveContext`].
    Perspective,
    /// The view store: reconciled through the [`ViewsContext`].
    View,
}

impl StoreCategory {
    /// Classify a store by name. The perspective / view stores are named by
    /// the same `*_STORE_NAME` constants their `store_name()` impls return —
    /// if either moves, this stops matching at compile-checked equality, not
    /// silently. Everything else is an entity-backed store.
    fn of(store_name: &str) -> Self {
        if store_name == PERSPECTIVE_STORE_NAME {
            StoreCategory::Perspective
        } else if store_name == VIEW_STORE_NAME {
            StoreCategory::View
        } else {
            StoreCategory::Entity
        }
    }
}

/// Reconcile every in-memory cache that might shadow the on-disk state the
/// store layer just rewrote — uniformly, keyed on `outcome.items` + each
/// store's [`StoreCategory`].
///
/// Called after `StoreContext::undo` / `redo` succeeds. This is the
/// convergence point where undo/redo becomes indistinguishable from a normal
/// edit downstream: for every `(store, item)` the reversed/reapplied group
/// touched, it dispatches to the cache that owns that store's category, which
/// derives the byte transition (created / removed / field-diff) and emits on
/// the *same* broadcast bus a normal edit uses. Nothing here special-cases
/// undo vs redo — only the stamped `origin` differs.
///
/// `origin` is `"undo"` or `"redo"`. A single undo/redo call is one command,
/// so all of its reconciled items share one fresh `txn` — a consumer
/// coalesces them into one atomic re-render. (The reversed group's own id is
/// not exposed on `UndoOutcome`; the only invariant the UI needs is "same txn
/// for all items of this one undo".)
///
/// Per-category failures are independent and logged at warn-level, not
/// surfaced as command errors — the undo/redo already succeeded on disk.
async fn reconcile_post_undo_caches(ctx: &CommandContext, outcome: &UndoOutcome, origin: &str) {
    // Resolve the optional cache handles once, up front. Each is `None` when
    // its extension/sub-context is not attached (entity-only boards, cold
    // perspective context, …), and the per-item dispatch simply skips that
    // category — no bespoke guard per store name.
    let ectx = ctx.extension::<EntityContext>();
    let kanban = ctx.extension::<KanbanContext>();
    let perspectives = kanban
        .as_ref()
        .and_then(|k| k.perspective_context_if_ready());
    let views = kanban.as_ref().and_then(|k| k.views());

    reconcile_caches(
        outcome,
        origin,
        ectx.as_deref(),
        views,
        perspectives,
    )
    .await;
}

/// The category-keyed, per-item cache reconcile shared by production
/// (`reconcile_post_undo_caches`) and the change-propagation e2e test.
///
/// Extracted so the two cannot diverge: this is the single body that turns one
/// finished `UndoOutcome` into the dependent-cache resyncs + their emitted
/// `store/changed` events. Production resolves the cache handles off the
/// [`CommandContext`] extensions; the test holds them directly — both then
/// pass them here.
///
/// Each handle is `Option` because a category may be absent on a given board
/// (entity-only boards have no views/perspectives; a cold board has no entity
/// cache attached yet). A `None` handle means the per-item dispatch skips that
/// category — there is no bespoke guard per store name.
///
/// All items of one undo/redo call share a single fresh `txn`: one undo/redo
/// is one command, so a consumer coalesces its items into one atomic
/// re-render. `origin` is `"undo"` or `"redo"`.
pub async fn reconcile_caches(
    outcome: &UndoOutcome,
    origin: &str,
    entity: Option<&EntityContext>,
    views: Option<&RwLock<ViewsContext>>,
    perspectives: Option<&RwLock<PerspectiveContext>>,
) {
    let txn = UndoEntryId::new().to_string();

    for (store_name, item_id) in &outcome.items {
        let prov = EventProvenance::new(Some(txn.clone()), origin);
        match StoreCategory::of(store_name) {
            StoreCategory::Entity => {
                if let Some(ectx) = entity {
                    ectx.sync_entity_cache_from_disk_with(store_name, item_id.as_str(), prov)
                        .await;
                }
            }
            StoreCategory::Perspective => {
                if let Some(pctx) = perspectives {
                    let mut pctx = pctx.write().await;
                    if let Err(e) = pctx.reload_from_disk_with(item_id.as_str(), prov).await {
                        tracing::warn!(
                            id = %item_id.as_str(),
                            error = %e,
                            "perspective reload_from_disk after undo/redo failed"
                        );
                    }
                }
            }
            StoreCategory::View => {
                if let Some(views_lock) = views {
                    let mut views = views_lock.write().await;
                    if let Err(e) = views.reload_from_disk_with(item_id.as_str(), prov).await {
                        tracing::warn!(
                            id = %item_id.as_str(),
                            error = %e,
                            "view reload_from_disk after undo/redo failed"
                        );
                    }
                }
            }
        }
    }
}
