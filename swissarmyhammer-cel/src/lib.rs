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
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Global CEL state containing both the context and variable tracking
struct GlobalCelState {
    context: Context<'static>,
    variables: HashMap<String, CelValue>,
}

impl GlobalCelState {
    fn new() -> Self {
        Self {
            context: Context::default(),
            variables: HashMap::new(),
        }
    }
}

/// Process-global CEL state shared across all components
static GLOBAL_CEL_STATE: Lazy<RwLock<GlobalCelState>> =
    Lazy::new(|| RwLock::new(GlobalCelState::new()));

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
        let mut state = GLOBAL_CEL_STATE
            .write()
            .map_err(|e| format!("Lock error: {}", e))?;

        // Evaluate the expression in the current context
        let value = Self::evaluate_expression_internal(expression, &state.context);

        // Store the result as a variable in both context and tracking map
        state
            .context
            .add_variable(name, value.clone())
            .map_err(|e| format!("Failed to add variable: {}", e))?;

        state.variables.insert(name.to_string(), value.clone());

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
        let state = GLOBAL_CEL_STATE
            .read()
            .map_err(|e| format!("Lock error: {}", e))?;
        Ok(Self::evaluate_expression_internal(
            expression,
            &state.context,
        ))
    }

    /// Copy all global variables to a target CEL context
    ///
    /// This enables context stacking where workflow-specific variables are layered
    /// on top of global variables, allowing expressions to access both.
    ///
    /// # Arguments
    ///
    /// * `target` - The CEL context to copy variables into
    ///
    /// # Returns
    ///
    /// Returns Ok(()) on success, or an error message if copying fails
    pub fn copy_to_context(&self, target: &mut Context) -> Result<(), String> {
        let state = GLOBAL_CEL_STATE
            .read()
            .map_err(|e| format!("Lock error: {}", e))?;

        // Copy all tracked variables to the target context
        for (name, value) in &state.variables {
            target
                .add_variable(name, value.clone())
                .map_err(|e| format!("Failed to copy variable '{}': {}", name, e))?;
        }

        Ok(())
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
            cel_value_to_json(&CelValue::Float(3.5)),
            serde_json::json!(3.5)
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

    #[test]
    fn test_copy_to_context() {
        let state = CelState::global();

        // Set some global variables
        state.set("global_var", "100").unwrap();
        state.set("abort", "true").unwrap();

        // Create a new context and copy globals to it
        let mut new_context = cel_interpreter::Context::default();
        state.copy_to_context(&mut new_context).unwrap();

        // Verify global variables are accessible in new context
        let program = cel_interpreter::Program::compile("global_var").unwrap();
        let result = program.execute(&new_context).unwrap();
        assert_eq!(cel_value_to_json(&result), serde_json::json!(100));

        let program = cel_interpreter::Program::compile("abort").unwrap();
        let result = program.execute(&new_context).unwrap();
        assert_eq!(cel_value_to_json(&result), serde_json::json!(true));
    }

    #[test]
    fn test_context_stacking_with_local_variables() {
        let state = CelState::global();

        // Set global variable
        state.set("global_count", "5").unwrap();

        // Create new context with global variables
        let mut stacked_context = cel_interpreter::Context::default();
        state.copy_to_context(&mut stacked_context).unwrap();

        // Add local variables on top
        stacked_context
            .add_variable("local_count", CelValue::Int(10))
            .unwrap();

        // Verify both global and local variables are accessible
        let program = cel_interpreter::Program::compile("global_count + local_count").unwrap();
        let result = program.execute(&stacked_context).unwrap();
        assert_eq!(cel_value_to_json(&result), serde_json::json!(15));
    }

    #[test]
    fn test_context_stacking_local_overrides_global() {
        let state = CelState::global();

        // Set global variable
        state.set("shared_var", "\"global\"").unwrap();

        // Create new context with global variables
        let mut stacked_context = cel_interpreter::Context::default();
        state.copy_to_context(&mut stacked_context).unwrap();

        // Override with local variable (same name)
        stacked_context
            .add_variable(
                "shared_var",
                CelValue::String(Arc::new("local".to_string())),
            )
            .unwrap();

        // Verify local value takes precedence
        let program = cel_interpreter::Program::compile("shared_var").unwrap();
        let result = program.execute(&stacked_context).unwrap();
        assert_eq!(cel_value_to_json(&result), serde_json::json!("local"));
    }

    #[test]
    fn test_set_boolean_true() {
        let state = CelState::global();

        // Set a boolean true value
        let result = state.set("flag_enabled", "true");
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(cel_value_to_json(&value), serde_json::json!(true));

        // Verify we can read it back
        let result = state.get("flag_enabled");
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(cel_value_to_json(&value), serde_json::json!(true));

        // Verify we can use it in a boolean expression
        let result = state.get("flag_enabled == true");
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(cel_value_to_json(&value), serde_json::json!(true));
    }

    #[test]
    fn test_set_boolean_false() {
        let state = CelState::global();

        // Set a boolean false value
        let result = state.set("flag_disabled", "false");
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(cel_value_to_json(&value), serde_json::json!(false));

        // Verify we can read it back
        let result = state.get("flag_disabled");
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(cel_value_to_json(&value), serde_json::json!(false));

        // Verify we can use it in a boolean expression
        let result = state.get("flag_disabled == false");
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(cel_value_to_json(&value), serde_json::json!(true));
    }

    #[test]
    fn test_boolean_expressions() {
        let state = CelState::global();

        // Set boolean variables
        state.set("is_ready", "true").unwrap();
        state.set("is_error", "false").unwrap();
        state.set("has_permission", "true").unwrap();

        // Test direct boolean evaluation
        let result = state.get("is_ready").unwrap();
        assert_eq!(cel_value_to_json(&result), serde_json::json!(true));

        // Test equality comparisons
        let result = state.get("is_ready == true").unwrap();
        assert_eq!(cel_value_to_json(&result), serde_json::json!(true));

        let result = state.get("is_error == false").unwrap();
        assert_eq!(cel_value_to_json(&result), serde_json::json!(true));

        // Test boolean AND
        let result = state.get("is_ready && has_permission").unwrap();
        assert_eq!(cel_value_to_json(&result), serde_json::json!(true));

        // Test boolean OR
        let result = state.get("is_error || has_permission").unwrap();
        assert_eq!(cel_value_to_json(&result), serde_json::json!(true));

        // Test boolean NOT
        let result = state.get("!is_error").unwrap();
        assert_eq!(cel_value_to_json(&result), serde_json::json!(true));

        // Test complex boolean expression
        let result = state
            .get("is_ready && !is_error && has_permission")
            .unwrap();
        assert_eq!(cel_value_to_json(&result), serde_json::json!(true));
    }

    #[test]
    fn test_boolean_in_stacked_context() {
        let state = CelState::global();

        // Set global boolean variables
        state.set("global_enabled", "true").unwrap();
        state.set("global_disabled", "false").unwrap();

        // Create new context with global variables
        let mut stacked_context = cel_interpreter::Context::default();
        state.copy_to_context(&mut stacked_context).unwrap();

        // Add local boolean variable
        stacked_context
            .add_variable("local_flag", CelValue::Bool(true))
            .unwrap();

        // Verify global booleans are accessible
        let program = cel_interpreter::Program::compile("global_enabled == true").unwrap();
        let result = program.execute(&stacked_context).unwrap();
        assert_eq!(cel_value_to_json(&result), serde_json::json!(true));

        let program = cel_interpreter::Program::compile("global_disabled == false").unwrap();
        let result = program.execute(&stacked_context).unwrap();
        assert_eq!(cel_value_to_json(&result), serde_json::json!(true));

        // Verify local boolean is accessible
        let program = cel_interpreter::Program::compile("local_flag").unwrap();
        let result = program.execute(&stacked_context).unwrap();
        assert_eq!(cel_value_to_json(&result), serde_json::json!(true));

        // Verify mixed boolean expression
        let program =
            cel_interpreter::Program::compile("global_enabled && local_flag && !global_disabled")
                .unwrap();
        let result = program.execute(&stacked_context).unwrap();
        assert_eq!(cel_value_to_json(&result), serde_json::json!(true));
    }
}
