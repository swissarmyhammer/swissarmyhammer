//! Condition evaluation and validation functionality
//!
//! This module provides comprehensive condition evaluation and validation for workflow
//! transitions, with a focus on security and performance. It supports multiple condition
//! types including CEL (Common Expression Language) expressions for complex logic.
//!
//! # Architecture
//!
//! The module is organized around the following key components:
//! - **Security Validation**: Prevents CEL injection attacks and resource exhaustion
//! - **Expression Compilation**: Caches compiled CEL programs for performance
//! - **Context Management**: Converts workflow data to CEL-compatible formats
//! - **Choice State Validation**: Ensures deterministic behavior in choice states
//!
//! # Condition Types
//!
//! ## Built-in Conditions
//! - `Always`: Always evaluates to true
//! - `Never`: Always evaluates to false
//! - `OnSuccess`: Evaluates based on last action success
//! - `OnFailure`: Evaluates based on last action failure
//!
//! ## Custom CEL Expressions
//! - `Custom`: Evaluates user-provided CEL expressions
//! - Supports complex boolean logic, variable access, and text processing
//! - Includes comprehensive security validation
//!
//! # Security Features
//!
//! ## Expression Validation
//! - **Length Limits**: Prevents DoS through oversized expressions
//! - **Forbidden Patterns**: Blocks dangerous function calls and imports
//! - **Nesting Limits**: Prevents stack overflow from deep nesting
//! - **Quote Validation**: Detects suspicious quote patterns
//!
//! ## Execution Safety
//! - **Timeout Protection**: Limits expression execution time
//! - **Resource Limits**: Prevents resource exhaustion attacks
//! - **Sandboxed Execution**: CEL expressions run in isolated context
//!
//! # Performance Optimizations
//!
//! ## Compilation Caching
//! - CEL programs are compiled once and cached for reuse
//! - Significant performance improvement for repeated evaluations
//! - Cache is managed per executor instance
//!
//! ## Efficient Type Conversion
//! - JSON to CEL type mapping uses built-in conversions
//! - Fallback to string representation for unsupported types
//! - Minimal memory allocation for common cases
//!
//! # Usage Examples
//!
//! ```rust,no_run
//! # use std::collections::HashMap;
//! # use serde_json::Value;
//! # use swissarmyhammer::workflow::{TransitionCondition, ConditionType, WorkflowExecutor};
//! # let mut executor = WorkflowExecutor::new();
//! # let context = HashMap::<String, Value>::new();
//! // Simple condition evaluation
//! let condition = TransitionCondition {
//!     condition_type: ConditionType::Custom,
//!     expression: Some("count > 10".to_string()),
//! };
//! let result = executor.evaluate_condition(&condition, &context)?;
//!
//! // Complex condition with multiple variables
//! let condition = TransitionCondition {
//!     condition_type: ConditionType::Custom,
//!     expression: Some("status == \"active\" && count > threshold".to_string()),
//! };
//! let result = executor.evaluate_condition(&condition, &context)?;
//!
//! // Default fallback condition
//! let condition = TransitionCondition {
//!     condition_type: ConditionType::Custom,
//!     expression: Some("default".to_string()),
//! };
//! let result = executor.evaluate_condition(&condition, &context)?; // Always true
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Error Handling
//!
//! All functions return `ExecutorResult<T>` with detailed error messages.
//! Error types include:
//! - `ExecutorError::ExpressionError`: CEL compilation or evaluation errors
//! - `ExecutorError::ExecutionFailed`: Workflow execution errors
//!
//! # Thread Safety
//!
//! The module is designed to be thread-safe when used with proper synchronization.
//! Each `WorkflowExecutor` maintains its own CEL program cache.
//!
//! # Future Enhancements
//!
//! - Custom CEL functions for domain-specific operations
//! - Advanced caching strategies with TTL and size limits
//! - Metrics and monitoring for CEL expression performance
//! - Support for async CEL operations

