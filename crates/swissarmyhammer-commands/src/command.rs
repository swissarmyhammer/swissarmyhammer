use crate::context::CommandContext;
use crate::error::Result;
use async_trait::async_trait;
use serde_json::Value;

/// The Command trait — every command must implement both availability checking
/// and execution.
///
/// Commands are resolved by ID from the `CommandsRegistry`. The dispatcher
/// calls `available()` before `execute()` to gate access.
///
/// Both methods receive a `CommandContext` containing the scope chain, target,
/// args, and service references needed for execution.
#[async_trait]
pub trait Command: Send + Sync {
    /// Check whether this command can execute in the given context.
    ///
    /// Returns `true` if the command's preconditions are met (required entities
    /// in scope, undo stack non-empty, etc.). The dispatcher calls this before
    /// `execute()` and returns an error if it returns `false`.
    fn available(&self, ctx: &CommandContext) -> bool;

    /// Execute the command, returning a JSON result.
    ///
    /// The context provides scope chain resolution, argument access, and
    /// service references. Commands should resolve parameters from the
    /// context rather than taking explicit arguments.
    async fn execute(&self, ctx: &CommandContext) -> Result<Value>;
}
