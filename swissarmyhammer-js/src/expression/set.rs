//! Set expression operation
//!
//! Evaluates a JavaScript expression and stores the result as a named variable
//! in the global JS context.

use crate::context::JsContext;
use crate::error::JsError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{ExecutionResult, Execute};
use swissarmyhammer_operations_macros::operation;

/// Evaluate a JavaScript expression and store the result as a named variable.
///
/// After storing, all new/modified JS globals are captured back into the
/// tracked context automatically.
#[operation(
    verb = "set",
    noun = "expression",
    description = "Evaluate expression and store as variable"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct SetExpression {
    /// Name of the variable to store the result
    #[serde(alias = "key")]
    pub name: Option<String>,

    /// JavaScript expression to evaluate
    #[serde(alias = "value")]
    pub expression: Option<Value>,
}

impl SetExpression {
    /// Get the variable name, checking both name and key fields
    pub fn get_name(&self) -> Result<String, String> {
        self.name
            .clone()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "Either 'name' or 'key' parameter is required".to_string())
    }

    /// Get the expression string, converting JSON values to JS expression strings
    pub fn get_expression(&self) -> Result<String, String> {
        let value = self
            .expression
            .clone()
            .ok_or_else(|| "Either 'expression' or 'value' parameter is required".to_string())?;

        Ok(match value {
            Value::String(s) => s,
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::Null => "null".to_string(),
            Value::Array(_) | Value::Object(_) => {
                // For arrays and objects, wrap in parens so they parse as expressions
                format!(
                    "({})",
                    serde_json::to_string(&value)
                        .map_err(|e| format!("Failed to serialize: {}", e))?
                )
            }
        })
    }
}

#[async_trait]
impl Execute<JsContext, JsError> for SetExpression {
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

        let expression = match self.get_expression() {
            Ok(e) => e,
            Err(e) => {
                return ExecutionResult::Failed {
                    error: JsError::evaluation(e),
                    log_entry: None,
                }
            }
        };

        tracing::debug!("JS set: name='{}', expression='{}'", name, expression);

        match ctx.state().set(&name, &expression).await {
            Ok(json_result) => {
                tracing::info!("JS set '{}' = {:?}", name, json_result);
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
