//! Set expression operation
//!
//! Evaluates a JavaScript expression and stores the result as a named variable
//! in the global JS context.

use crate::context::JsContext;
use crate::error::JsError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{Execute, ExecutionResult};
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
                ExecutionResult::Unlogged { value: json_result }
            }
            Err(e) => ExecutionResult::Failed {
                error: JsError::evaluation(e),
                log_entry: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_operations::Operation;

    #[test]
    fn test_get_name_with_name_field() {
        let expr = SetExpression {
            name: Some("x".to_string()),
            expression: None,
        };
        assert_eq!(expr.get_name().unwrap(), "x");
    }

    #[test]
    fn test_get_name_empty_returns_error() {
        let expr = SetExpression {
            name: Some(String::new()),
            expression: None,
        };
        assert!(expr.get_name().is_err());
        assert!(expr
            .get_name()
            .unwrap_err()
            .contains("'name' or 'key' parameter is required"));
    }

    #[test]
    fn test_get_name_none_returns_error() {
        let expr = SetExpression {
            name: None,
            expression: None,
        };
        assert!(expr.get_name().is_err());
    }

    #[test]
    fn test_get_expression_string() {
        let expr = SetExpression {
            name: None,
            expression: Some(Value::String("10 + 5".to_string())),
        };
        assert_eq!(expr.get_expression().unwrap(), "10 + 5");
    }

    #[test]
    fn test_get_expression_bool_true() {
        let expr = SetExpression {
            name: None,
            expression: Some(Value::Bool(true)),
        };
        assert_eq!(expr.get_expression().unwrap(), "true");
    }

    #[test]
    fn test_get_expression_bool_false() {
        let expr = SetExpression {
            name: None,
            expression: Some(Value::Bool(false)),
        };
        assert_eq!(expr.get_expression().unwrap(), "false");
    }

    #[test]
    fn test_get_expression_number() {
        let expr = SetExpression {
            name: None,
            expression: Some(serde_json::json!(42)),
        };
        assert_eq!(expr.get_expression().unwrap(), "42");
    }

    #[test]
    fn test_get_expression_null() {
        let expr = SetExpression {
            name: None,
            expression: Some(Value::Null),
        };
        assert_eq!(expr.get_expression().unwrap(), "null");
    }

    #[test]
    fn test_get_expression_array() {
        let expr = SetExpression {
            name: None,
            expression: Some(serde_json::json!([1, 2, 3])),
        };
        let result = expr.get_expression().unwrap();
        assert!(result.starts_with('('));
        assert!(result.ends_with(')'));
        assert!(result.contains("[1,2,3]"));
    }

    #[test]
    fn test_get_expression_object() {
        let expr = SetExpression {
            name: None,
            expression: Some(serde_json::json!({"a": 1})),
        };
        let result = expr.get_expression().unwrap();
        assert!(result.starts_with('('));
        assert!(result.ends_with(')'));
        assert!(result.contains("\"a\""));
    }

    #[test]
    fn test_get_expression_none_returns_error() {
        let expr = SetExpression {
            name: None,
            expression: None,
        };
        assert!(expr.get_expression().is_err());
        assert!(expr
            .get_expression()
            .unwrap_err()
            .contains("'expression' or 'value' parameter is required"));
    }

    #[test]
    fn test_operation_trait_verb() {
        let expr = SetExpression {
            name: None,
            expression: None,
        };
        assert_eq!(expr.verb(), "set");
    }

    #[test]
    fn test_operation_trait_noun() {
        let expr = SetExpression {
            name: None,
            expression: None,
        };
        assert_eq!(expr.noun(), "expression");
    }

    #[test]
    fn test_operation_trait_description() {
        let expr = SetExpression {
            name: None,
            expression: None,
        };
        assert_eq!(
            expr.description(),
            "Evaluate expression and store as variable"
        );
    }

    #[test]
    fn test_debug_impl() {
        let expr = SetExpression {
            name: Some("x".to_string()),
            expression: Some(Value::String("42".to_string())),
        };
        let debug = format!("{:?}", expr);
        assert!(debug.contains("SetExpression"));
        assert!(debug.contains("x"));
    }

    #[test]
    fn test_serde_deserialize_with_aliases() {
        // Test key/value aliases
        let json = serde_json::json!({"key": "x", "value": "42"});
        let expr: SetExpression = serde_json::from_value(json).unwrap();
        assert_eq!(expr.get_name().unwrap(), "x");
        assert_eq!(expr.get_expression().unwrap(), "42");
    }

    #[test]
    fn test_serde_deserialize_with_primary_fields() {
        let json = serde_json::json!({"name": "y", "expression": "10 + 5"});
        let expr: SetExpression = serde_json::from_value(json).unwrap();
        assert_eq!(expr.get_name().unwrap(), "y");
        assert_eq!(expr.get_expression().unwrap(), "10 + 5");
    }

    #[test]
    fn test_serde_serialize() {
        let expr = SetExpression {
            name: Some("z".to_string()),
            expression: Some(Value::String("99".to_string())),
        };
        let json = serde_json::to_value(&expr).unwrap();
        assert_eq!(json["name"], "z");
        assert_eq!(json["expression"], "99");
    }

    #[tokio::test]
    async fn test_execute_success() {
        let ctx = JsContext::new();
        let expr = SetExpression {
            name: Some("set_test_exec".to_string()),
            expression: Some(Value::String("100 + 1".to_string())),
        };
        let result = expr.execute(&ctx).await;
        let value = result.into_result().unwrap();
        assert_eq!(value, serde_json::json!(101));
    }

    #[tokio::test]
    async fn test_execute_missing_name() {
        let ctx = JsContext::new();
        let expr = SetExpression {
            name: None,
            expression: Some(Value::String("42".to_string())),
        };
        let result = expr.execute(&ctx).await;
        assert!(result.into_result().is_err());
    }

    #[tokio::test]
    async fn test_execute_missing_expression() {
        let ctx = JsContext::new();
        let expr = SetExpression {
            name: Some("valid_name".to_string()),
            expression: None,
        };
        let result = expr.execute(&ctx).await;
        assert!(result.into_result().is_err());
    }

    #[tokio::test]
    async fn test_execute_invalid_js() {
        let ctx = JsContext::new();
        let expr = SetExpression {
            name: Some("bad_js".to_string()),
            expression: Some(Value::String("this is not valid +++".to_string())),
        };
        let result = expr.execute(&ctx).await;
        assert!(result.into_result().is_err());
    }
}
