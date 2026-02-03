//! Operation processor trait

use crate::{Execute, LogEntry};
use async_trait::async_trait;
use serde_json::Value;

/// Operation processor that handles execution and logging
///
/// Implementations orchestrate:
/// 1. Executing operations via the Execute trait
/// 2. Extracting log entries from ExecutionResult
/// 3. Writing logs to appropriate storage locations
#[async_trait]
pub trait OperationProcessor<C, E>
where
    C: Send + Sync,
    E: Send + Sync + std::fmt::Display,
{
    /// Execute an operation and handle any logging
    ///
    /// This is the main entry point - it:
    /// 1. Calls operation.execute(ctx)
    /// 2. Extracts the log entry (if any)
    /// 3. Writes logs to appropriate locations
    /// 4. Returns the final result
    async fn process<T>(&self, operation: &T, ctx: &C) -> Result<Value, E>
    where
        T: Execute<C, E> + Send + Sync;

    /// Write a log entry to persistent storage
    ///
    /// Implementations decide where logs go (files, DB, etc.)
    async fn write_log(
        &self,
        ctx: &C,
        log_entry: &LogEntry,
        affected_resources: &[String],
    ) -> Result<(), E>;
}
