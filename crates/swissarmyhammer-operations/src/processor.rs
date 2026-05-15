//! Operation processor trait

use crate::Execute;
use async_trait::async_trait;
use serde_json::Value;

/// Operation processor that handles execution
///
/// Implementations orchestrate executing operations via the [`Execute`]
/// trait and lifting the [`crate::ExecutionResult`] into the domain
/// `Result<T, E>` that the caller expects.
#[async_trait]
pub trait OperationProcessor<C, E>
where
    C: Send + Sync,
    E: Send + Sync + std::fmt::Display,
{
    /// Execute an operation and return the result
    ///
    /// This is the main entry point - it:
    /// 1. Calls operation.execute(ctx)
    /// 2. Returns the final result
    async fn process<T>(&self, operation: &T, ctx: &C) -> Result<Value, E>
    where
        T: Execute<C, E> + Send + Sync;
}
