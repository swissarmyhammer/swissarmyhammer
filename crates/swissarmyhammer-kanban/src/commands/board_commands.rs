//! Board-level command implementations.
//!
//! `update.board` is the dispatch-layer wrapper around the
//! [`crate::board::UpdateBoard`] operation. Without this wrapper the AI panel
//! (and any future board-metadata editor) would dispatch `update.board` and
//! the command registry would reject it as unknown, dropping every write —
//! exactly the regression fixed by task `01KSNJ6AE18EQYDC2WSYFSSAY1`.
//!
//! The command intentionally mirrors `UpdateBoard`'s tri-state field shape:
//! `name`, `description`, and `model` are each optional. A missing arg leaves
//! the corresponding field on the board entity untouched, so partial updates
//! (e.g. "just set the model") never clobber unrelated metadata.

use super::run_op;
use crate::board::UpdateBoard;
use crate::commands_core::{Command, CommandContext};
use crate::context::KanbanContext;
use async_trait::async_trait;
use serde_json::Value;

/// Update the board entity's `name`, `description`, or `model` field.
///
/// Wraps [`crate::board::UpdateBoard`], reading each supported field from the
/// dispatched `args` bag. Every field is optional and is forwarded to
/// `UpdateBoard` only when present — a missing arg leaves the existing value
/// on the board entity untouched. `model` is validated against the kanban-
/// tagged chat-capable agent set by `UpdateBoard::execute` itself, so an
/// invalid id surfaces as a `CommandError::ExecutionFailed`.
pub struct UpdateBoardCmd;

#[async_trait]
impl Command for UpdateBoardCmd {
    /// Always available — a board is the singleton root of any open
    /// `.kanban`, so no scope-chain precondition applies. `UpdateBoard`
    /// itself reports a `NotInitialized` error if dispatched against a
    /// directory with no board entity, which surfaces cleanly through
    /// `run_op` as an execution failure.
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    /// Execute the wrapped [`UpdateBoard`] operation with whichever of the
    /// three supported fields the caller supplied. A call with no
    /// recognized fields succeeds as a no-op so a future caller can issue
    /// a partial save without special-casing the empty path.
    async fn execute(&self, ctx: &CommandContext) -> crate::commands_core::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let mut op = UpdateBoard::new();
        if let Some(name) = ctx.arg("name").and_then(|v| v.as_str()) {
            op = op.with_name(name);
        }
        if let Some(description) = ctx.arg("description").and_then(|v| v.as_str()) {
            op = op.with_description(description);
        }
        if let Some(model) = ctx.arg("model").and_then(|v| v.as_str()) {
            op = op.with_model(model);
        }

        run_op(&op, &kanban).await
    }
}
