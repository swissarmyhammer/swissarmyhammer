//! Get expression operation
//!
//! Retrieves a variable's value from the global JS context by evaluating
//! the variable name as a JS expression.

use crate::context::JsContext;
use crate::error::JsError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{ExecutionResult, Execute};
use swissarmyhammer_operations_macros::operation;

/// Retrieve the value of a stored variable from the global JS context.
///
/// The name is evaluated as a JS expression, so simple variable names
/// like `"x"` return the variable's value, while expressions like
/// `"x * 2"` are also supported.
#[operation(
    verb = "get",
    noun = "expression",
    description = "Retrieve variable value"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct GetExpression {
    /// Name of the variable to retrieve
    #[serde(alias = "key")]
    pub name: Option<String>,
}

impl GetExpression {
    /// Get the variable name
    pub fn get_name(&self) -> Result<String, String> {
        self.name
            .clone()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "Either 'name' or 'key' parameter is required".to_string())
    }
}

#[async_trait]
impl Execute<JsContext, JsError> for GetExpression {
    async fn execute(&self, ctx: &JsContext) -> ExecutionResult<Value, JsError> {
        let name = match self.get_name() {
            Ok(n) => n,
            Err(e) => {
                return ExecutionResult::Failed {
                    error: JsError::evaluation(e),
                    log_entry: None,
                }
            }
        };

        tracing::debug!("JS get: name='{}'", name);

        match ctx.state().get(&name).await {
            Ok(json_result) => {
                tracing::debug!("JS get '{}' = {:?}", name, json_result);
                ExecutionResult::Unlogged {
                    value: json_result,
                }
            }
            Err(e) => ExecutionResult::Failed {
                error: JsError::evaluation(e),
                log_entry: None,
            },
        }
    }
}
