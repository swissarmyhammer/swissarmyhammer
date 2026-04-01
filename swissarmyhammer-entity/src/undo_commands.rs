//! Entity-layer undo/redo command implementations.
//!
//! These commands operate on `EntityContext` directly (via extension) and use
//! the EntityContext's built-in `UndoStack` to track which changelog/transaction
//! IDs to target. They are entity-layer infrastructure, reusable outside kanban.

use async_trait::async_trait;
use serde_json::{json, Value};

use swissarmyhammer_commands::{Command, CommandContext, CommandError};

use crate::context::EntityContext;

/// Undo the most recent undoable operation.
///
/// Reads the undo target from the EntityContext's UndoStack, calls
/// `EntityContext::undo()` (which also updates the stack pointer and saves),
/// then returns the result.
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
        let ectx = ctx.require_extension::<EntityContext>()?;

        // Read the undo target ID from the stack (clone to release the lock)
        let id = {
            let stack = ectx.undo_stack().await;
            match stack.undo_target() {
                Some(id) => id.to_string(),
                None => return Ok(json!({ "noop": true })),
            }
        };

        // undo() internally calls record_undo + save_undo_stack
        let result_ulid = ectx
            .undo(&id)
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        Ok(json!({ "undone": id, "operation_id": result_ulid }))
    }
}

/// Redo the most recently undone operation.
///
/// Reads the redo target from the EntityContext's UndoStack, calls
/// `EntityContext::redo()` (which also updates the stack pointer and saves),
/// then returns the result.
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
        let ectx = ctx.require_extension::<EntityContext>()?;

        // Read the redo target ID from the stack (clone to release the lock)
        let id = {
            let stack = ectx.undo_stack().await;
            match stack.redo_target() {
                Some(id) => id.to_string(),
                None => return Ok(json!({ "noop": true })),
            }
        };

        // redo() internally calls record_redo + save_undo_stack
        let result_ulid = ectx
            .redo(&id)
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        Ok(json!({ "redone": id, "operation_id": result_ulid }))
    }
}
