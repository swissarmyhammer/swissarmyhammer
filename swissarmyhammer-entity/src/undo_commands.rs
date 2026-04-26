//! Entity-layer undo/redo command implementations.
//!
//! These commands operate on `StoreContext` (via extension) and use the
//! store-level undo/redo stack. They are entity-layer infrastructure,
//! reusable outside kanban.
//!
//! Undo and redo at the [`StoreContext`] layer rewrite files on disk but
//! do not know about the entity-layer cache that [`EntityContext`] keeps
//! in memory. When an [`EntityContext`] extension is attached to the
//! command context, these commands reconcile the cache with the post-undo
//! on-disk state (evicting entries whose file is now gone, refreshing
//! entries whose file was rewritten) so subsequent reads see the reversed
//! state without waiting for the file-watcher event to round-trip.

use async_trait::async_trait;
use serde_json::{json, Value};

use swissarmyhammer_commands::{Command, CommandContext, CommandError};
use swissarmyhammer_store::{StoreContext, UndoOutcome};

use crate::context::EntityContext;

/// Undo the most recent undoable operation.
///
/// Delegates to `StoreContext::undo()` which finds the correct store and
/// reverses the most recent changelog entry.
/// Returns `{ "noop": true }` when the stack is empty.
pub struct UndoCmd;

#[async_trait]
impl Command for UndoCmd {
    /// Returns `true` only when the undo stack has entries to undo.
    ///
    /// Checks the cached `can_undo` flag on UIState, which is updated after
    /// every stack-mutating operation (write, delete, undo, redo). Falls back
    /// to `false` if UIState is not available on the context.
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.ui_state
            .as_ref()
            .map(|ui| ui.can_undo())
            .unwrap_or(false)
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let store_ctx = ctx.require_extension::<StoreContext>()?;

        match store_ctx.undo().await {
            Ok(outcome) => {
                sync_entity_cache(ctx, &outcome).await;
                Ok(json!({ "undone": true }))
            }
            Err(swissarmyhammer_store::StoreError::NotFound(_)) => Ok(json!({ "noop": true })),
            Err(e) => Err(CommandError::ExecutionFailed(e.to_string())),
        }
    }
}

/// Redo the most recently undone operation.
///
/// Delegates to `StoreContext::redo()` which finds the correct store and
/// re-applies the most recently undone changelog entry.
/// Returns `{ "noop": true }` when the stack is empty.
pub struct RedoCmd;

#[async_trait]
impl Command for RedoCmd {
    /// Returns `true` only when the undo stack has entries to redo.
    ///
    /// Checks the cached `can_redo` flag on UIState, which is updated after
    /// every stack-mutating operation (write, delete, undo, redo). Falls back
    /// to `false` if UIState is not available on the context.
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.ui_state
            .as_ref()
            .map(|ui| ui.can_redo())
            .unwrap_or(false)
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let store_ctx = ctx.require_extension::<StoreContext>()?;

        match store_ctx.redo().await {
            Ok(outcome) => {
                sync_entity_cache(ctx, &outcome).await;
                Ok(json!({ "redone": true }))
            }
            Err(swissarmyhammer_store::StoreError::NotFound(_)) => Ok(json!({ "noop": true })),
            Err(e) => Err(CommandError::ExecutionFailed(e.to_string())),
        }
    }
}

/// Reconcile the entity-layer cache with the post-undo/redo disk state.
///
/// `StoreContext::undo` / `redo` move entity files on disk but do not update
/// the [`EntityContext`] cache. When the affected store is backed by an
/// `EntityContext` (the usual kanban / perspectives wiring), this helper
/// asks the context to re-read the entity from disk: [`EntityContext::sync_entity_cache_from_disk`]
/// refreshes the cache when the file exists and evicts it when it does not,
/// so callers of `EntityContext::read` / `list` see the reversed state
/// immediately without waiting for the file-watcher event round trip.
///
/// A no-op when no [`EntityContext`] extension is attached — the store-only
/// callers (perspective tests, raw [`StoreContext`] users) are unaffected.
async fn sync_entity_cache(ctx: &CommandContext, outcome: &UndoOutcome) {
    let Some(ectx) = ctx.extension::<EntityContext>() else {
        return;
    };
    ectx.sync_entity_cache_from_disk(&outcome.store_name, outcome.item_id.as_str())
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_commands::ui_state::UIState;

    #[tokio::test]
    async fn undo_cmd_available_false_without_ui_state() {
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        assert!(!UndoCmd.available(&ctx));
    }

    #[tokio::test]
    async fn redo_cmd_available_false_without_ui_state() {
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        assert!(!RedoCmd.available(&ctx));
    }

    #[tokio::test]
    async fn undo_cmd_available_with_ui_state_default_is_false() {
        let ui = Arc::new(UIState::default());
        let ctx = CommandContext::new("test", vec![], None, HashMap::new()).with_ui_state(ui);
        // Default UIState has can_undo = false
        assert!(!UndoCmd.available(&ctx));
    }

    #[tokio::test]
    async fn redo_cmd_available_with_ui_state_default_is_false() {
        let ui = Arc::new(UIState::default());
        let ctx = CommandContext::new("test", vec![], None, HashMap::new()).with_ui_state(ui);
        assert!(!RedoCmd.available(&ctx));
    }

    #[tokio::test]
    async fn undo_cmd_missing_extension_errors() {
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        let result = UndoCmd.execute(&ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn redo_cmd_missing_extension_errors() {
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        let result = RedoCmd.execute(&ctx).await;
        assert!(result.is_err());
    }
}