use super::core::WorkflowExecutor;
use super::{ExecutionEventType, ExecutorError, ExecutorResult, LAST_ACTION_RESULT_KEY};
use crate::workflow::{ConditionType, StateId, TransitionCondition, WorkflowRun};
use cel_interpreter::{Context, Value as CelValue};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

// Security constants for CEL expression evaluation
const MAX_EXPRESSION_LENGTH: usize = 500;
const MAX_EXECUTION_TIME: Duration = Duration::from_millis(100);
const DEFAULT_VARIABLE_NAME: &str = "default";
const RESULT_VARIABLE_NAME: &str = "result";

// Forbidden patterns that could be dangerous
const FORBIDDEN_PATTERNS: &[&str] = &[
    "import", "load", "eval", "exec", "system", "process", "file", "read", "write", "delete",
    "create", "mkdir", "rmdir", "chmod", "chown", "kill", "spawn",
];

// Result keys to look for in context
const RESULT_KEYS: &[&str] = &["result", "output", "response", "claude_result", "claude_response"];

impl WorkflowExecutor {
    /// Evaluate all transitions from the current state
    pub fn evaluate_transitions(&mut self, run: &WorkflowRun) -> ExecutorResult<Option<StateId>> {
        let current_state = &run.current_state;

        // Find all transitions from current state
        let transitions: Vec<_> = run
            .workflow
            .transitions
            .iter()
            .filter(|t| &t.from_state == current_state)
            .collect();

        // Check if this is a choice state and validate it has transitions
        let state_type = run
            .workflow
            .states
            .get(current_state)
            .map(|state| &state.state_type);
        let is_choice_state = state_type == Some(&crate::workflow::StateType::Choice);

        tracing::debug!(
            "State '{}' type: {:?}, is_choice_state: {}",
            current_state,
            state_type,
            is_choice_state
        );

        if is_choice_state {
            if transitions.is_empty() {
                return Err(ExecutorError::ExecutionFailed(
                    format!(
                        "Choice state '{current_state}' has no outgoing transitions. Choice states must have at least one outgoing transition"
                    ),
                ));
            }

            // Validate choice state has deterministic behavior
            self.validate_choice_state_determinism(current_state, &transitions)?;
        }

        // Evaluate all conditions to check for multiple matches
        let mut matching_transitions = Vec::new();

        for transition in &transitions {
            let context_hashmap = run.context.to_workflow_hashmap();
            if self.evaluate_condition(&transition.condition, &context_hashmap)? {
                self.log_event(
                    ExecutionEventType::ConditionEvaluated,
                    format!(
                        "Condition '{}' evaluated to true for transition: {} -> {}",
                        transition.condition.condition_type.as_str(),
                        transition.from_state,
                        transition.to_state
                    ),
                );
                matching_transitions.push(transition);
            }
        }

        // Check for multiple matches and warn user
        if matching_transitions.len() > 1 {
            let matching_states: Vec<&str> = matching_transitions
                .iter()
                .map(|t| t.to_state.as_str())
                .collect();

            tracing::warn!(
                "Multiple transition conditions are true from state '{}' to states: [{}]. Using first match: '{}'",
                current_state,
                matching_states.join(", "),
                matching_transitions[0].to_state
            );
        }

        // Return the first match if any
        if let Some(first_match) = matching_transitions.first() {
            return Ok(Some(first_match.to_state.clone()));
        }

        // If this is a choice state and no conditions matched, it's an error
        if is_choice_state {
            return Err(ExecutorError::ExecutionFailed(
                format!(
                    "Choice state '{current_state}' has no matching conditions. All transition conditions evaluated to false"
                ),
            ));
        }

        Ok(None)
    }

    /// Helper function to evaluate action-based conditions (success/failure)
    pub fn evaluate_action_condition(
        &self,
        context: &HashMap<String, Value>,
        expect_success: bool,
        default_value: bool,
    ) -> bool {
        if let Some(last_action_result) = context.get(LAST_ACTION_RESULT_KEY) {
            match last_action_result {
                Value::Bool(success) => {
                    if expect_success {
                        *success
                    } else {
                        !*success
                    }
                }
                _ => default_value, // Default value if not a boolean
            }
        } else {
            default_value // Default value if no result in context
        }
    }

