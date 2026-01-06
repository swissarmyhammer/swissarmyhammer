//! CEL (Common Expression Language) state management for SwissArmyHammer
//!
//! This crate provides process-global CEL state management that can be shared
//! across different components (MCP tools, workflows, etc.) within the same process.
//!
//! # Architecture
//!
//! - **Process-Global State**: Single CEL Context shared by all components
//! - **Thread-Safe**: Protected by RwLock for concurrent access
//! - **In-Memory Only**: No persistence, state is lost when process terminates
//!
//! # Example
//!
//! ```rust
//! use swissarmyhammer_cel::CelState;
//!
//! let state = CelState::global();
//!
//! // Set a variable
//! let result = state.set("x", "10 + 5");
//! assert!(result.is_ok());
//!
//! // Get/evaluate an expression
//! let result = state.get("x * 2");
//! assert!(result.is_ok());
//! ```

use cel_interpreter::{Context, Program, Value as CelValue};
use once_cell::sync::Lazy;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Process-global CEL context shared across all components
static GLOBAL_CEL_CONTEXT: Lazy<RwLock<Context<'static>>> =
    Lazy::new(|| RwLock::new(Context::default()));

/// CEL state manager providing thread-safe access to a process-global CEL context
#[derive(Clone)]
pub struct CelState;

impl CelState {
    /// Get the global CEL state instance
    pub fn global() -> Self {
        Self
    }

    /// Evaluate a CEL expression and store the result as a named variable
    ///
    /// # Arguments
    ///
    /// * `name` - Variable name to store the result
    /// * `expression` - CEL expression to evaluate
    ///
    /// # Returns
    ///
    /// Returns the computed CEL value, or a CEL String containing an error message
    pub fn set(&self, name: &str, expression: &str) -> Result<CelValue, String> {
        let mut context = self
            .write_context()
            .map_err(|e| format!("Lock error: {}", e))?;

        // Evaluate the expression in the current context
        let value = Self::evaluate_expression_internal(expression, &context);

        // Store the result as a variable
        context
            .add_variable(name, value.clone())
            .map_err(|e| format!("Failed to add variable: {}", e))?;

        Ok(value)
    }

    /// Evaluate a CEL expression in the current context without storing it
    ///
    /// # Arguments
    ///
    /// * `expression` - CEL expression to evaluate
    ///
    /// # Returns
    ///
    /// Returns the computed CEL value, or a CEL String containing an error message
    pub fn get(&self, expression: &str) -> Result<CelValue, String> {
        let context = self
            .read_context()
            .map_err(|e| format!("Lock error: {}", e))?;
        Ok(Self::evaluate_expression_internal(expression, &context))
    }

    /// Get a read lock on the CEL context
    fn read_context(&self) -> Result<RwLockReadGuard<'static, Context<'static>>, String> {
        GLOBAL_CEL_CONTEXT
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))
    }

    /// Get a write lock on the CEL context
    fn write_context(&self) -> Result<RwLockWriteGuard<'static, Context<'static>>, String> {
        GLOBAL_CEL_CONTEXT
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))
    }

    /// Internal method to compile and execute a CEL expression
    ///
    /// Returns the result or an error as a CEL String value
    fn evaluate_expression_internal(expression: &str, context: &Context<'_>) -> CelValue {
        // Compile the expression
        let program = match Program::compile(expression) {
            Ok(p) => p,
            Err(e) => {
                return CelValue::String(Arc::new(format!("CEL compilation error: {}", e)));
            }
        };

        // Execute the program
        match program.execute(context) {
            Ok(value) => value,
            Err(e) => CelValue::String(Arc::new(format!("CEL execution error: {}", e))),
        }
    }
}

impl Default for CelState {
    fn default() -> Self {
        Self::global()
    }
}

