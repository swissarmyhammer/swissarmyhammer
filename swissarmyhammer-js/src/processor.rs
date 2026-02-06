//! JS operation processor
//!
//! Simple processor that delegates to operation execution without logging,
//! since JS variable state is in-memory and doesn't need audit trails.

use crate::error::{JsError, Result};
use crate::JsContext;
use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_operations::{Execute, LogEntry, OperationProcessor};

/// Processor for JS operations
///
/// Unlike the kanban processor, this doesn't write logs since
/// JS variable operations are ephemeral in-memory state.
pub struct JsOperationProcessor;

impl JsOperationProcessor {
    /// Create a new processor
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsOperationProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OperationProcessor<JsContext, JsError> for JsOperationProcessor {
    async fn process<T>(&self, operation: &T, ctx: &JsContext) -> Result<Value>
    where
        T: Execute<JsContext, JsError> + Send + Sync,
    {
        let exec_result = operation.execute(ctx).await;
        let (result, _log_entry) = exec_result.split();
        result
    }

    async fn write_log(
        &self,
        _ctx: &JsContext,
        _log_entry: &LogEntry,
        _affected_resources: &[String],
    ) -> Result<()> {
        // No logging for in-memory JS state operations
        Ok(())
    }
}