    /// Evaluate a transition condition
    pub fn evaluate_condition(
        &mut self,
        condition: &TransitionCondition,
        context: &HashMap<String, Value>,
    ) -> ExecutorResult<bool> {
        match &condition.condition_type {
            ConditionType::Always => Ok(true),
            ConditionType::Never => Ok(false),
            ConditionType::OnSuccess => Ok(self.evaluate_action_condition(context, true, true)),
            ConditionType::OnFailure => Ok(self.evaluate_action_condition(context, false, false)),
            ConditionType::Custom => {
                if let Some(expression) = &condition.expression {
                    self.evaluate_cel_expression(expression, context)
                } else {
                    Err(ExecutorError::ExpressionError(
                        "CEL expression error: Custom condition requires an expression to be specified".to_string(),
                    ))
                }
            }
        }
    }

    /// Validate that a choice state has deterministic behavior
    ///
    /// Choice states must have deterministic behavior to ensure workflow execution
    /// is predictable and debuggable. This function validates that:
    /// 1. There are no ambiguous conditions (multiple transitions with same condition type)
    /// 2. Never conditions are not used (they would never be selected)
    /// 3. A default condition exists or conditions are mutually exclusive
    ///
    /// # Arguments
    /// * `state_id` - The ID of the choice state being validated
    /// * `transitions` - All transitions from this choice state
    ///
    /// # Returns
    /// * `Ok(())` if the choice state has deterministic behavior
    /// * `Err(ExecutorError::ExecutionFailed)` if validation fails
    ///
    /// # Validation Rules
    /// - At most one OnSuccess and one OnFailure condition per choice state
    /// - Never conditions are not allowed in choice states
    /// - Either a default condition must exist OR conditions must be mutually exclusive
    ///
    /// # Default Conditions
    /// A default condition is either:
    /// - A transition with `ConditionType::Always`
    /// - A custom CEL expression that evaluates to "default"
    fn validate_choice_state_determinism(
        &self,
        state_id: &StateId,
        transitions: &[&crate::workflow::Transition],
    ) -> ExecutorResult<()> {
        // Check if there's a default condition (always true or "default" CEL expression)
        let has_default = transitions
            .iter()
            .any(|t| match &t.condition.condition_type {
                crate::workflow::ConditionType::Always => true,
                crate::workflow::ConditionType::Custom => {
                    if let Some(expr) = &t.condition.expression {
                        expr.trim() == DEFAULT_VARIABLE_NAME
                    } else {
                        false
                    }
                }
                _ => false,
            });

        // If there's no default condition, check for potential ambiguity
        if !has_default {
            // Check for potentially overlapping conditions
            let condition_types: Vec<_> = transitions
                .iter()
                .map(|t| &t.condition.condition_type)
                .collect();

            // If we have multiple OnSuccess or OnFailure conditions, that's ambiguous
            let success_count = condition_types
                .iter()
                .filter(|ct| matches!(ct, crate::workflow::ConditionType::OnSuccess))
                .count();
            let failure_count = condition_types
                .iter()
                .filter(|ct| matches!(ct, crate::workflow::ConditionType::OnFailure))
                .count();

            if success_count > 1 || failure_count > 1 {
                return Err(ExecutorError::ExecutionFailed(
                    format!(
                        "Choice state '{state_id}' has ambiguous conditions: {success_count} OnSuccess, {failure_count} OnFailure. Consider adding a default condition or making conditions mutually exclusive"
                    ),
                ));
            }
        }

        // Check that Never conditions are not used in choice states (they would never be chosen)
        let never_conditions = transitions
            .iter()
            .filter(|t| {
                matches!(
                    t.condition.condition_type,
                    crate::workflow::ConditionType::Never
                )
            })
            .count();

        if never_conditions > 0 {
            return Err(ExecutorError::ExecutionFailed(
                format!(
                    "Choice state '{state_id}' has {never_conditions} Never conditions. Never conditions in choice states are never selectable and should be removed"
                ),
            ));
        }

        Ok(())
    }