/// Convert a CEL Value to a JSON Value
pub fn cel_value_to_json(value: &CelValue) -> serde_json::Value {
    match value {
        CelValue::Int(i) => serde_json::json!(i),
        CelValue::UInt(u) => serde_json::json!(u),
        CelValue::Float(f) => serde_json::json!(f),
        CelValue::Bool(b) => serde_json::json!(b),
        CelValue::String(s) => serde_json::json!(s.as_ref()),
        CelValue::Bytes(b) => {
            // Convert bytes to base64 string for JSON compatibility
            serde_json::json!(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                b.as_ref()
            ))
        }
        CelValue::List(list) => {
            let json_list: Vec<serde_json::Value> = list.iter().map(cel_value_to_json).collect();
            serde_json::json!(json_list)
        }
        CelValue::Map(map) => {
            let json_map: serde_json::Map<String, serde_json::Value> = map
                .map
                .iter()
                .map(|(k, v)| {
                    let key_string = match k {
                        cel_interpreter::objects::Key::Int(i) => i.to_string(),
                        cel_interpreter::objects::Key::Uint(u) => u.to_string(),
                        cel_interpreter::objects::Key::Bool(b) => b.to_string(),
                        cel_interpreter::objects::Key::String(s) => s.to_string(),
                    };
                    (key_string, cel_value_to_json(v))
                })
                .collect();
            serde_json::json!(json_map)
        }
        CelValue::Null => serde_json::Value::Null,
        CelValue::Timestamp(ts) => {
            // Convert timestamp to RFC3339 string
            serde_json::json!(ts.to_rfc3339())
        }
        CelValue::Duration(d) => {
            // Convert duration to seconds
            serde_json::json!(d.as_seconds_f64())
        }
        CelValue::Function(name, _) => {
            // Functions can't be easily serialized, return function name as string
            serde_json::json!(format!("<function: {}>", name))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cel_state_set_and_get() {
        let state = CelState::global();

        // Set a variable
        let result = state.set("test_var", "42");
        assert!(result.is_ok());

        // Get the variable
        let result = state.get("test_var");
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(cel_value_to_json(&value), serde_json::json!(42));
    }

    #[test]
    fn test_cel_state_expression_evaluation() {
        let state = CelState::global();

        // Set a variable with an expression
        let result = state.set("calc", "10 + 5 * 2");
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(cel_value_to_json(&value), serde_json::json!(20));

        // Evaluate an expression using the variable
        let result = state.get("calc * 2");
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(cel_value_to_json(&value), serde_json::json!(40));
    }

    #[test]
    fn test_cel_state_error_handling() {
        let state = CelState::global();

        // Invalid expression should return error as string
        let result = state.get("2 +");
        assert!(result.is_ok());
        let value = result.unwrap();
        match value {
            CelValue::String(s) => assert!(s.contains("compilation error")),
            _ => panic!("Expected error string"),
        }
    }

    #[test]
    fn test_cel_value_to_json_primitives() {
        assert_eq!(cel_value_to_json(&CelValue::Int(42)), serde_json::json!(42));
        assert_eq!(
            cel_value_to_json(&CelValue::UInt(100)),
            serde_json::json!(100)
        );
        assert_eq!(
            cel_value_to_json(&CelValue::Float(3.14)),
            serde_json::json!(3.14)
        );
        assert_eq!(
            cel_value_to_json(&CelValue::Bool(true)),
            serde_json::json!(true)
        );
        assert_eq!(
            cel_value_to_json(&CelValue::String(Arc::new("hello".to_string()))),
            serde_json::json!("hello")
        );
        assert_eq!(cel_value_to_json(&CelValue::Null), serde_json::Value::Null);
    }

    #[test]
    fn test_cel_value_to_json_list() {
        let list = vec![CelValue::Int(1), CelValue::Int(2), CelValue::Int(3)];
        let cel_list = CelValue::List(list.into());
        assert_eq!(cel_value_to_json(&cel_list), serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn test_cel_value_to_json_map() {
        let mut map = std::collections::HashMap::new();
        map.insert(
            "name".to_string(),
            CelValue::String(Arc::new("test".to_string())),
        );
        map.insert("value".to_string(), CelValue::Int(42));
        let cel_map = CelValue::Map(map.into());
        let json = cel_value_to_json(&cel_map);
        assert_eq!(json["name"], "test");
        assert_eq!(json["value"], 42);
    }
}
