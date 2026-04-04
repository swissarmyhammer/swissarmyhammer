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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::get::GetExpression;
    use crate::expression::set::SetExpression;

    #[test]
    fn test_new() {
        let _processor = JsOperationProcessor::new();
    }

    #[test]
    fn test_default() {
        let _processor = JsOperationProcessor;
    }

    #[tokio::test]
    async fn test_process_set_operation() {
        let processor = JsOperationProcessor::new();
        let ctx = JsContext::new();

        let op = SetExpression {
            name: Some("proc_test_x".to_string()),
            expression: Some(serde_json::Value::String("42".to_string())),
        };

        let result = processor.process(&op, &ctx).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!(42));
    }

    #[tokio::test]
    async fn test_process_get_operation() {
        let processor = JsOperationProcessor::new();
        let ctx = JsContext::new();

        // First set a variable
        let set_op = SetExpression {
            name: Some("proc_get_var".to_string()),
            expression: Some(serde_json::Value::String("'hello'".to_string())),
        };
        let _ = processor.process(&set_op, &ctx).await;

        // Then get it
        let get_op = GetExpression {
            name: Some("proc_get_var".to_string()),
        };
        let result = processor.process(&get_op, &ctx).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!("hello"));
    }

    #[tokio::test]
    async fn test_process_failed_operation() {
        let processor = JsOperationProcessor::new();
        let ctx = JsContext::new();

        let op = SetExpression {
            name: None,
            expression: None,
        };

        let result = processor.process(&op, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_write_log_is_noop() {
        let processor = JsOperationProcessor::new();
        let ctx = JsContext::new();

        let log_entry = LogEntry::new(
            "set expression",
            serde_json::json!({"name": "x"}),
            serde_json::json!(42),
            None,
            0,
        );

        let result = processor.write_log(&ctx, &log_entry, &[]).await;
        assert!(result.is_ok());
    }
}