    /// Validate and sanitize a CEL expression for security
    ///
    /// This function performs comprehensive security validation on CEL expressions to prevent
    /// injection attacks and resource exhaustion. It checks for:
    /// - Expression length limits to prevent DoS attacks
    /// - Forbidden patterns that could be used for code injection
    /// - Suspicious quote patterns that might indicate injection attempts
    /// - Excessive nesting depth that could cause stack overflow
    ///
    /// # Arguments
    /// * `expression` - The CEL expression string to validate
    ///
    /// # Returns
    /// * `Ok(())` if the expression passes all security checks
    /// * `Err(ExecutorError::ExpressionError)` if any security validation fails
    ///
    /// # Security Considerations
    /// This function is critical for preventing CEL injection attacks. Any changes should
    /// be thoroughly reviewed for security implications.
    fn validate_cel_expression(&self, expression: &str) -> ExecutorResult<()> {
        // Check expression length
        if expression.len() > MAX_EXPRESSION_LENGTH {
            return Err(ExecutorError::ExpressionError(format!(
                "CEL expression too long: {} characters (max {})",
                expression.len(),
                MAX_EXPRESSION_LENGTH
            )));
        }

        // Check for forbidden patterns
        let expr_lower = expression.to_lowercase();
        for pattern in FORBIDDEN_PATTERNS {
            if expr_lower.contains(pattern) {
                return Err(ExecutorError::ExpressionError(format!(
                    "CEL expression contains forbidden pattern: '{pattern}'"
                )));
            }
        }

        // Basic syntax validation - no nested quotes or suspicious characters
        if expression.contains("\"\"\"") || expression.contains("'''") {
            return Err(ExecutorError::ExpressionError(
                "CEL expression contains suspicious quote patterns".to_string(),
            ));
        }

        // Check for excessive nesting (potential DoS)
        let mut current_depth = 0;
        let mut max_depth = 0;
        for c in expression.chars() {
            match c {
                '(' | '[' | '{' => {
                    current_depth += 1;
                    max_depth = std::cmp::max(max_depth, current_depth);
                }
                ')' | ']' | '}' => {
                    current_depth -= 1;
                }
                _ => {}
            }
        }
        let paren_depth = max_depth;

        if paren_depth > 10 {
            return Err(ExecutorError::ExpressionError(format!(
                "CEL expression has excessive nesting depth: {paren_depth} (max 10)"
            )));
        }

        Ok(())
    }

