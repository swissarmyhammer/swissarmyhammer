//! Entity-layer undo/redo command implementations.
//!
//! These commands operate on `StoreContext` (via extension) and use the
//! store-level undo/redo stack. They are entity-layer infrastructure,
//! reusable outside kanban.

use async_trait::async_trait;
use serde_json::{json, Value};

use swissarmyhammer_commands::{Command, CommandContext, CommandError};
use swissarmyhammer_store::StoreContext;

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
            Ok(()) => Ok(json!({ "undone": true })),
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
            Ok(()) => Ok(json!({ "redone": true })),
            Err(swissarmyhammer_store::StoreError::NotFound(_)) => Ok(json!({ "noop": true })),
            Err(e) => Err(CommandError::ExecutionFailed(e.to_string())),
        }
    }
}
