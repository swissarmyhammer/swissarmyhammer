//! The core Operation and Execute traits

use crate::{ExecutionResult, ParamMeta};
use async_trait::async_trait;
use serde_json::Value;

/// Metadata about an operation - verb, noun, description
///
/// This trait provides static metadata. The struct fields
/// themselves define the parameters.
///
/// Note: Methods take &self to make the trait object-safe, but
/// implementations return static values.
pub trait Operation: Send + Sync {
    /// The action verb (e.g., "add", "get", "list", "update", "delete")
    fn verb(&self) -> &'static str;

    /// The target noun (e.g., "task", "board", "file")
    fn noun(&self) -> &'static str;

    /// Human-readable description
    fn description(&self) -> &'static str;

    /// Get parameter metadata for CLI generation
    ///
    /// Default returns empty - derive macro will override this
    fn parameters(&self) -> &'static [ParamMeta] {
        &[]
    }

    /// CLI usage examples (optional)
    fn examples(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }

    /// The canonical op string (e.g., "add task")
    fn op_string(&self) -> String {
        format!("{} {}", self.verb(), self.noun())
    }
}

/// Execute trait for running operations with a specific context type
///
/// Generic over the context type (C) and error type (E), allowing
/// different domains to use their own context and error types.
#[async_trait]
pub trait Execute<C, E>: Operation
where
    C: Send + Sync,
{
    /// Execute the operation and return result + logging intent
    ///
    /// Returns ExecutionResult which indicates:
    /// - Logged: Mutation operations that should be audited
    /// - Unlogged: Read-only operations with no side effects
    /// - Failed: Errors (optionally logged)
    async fn execute(&self, ctx: &C) -> ExecutionResult<Value, E>;

    /// Extract affected resource IDs for targeted logging
    ///
    /// Used for per-resource logs (e.g., per-task logs in kanban).
    /// Default returns empty (most operations don't affect specific resources).
    fn affected_resource_ids(&self, _result: &Value) -> Vec<String> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestContext;

    #[derive(Debug)]
    struct TestError;

    // Example operation struct - fields ARE the parameters
    struct AddTask {
        title: String,
        #[allow(dead_code)]
        description: Option<String>,
    }

    impl Operation for AddTask {
        fn verb(&self) -> &'static str {
            "add"
        }
        fn noun(&self) -> &'static str {
            "task"
        }
        fn description(&self) -> &'static str {
            "Create a new task"
        }
    }

    #[async_trait]
    impl Execute<TestContext, TestError> for AddTask {
        async fn execute(&self, _ctx: &TestContext) -> ExecutionResult<Value, TestError> {
            ExecutionResult::Unlogged {
                value: serde_json::json!({
                    "title": self.title
                }),
            }
        }
    }

    #[test]
    fn test_operation_metadata() {
        let op = AddTask {
            title: "Test".to_string(),
            description: None,
        };
        assert_eq!(op.verb(), "add");
        assert_eq!(op.noun(), "task");
        assert_eq!(op.op_string(), "add task");
    }

    #[tokio::test]
    async fn test_execute() {
        let op = AddTask {
            title: "Test".to_string(),
            description: None,
        };
        let ctx = TestContext;
        let result = op.execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["title"], "Test");
    }
}