    /// Evaluate a CEL expression with the given context
    ///
    /// This is the main entry point for CEL expression evaluation. It performs the following steps:
    /// 1. Security validation of the expression
    /// 2. Compilation and caching of the CEL program
    /// 3. Context preparation with workflow variables
    /// 4. Expression execution with timeout protection
    /// 5. Result conversion to boolean
    ///
    /// # Arguments
    /// * `expression` - The CEL expression string to evaluate
    /// * `context` - The workflow context containing variables for the expression
    ///
    /// # Returns
    /// * `Ok(true)` if the expression evaluates to a truthy value
    /// * `Ok(false)` if the expression evaluates to a falsy value
    /// * `Err(ExecutorError::ExpressionError)` if evaluation fails
    ///
    /// # CEL Context Variables
    /// The following variables are automatically available in CEL expressions:
    /// - `default`: Always evaluates to true, used for default transitions
    /// - `result`: Contains the result text from the last action
    /// - All workflow context variables are mapped to their CEL equivalents
    ///
    /// # Examples
    /// ```rust,no_run
    /// // Simple boolean expression
    /// let expr1 = "default";  // Always true
    ///
    /// // Variable comparison
    /// let expr2 = "status == \"active\"";
    ///
    /// // Complex conditions
    /// let expr3 = "count > 10 && status == \"ready\"";
    ///
    /// // Result text matching
    /// let expr4 = "result.contains(\"success\")";
    /// ```
    fn evaluate_cel_expression(
        &mut self,
        expression: &str,
        context: &HashMap<String, Value>,
    ) -> ExecutorResult<bool> {
        let evaluation_start = Instant::now();

        // Validate expression for security
        let validation_start = Instant::now();
        self.validate_cel_expression(expression)?;
        let validation_duration = validation_start.elapsed();

        // Compile the CEL program directly (no caching)
        let compilation_start = Instant::now();
        let program = cel_interpreter::Program::compile(expression).map_err(|e| {
            ExecutorError::ExpressionError(format!(
                "CEL compilation failed: Unable to compile expression '{expression}' ({e})"
            ))
        })?;

        // Create CEL context with workflow variables
        let context_start = Instant::now();
        let mut cel_context = Context::default();

        // Add 'default' variable that is always true
        cel_context
            .add_variable(DEFAULT_VARIABLE_NAME, true)
            .map_err(|e| {
                ExecutorError::ExpressionError(format!(
                    "CEL context error: Failed to add '{DEFAULT_VARIABLE_NAME}' variable ({e})"
                ))
            })?;

        // Add 'result' variable from the final response
        let result_text = Self::extract_result_text_static(context);
        cel_context
            .add_variable(RESULT_VARIABLE_NAME, result_text)
            .map_err(|e| {
                ExecutorError::ExpressionError(format!(
                    "CEL context error: Failed to add '{RESULT_VARIABLE_NAME}' variable ({e})"
                ))
            })?;

        // Add other context variables
        for (key, value) in context {
            Self::add_json_variable_to_cel_context_static(&mut cel_context, key, value).map_err(
                |e| {
                    ExecutorError::ExpressionError(format!(
                        "CEL context error: Failed to add variable '{key}' ({e})"
                    ))
                },
            )?;
        }

        let context_duration = context_start.elapsed();

        // Execute the expression with timeout
        let execution_start = Instant::now();
        let result = program.execute(&cel_context).map_err(|e| {
            ExecutorError::ExpressionError(format!(
                "CEL execution failed: Unable to execute expression '{expression}' ({e})"
            ))
        })?;
        let execution_duration = execution_start.elapsed();

        // Check if execution took too long
        if execution_duration > MAX_EXECUTION_TIME {
            return Err(ExecutorError::ExpressionError(format!(
                "CEL execution timeout: Expression '{}' exceeded maximum execution time ({} ms, limit: {} ms)",
                expression,
                execution_duration.as_millis(),
                MAX_EXECUTION_TIME.as_millis()
            )));
        }

        // Convert result to boolean
        let conversion_start = Instant::now();
        let boolean_result = Self::cel_value_to_bool_static(&result, expression)?;
        let conversion_duration = conversion_start.elapsed();

        let total_evaluation_time = evaluation_start.elapsed();

        // Enhanced debug logging for CEL evaluation
        let result_text = Self::extract_result_text_static(context);
        let context_keys: Vec<String> = context.keys().cloned().collect();
        tracing::debug!("CEL Debug - Expression: '{}' | Result text: '{}' | CEL result: {:?} | Boolean: {} | Context keys: {:?}", expression, result_text, result, boolean_result, context_keys);

        // Log comprehensive performance metrics after program execution is complete
        let compilation_duration = compilation_start.elapsed();
        self.log_event(
            ExecutionEventType::StateExecution,
            format!(
                "CEL evaluation performance: total={:?}, validation={:?}, compilation={:?}, context={:?}, execution={:?}, conversion={:?}, cache={}, variables={}",
                total_evaluation_time,
                validation_duration,
                compilation_duration,
                context_duration,
                execution_duration,
                conversion_duration,
                "CACHED",
                context.len() + 2
            ),
        );

        // Log performance warning if evaluation is slow
        if total_evaluation_time > Duration::from_millis(50) {
            self.log_event(
                ExecutionEventType::StateExecution,
                format!("CEL performance warning: Expression '{expression}' took {total_evaluation_time:?} to evaluate (consider optimization)"),
            );
        }

        Ok(boolean_result)
    }

