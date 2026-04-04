//! Get expression operation
//!
//! Retrieves a variable's value from the global JS context by evaluating
//! the variable name as a JS expression.

use crate::context::JsContext;
use crate::error::JsError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{Execute, ExecutionResult};
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
    fn test_get_name_with_name() {
        let expr = GetExpression {
            name: Some("x".to_string()),
        };
        assert_eq!(expr.get_name().unwrap(), "x");
    }

    #[test]
    fn test_get_name_empty_returns_error() {
        let expr = GetExpression {
            name: Some(String::new()),
        };
        assert!(expr.get_name().is_err());
        assert!(expr
            .get_name()
            .unwrap_err()
            .contains("'name' or 'key' parameter is required"));
    }

    #[test]
    fn test_get_name_none_returns_error() {
        let expr = GetExpression { name: None };
        assert!(expr.get_name().is_err());
    }

    #[test]
    fn test_operation_trait_verb() {
        let expr = GetExpression { name: None };
        assert_eq!(expr.verb(), "get");
    }

    #[test]
    fn test_operation_trait_noun() {
        let expr = GetExpression { name: None };
        assert_eq!(expr.noun(), "expression");
    }

    #[test]
    fn test_operation_trait_description() {
        let expr = GetExpression { name: None };
        assert_eq!(expr.description(), "Retrieve variable value");
    }

    #[test]
    fn test_debug_impl() {
        let expr = GetExpression {
            name: Some("myvar".to_string()),
        };
        let debug = format!("{:?}", expr);
        assert!(debug.contains("GetExpression"));
        assert!(debug.contains("myvar"));
    }

    #[test]
    fn test_serde_deserialize_with_key_alias() {
        let json = serde_json::json!({"key": "counter"});
        let expr: GetExpression = serde_json::from_value(json).unwrap();
        assert_eq!(expr.get_name().unwrap(), "counter");
    }

    #[test]
    fn test_serde_deserialize_with_name() {
        let json = serde_json::json!({"name": "total"});
        let expr: GetExpression = serde_json::from_value(json).unwrap();
        assert_eq!(expr.get_name().unwrap(), "total");
    }

    #[test]
    fn test_serde_serialize() {
        let expr = GetExpression {
            name: Some("abc".to_string()),
        };
        let json = serde_json::to_value(&expr).unwrap();
        assert_eq!(json["name"], "abc");
    }

    #[tokio::test]
    async fn test_execute_success() {
        let ctx = JsContext::new();
        // First set a variable so we can get it
        let _ = ctx.state().set("get_test_var", "777").await;

        let expr = GetExpression {
            name: Some("get_test_var".to_string()),
        };
        let result = expr.execute(&ctx).await;
        let value = result.into_result().unwrap();
        assert_eq!(value, serde_json::json!(777));
    }

    #[tokio::test]
    async fn test_execute_missing_name() {
        let ctx = JsContext::new();
        let expr = GetExpression { name: None };
        let result = expr.execute(&ctx).await;
        assert!(result.into_result().is_err());
    }

    #[tokio::test]
    async fn test_execute_undefined_variable() {
        let ctx = JsContext::new();
        let expr = GetExpression {
            name: Some("totally_nonexistent_get_test_abc".to_string()),
        };
        let result = expr.execute(&ctx).await;
        assert!(result.into_result().is_err());
    }

    #[tokio::test]
    async fn test_execute_expression_evaluation() {
        let ctx = JsContext::new();
        let _ = ctx.state().set("get_expr_a", "5").await;

        let expr = GetExpression {
            name: Some("get_expr_a + 10".to_string()),
        };
        let result = expr.execute(&ctx).await;
        let value = result.into_result().unwrap();
        assert_eq!(value, serde_json::json!(15));
    }
}