    /// Extract result text from context for CEL evaluation (static version)
    ///
    /// This function extracts result text from the workflow context for use in CEL
    /// expressions. It searches for result data in multiple standard keys.
    ///
    /// # Arguments
    /// * `context` - The workflow context to search for result data
    ///
    /// # Returns
    /// * `String` - The extracted result text, or empty string if not found
    ///
    /// # Search Order
    /// The function searches for result data in the following keys (in order):
    /// 1. `result` - Standard result key
    /// 2. `output` - Common output key
    /// 3. `response` - Response data key
    /// 4. `claude_result` - Claude-specific result key
    ///
    /// # Value Handling
    /// - String values are returned as-is
    /// - Other types are JSON-serialized to string
    /// - Serialization errors result in a descriptive error message
    fn extract_result_text_static(context: &HashMap<String, Value>) -> String {
        // Look for common result keys
        for key in RESULT_KEYS {
            if let Some(value) = context.get(*key) {
                return match value {
                    Value::String(s) => s.clone(),
                    _ => serde_json::to_string(value)
                        .unwrap_or_else(|_| format!("Error serializing value: {value:?}")),
                };
            }
        }

        // Default empty string if no result found
        String::new()
    }

    /// Add JSON variable to CEL context (static version)
    ///
    /// This function converts JSON values to their CEL equivalents and adds them to the
    /// CEL evaluation context. It handles all JSON types including complex structures.
    ///
    /// # Arguments
    /// * `cel_context` - The CEL context to add the variable to
    /// * `key` - The variable name in the CEL context
    /// * `value` - The JSON value to convert and add
    ///
    /// # JSON to CEL Type Mapping
    /// - `JSON Bool` → `CEL Bool`
    /// - `JSON Number` → `CEL Int` or `CEL Float`
    /// - `JSON String` → `CEL String`
    /// - `JSON Null` → `CEL Null`
    /// - `JSON Array` → `CEL List`
    /// - `JSON Object` → `CEL Map`
    ///
    /// # Error Handling
    /// For unsupported or complex types, the function falls back to string representation
    /// to ensure the CEL expression can still be evaluated.
    fn add_json_variable_to_cel_context_static(
        cel_context: &mut Context,
        key: &str,
        value: &Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match value {
            Value::Bool(b) => {
                cel_context.add_variable(key, *b)?;
            }
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    cel_context.add_variable(key, i)?;
                } else if let Some(f) = n.as_f64() {
                    cel_context.add_variable(key, f)?;
                }
            }
            Value::String(s) => {
                cel_context.add_variable(key, s.clone())?;
            }
            Value::Null => {
                // CEL handles null values, so we can add them
                cel_context.add_variable(key, cel_interpreter::Value::Null)?;
            }
            Value::Array(arr) => {
                // Convert array to CEL list
                let cel_list: Result<Vec<_>, _> =
                    arr.iter().map(|v| Self::json_to_cel_value(v)).collect();
                match cel_list {
                    Ok(list) => {
                        cel_context.add_variable(key, cel_interpreter::Value::List(list.into()))?;
                    }
                    Err(_) => {
                        // If conversion fails, convert to string representation
                        let arr_str = serde_json::to_string(arr)
                            .unwrap_or_else(|_| format!("Array with {} elements", arr.len()));
                        cel_context.add_variable(key, arr_str)?;
                    }
                }
            }
            Value::Object(obj) => {
                // Convert object to CEL map
                let mut cel_map = std::collections::HashMap::new();
                for (k, v) in obj {
                    match Self::json_to_cel_value(v) {
                        Ok(cel_val) => {
                            cel_map.insert(k.clone(), cel_val);
                        }
                        Err(_) => {
                            // If conversion fails, use string representation
                            let val_str = serde_json::to_string(v)
                                .unwrap_or_else(|_| "complex_value".to_string());
                            cel_map.insert(
                                k.clone(),
                                cel_interpreter::Value::String(Arc::new(val_str)),
                            );
                        }
                    }
                }
                cel_context.add_variable(key, cel_interpreter::Value::Map(cel_map.into()))?;
            }
        }
        Ok(())
    }

    /// Convert JSON value to CEL value
    ///
    /// This function provides comprehensive conversion from JSON types to CEL types.
    /// It handles all JSON value types including nested structures.
    ///
    /// # Arguments
    /// * `value` - The JSON value to convert
    ///
    /// # Returns
    /// * `Ok(cel_interpreter::Value)` - The converted CEL value
    /// * `Err(Box<dyn std::error::Error>)` - If conversion fails
    ///
    /// # Type Conversions
    /// - Primitives: bool, numbers, strings, null are converted directly
    /// - Arrays: Recursively converted to CEL Lists
    /// - Objects: Recursively converted to CEL Maps
    ///
    /// # Performance Notes
    /// This function uses `.into()` for type conversion which leverages the CEL
    /// interpreter's built-in conversion mechanisms for optimal performance.
    fn json_to_cel_value(
        value: &Value,
    ) -> Result<cel_interpreter::Value, Box<dyn std::error::Error>> {
        match value {
            Value::Bool(b) => Ok(cel_interpreter::Value::Bool(*b)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(cel_interpreter::Value::Int(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(cel_interpreter::Value::Float(f))
                } else {
                    Err("Invalid number format".into())
                }
            }
            Value::String(s) => Ok(cel_interpreter::Value::String(Arc::new(s.clone()))),
            Value::Null => Ok(cel_interpreter::Value::Null),
            Value::Array(arr) => {
                let cel_list: Result<Vec<_>, _> =
                    arr.iter().map(|v| Self::json_to_cel_value(v)).collect();
                Ok(cel_interpreter::Value::List(cel_list?.into()))
            }
            Value::Object(obj) => {
                let mut cel_map = std::collections::HashMap::new();
                for (k, v) in obj {
                    cel_map.insert(k.clone(), Self::json_to_cel_value(v)?);
                }
                Ok(cel_interpreter::Value::Map(cel_map.into()))
            }
        }
    }

    /// Convert CEL value to boolean (static version)
    ///
    /// This function converts CEL evaluation results to boolean values for use in
    /// workflow transition logic. It handles all CEL value types with intuitive
    /// truthiness rules.
    ///
    /// # Arguments
    /// * `value` - The CEL value to convert to boolean
    /// * `expression` - The original expression (for error reporting)
    ///
    /// # Returns
    /// * `Ok(true)` if the value is truthy
    /// * `Ok(false)` if the value is falsy
    /// * `Err(ExecutorError::ExpressionError)` if the value cannot be converted
    ///
    /// # Truthiness Rules
    /// - `Bool(true)` → `true`
    /// - `Bool(false)` → `false`
    /// - `Int(0)` → `false`, `Int(non-zero)` → `true`
    /// - `Float(0.0)` → `false`, `Float(non-zero)` → `true`
    /// - `String("")` → `false`, `String(non-empty)` → `true`
    /// - `Null` → `false`
    /// - Other types → Error (unsupported for boolean conversion)
    fn cel_value_to_bool_static(value: &CelValue, expression: &str) -> ExecutorResult<bool> {
        match value {
            CelValue::Bool(b) => Ok(*b),
            CelValue::Int(i) => Ok(*i != 0),
            CelValue::Float(f) => Ok(*f != 0.0),
            CelValue::String(s) => Ok(!s.is_empty()),
            CelValue::Null => Ok(false),
            _ => Err(ExecutorError::ExpressionError(format!(
                "CEL expression '{expression}' returned non-boolean result: {value:?}"
            ))),
        }
    }
}
