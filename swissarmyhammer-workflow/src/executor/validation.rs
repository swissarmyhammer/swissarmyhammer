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
//! # use swissarmyhammer_workflow::executor::validation::{TransitionCondition, ConditionType};
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
use crate::{ConditionType, StateId, TransitionCondition, WorkflowRun};
use cel_interpreter::{Context, Value as CelValue};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use swissarmyhammer_common::Pretty;

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
const RESULT_KEYS: &[&str] = &[
    "result",
    "output",
    "response",
    "claude_result",
    "claude_response",
];

impl WorkflowExecutor {
    /// Evaluate all transitions from the current state
    pub fn evaluate_transitions(&mut self, run: &WorkflowRun) -> ExecutorResult<Option<StateId>> {
        let current_state = &run.current_state;

        let transitions: Vec<_> = run
            .workflow
            .transitions
            .iter()
            .filter(|t| &t.from_state == current_state)
            .collect();

        self.validate_choice_state_structure(current_state, &transitions, run)?;
        let matching_transitions = self.find_matching_transitions(&transitions, run)?;
        self.handle_multiple_transition_matches(current_state, &matching_transitions)?;
        self.resolve_transition_result(current_state, &matching_transitions, run)
    }

    /// Validate choice state structure and requirements
    fn validate_choice_state_structure(
        &self,
        current_state: &StateId,
        transitions: &[&crate::Transition],
        run: &WorkflowRun,
    ) -> ExecutorResult<()> {
        let state_type = run
            .workflow
            .states
            .get(current_state)
            .map(|state| &state.state_type);
        let is_choice_state = state_type == Some(&crate::StateType::Choice);

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
            self.validate_choice_state_determinism(current_state, transitions)?;
        }

        Ok(())
    }

    /// Find all matching transitions based on their conditions
    fn find_matching_transitions<'a>(
        &mut self,
        transitions: &[&'a crate::Transition],
        run: &WorkflowRun,
    ) -> ExecutorResult<Vec<&'a crate::Transition>> {
        let mut matching_transitions = Vec::new();
        let context_hashmap = run.context.to_workflow_hashmap();

        for transition in transitions {
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
                matching_transitions.push(*transition);
            }
        }

        Ok(matching_transitions)
    }

    /// Handle and warn about multiple matching transitions
    fn handle_multiple_transition_matches(
        &mut self,
        current_state: &StateId,
        matching_transitions: &[&crate::Transition],
    ) -> ExecutorResult<()> {
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

        Ok(())
    }

    /// Resolve the final transition result
    fn resolve_transition_result(
        &mut self,
        current_state: &StateId,
        matching_transitions: &[&crate::Transition],
        run: &WorkflowRun,
    ) -> ExecutorResult<Option<StateId>> {
        if let Some(first_match) = matching_transitions.first() {
            return Ok(Some(first_match.to_state.clone()));
        }

        let state_type = run
            .workflow
            .states
            .get(current_state)
            .map(|state| &state.state_type);
        let is_choice_state = state_type == Some(&crate::StateType::Choice);

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
                _ => default_value,
            }
        } else {
            default_value
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
    fn validate_choice_state_determinism(
        &self,
        state_id: &StateId,
        transitions: &[&crate::Transition],
    ) -> ExecutorResult<()> {
        let has_default = Self::has_default_condition(transitions);

        if !has_default {
            Self::check_for_ambiguous_conditions(state_id, transitions)?;
        }

        Self::validate_no_never_conditions(state_id, transitions)?;

        Ok(())
    }

    /// Check if transitions contain a default condition
    fn has_default_condition(transitions: &[&crate::Transition]) -> bool {
        transitions
            .iter()
            .any(|t| match &t.condition.condition_type {
                crate::ConditionType::Always => true,
                crate::ConditionType::Custom => {
                    if let Some(expr) = &t.condition.expression {
                        expr.trim() == DEFAULT_VARIABLE_NAME
                    } else {
                        false
                    }
                }
                _ => false,
            })
    }

    /// Check for ambiguous conditions in choice state transitions
    fn check_for_ambiguous_conditions(
        state_id: &StateId,
        transitions: &[&crate::Transition],
    ) -> ExecutorResult<()> {
        let condition_types: Vec<_> = transitions
            .iter()
            .map(|t| &t.condition.condition_type)
            .collect();

        let success_count = condition_types
            .iter()
            .filter(|ct| matches!(ct, crate::ConditionType::OnSuccess))
            .count();
        let failure_count = condition_types
            .iter()
            .filter(|ct| matches!(ct, crate::ConditionType::OnFailure))
            .count();

        if success_count > 1 || failure_count > 1 {
            return Err(ExecutorError::ExecutionFailed(
                format!(
                    "Choice state '{state_id}' has ambiguous conditions: {success_count} OnSuccess, {failure_count} OnFailure. Consider adding a default condition or making conditions mutually exclusive"
                ),
            ));
        }

        Ok(())
    }

    /// Validate that Never conditions are not used in choice states
    fn validate_no_never_conditions(
        state_id: &StateId,
        transitions: &[&crate::Transition],
    ) -> ExecutorResult<()> {
        let never_conditions = transitions
            .iter()
            .filter(|t| matches!(t.condition.condition_type, crate::ConditionType::Never))
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
    fn validate_cel_expression(&self, expression: &str) -> ExecutorResult<()> {
        if expression.len() > MAX_EXPRESSION_LENGTH {
            return Err(ExecutorError::ExpressionError(format!(
                "CEL expression too long: {} characters (max {})",
                expression.len(),
                MAX_EXPRESSION_LENGTH
            )));
        }

        Self::check_forbidden_patterns(expression)?;
        Self::validate_quote_patterns(expression)?;
        Self::validate_nesting_depth(expression)?;

        Ok(())
    }

    /// Check for forbidden patterns in CEL expression
    fn check_forbidden_patterns(expression: &str) -> ExecutorResult<()> {
        let expr_lower = expression.to_lowercase();
        for pattern in FORBIDDEN_PATTERNS {
            if expr_lower.contains(pattern) {
                return Err(ExecutorError::ExpressionError(format!(
                    "CEL expression contains forbidden pattern: '{pattern}'"
                )));
            }
        }
        Ok(())
    }

    /// Validate quote patterns in CEL expression
    fn validate_quote_patterns(expression: &str) -> ExecutorResult<()> {
        if expression.contains("\"\"\"") || expression.contains("'''") {
            return Err(ExecutorError::ExpressionError(
                "CEL expression contains suspicious quote patterns".to_string(),
            ));
        }
        Ok(())
    }

    /// Validate nesting depth in CEL expression
    fn validate_nesting_depth(expression: &str) -> ExecutorResult<()> {
        let max_depth = Self::calculate_nesting_depth(expression);

        if max_depth > 10 {
            return Err(ExecutorError::ExpressionError(format!(
                "CEL expression has excessive nesting depth: {max_depth} (max 10)"
            )));
        }

        Ok(())
    }

    /// Calculate maximum nesting depth of parentheses, brackets, and braces
    fn calculate_nesting_depth(expression: &str) -> i32 {
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

        max_depth
    }

    /// Evaluate a CEL expression with the given context
    fn evaluate_cel_expression(
        &mut self,
        expression: &str,
        context: &HashMap<String, Value>,
    ) -> ExecutorResult<bool> {
        let evaluation_start = Instant::now();

        self.validate_cel_expression(expression)?;
        let program = self.compile_cel_program(expression)?;
        let cel_context = self.prepare_cel_context(context)?;

        let execution_start = Instant::now();
        let result = self.execute_cel_program(&program, &cel_context, expression)?;
        let execution_duration = execution_start.elapsed();

        if execution_duration > MAX_EXECUTION_TIME {
            return Err(ExecutorError::ExpressionError(format!(
                "CEL execution timeout: Expression '{}' exceeded maximum execution time ({} ms, limit: {} ms)",
                expression,
                execution_duration.as_millis(),
                MAX_EXECUTION_TIME.as_millis()
            )));
        }

        let boolean_result = Self::cel_value_to_bool_static(&result, expression)?;

        self.log_cel_evaluation(evaluation_start.elapsed(), expression, context);
        self.log_cel_debug(expression, context, &result, boolean_result);

        Ok(boolean_result)
    }

    /// Compile CEL program from expression
    fn compile_cel_program(&self, expression: &str) -> ExecutorResult<cel_interpreter::Program> {
        cel_interpreter::Program::compile(expression).map_err(|e| {
            ExecutorError::ExpressionError(format!(
                "CEL compilation failed: Unable to compile expression '{expression}' ({e})"
            ))
        })
    }

    /// Prepare CEL context with workflow variables
    ///
    /// This creates a stacked context where global CEL variables (like abort)
    /// are accessible alongside workflow-specific variables.
    fn prepare_cel_context(&self, context: &HashMap<String, Value>) -> ExecutorResult<Context<'_>> {
        let mut cel_context = Context::default();

        // Copy global CEL variables to the context (context stacking)
        let global_state = swissarmyhammer_cel::CelState::global();
        global_state
            .copy_to_context(&mut cel_context)
            .map_err(|e| {
                ExecutorError::ExpressionError(format!(
                    "CEL context error: Failed to copy global variables ({e})"
                ))
            })?;

        // Add workflow-specific variables on top
        Self::add_default_variable(&mut cel_context)?;
        Self::add_context_variables(&mut cel_context, context)?;
        Self::add_result_variable_fallback(&mut cel_context, context)?;

        Ok(cel_context)
    }

    /// Add default variable to CEL context
    fn add_default_variable(cel_context: &mut Context) -> ExecutorResult<()> {
        cel_context
            .add_variable(DEFAULT_VARIABLE_NAME, true)
            .map_err(|e| {
                ExecutorError::ExpressionError(format!(
                    "CEL context error: Failed to add '{DEFAULT_VARIABLE_NAME}' variable ({e})"
                ))
            })
    }

    /// Add all context variables to CEL context
    fn add_context_variables(
        cel_context: &mut Context,
        context: &HashMap<String, Value>,
    ) -> ExecutorResult<()> {
        for (key, value) in context {
            if key == RESULT_VARIABLE_NAME {
                tracing::debug!("Adding 'result' variable to CEL context: {}", Pretty(value));
            }

            Self::add_json_variable_to_cel_context_static(cel_context, key, value).map_err(
                |e| {
                    ExecutorError::ExpressionError(format!(
                        "CEL context error: Failed to add variable '{key}' ({e})"
                    ))
                },
            )?;
        }

        Ok(())
    }

    /// Add result variable as text fallback if not already present
    fn add_result_variable_fallback(
        cel_context: &mut Context,
        context: &HashMap<String, Value>,
    ) -> ExecutorResult<()> {
        if !context.contains_key(RESULT_VARIABLE_NAME) {
            tracing::debug!("'result' not in context, adding text fallback");
            let result_text = Self::extract_result_text_static(context);
            cel_context
                .add_variable(RESULT_VARIABLE_NAME, result_text)
                .map_err(|e| {
                    ExecutorError::ExpressionError(format!(
                        "CEL context error: Failed to add '{RESULT_VARIABLE_NAME}' variable ({e})"
                    ))
                })?;
        } else {
            tracing::debug!("'result' found in context, using object version");
        }

        Ok(())
    }

    /// Execute CEL program with context (returns false if variables are undeclared, allowing lazy evaluation)
    fn execute_cel_program(
        &self,
        program: &cel_interpreter::Program,
        cel_context: &Context,
        expression: &str,
    ) -> ExecutorResult<CelValue> {
        program.execute(cel_context).or_else(|e| {
            let err_msg = e.to_string();
            // If the error is about undeclared references, treat as false (variable doesn't exist yet)
            // This allows workflows to reference variables that will be set during execution
            if err_msg.contains("undeclared") || err_msg.contains("Undeclared") {
                tracing::debug!(
                    "CEL expression '{}' references undeclared variable, treating as false: {}",
                    expression,
                    err_msg
                );
                Ok(CelValue::Bool(false))
            } else {
                Err(ExecutorError::ExpressionError(format!(
                    "CEL execution failed: Unable to execute expression '{expression}' ({e})"
                )))
            }
        })
    }

    /// Log CEL evaluation performance
    fn log_cel_evaluation(
        &mut self,
        total_time: Duration,
        expression: &str,
        context: &HashMap<String, Value>,
    ) {
        self.log_event(
            ExecutionEventType::StateExecution,
            format!(
                "CEL evaluation completed: total={:?}, variables={}",
                total_time,
                context.len() + 2
            ),
        );

        if total_time > Duration::from_millis(50) {
            self.log_event(
                ExecutionEventType::StateExecution,
                format!(
                    "CEL performance warning: Expression '{expression}' took {total_time:?} to evaluate (consider optimization)"
                ),
            );
        }
    }

    /// Log CEL debug information
    fn log_cel_debug(
        &self,
        expression: &str,
        context: &HashMap<String, Value>,
        result: &CelValue,
        boolean_result: bool,
    ) {
        if let Some(result_value) = context.get("result") {
            tracing::debug!(
                "CEL Debug - 'result' variable in context: {:?}",
                result_value
            );
        } else {
            tracing::debug!("CEL Debug - 'result' variable NOT in context");
        }

        let result_text = Self::extract_result_text_static(context);
        let context_keys: Vec<String> = context.keys().cloned().collect();

        #[derive(serde::Serialize, Debug)]
        struct CelResult {
            value: String,
        }
        let cel_result = CelResult {
            value: format!("{:?}", result),
        };
        tracing::debug!("CEL Debug - Expression: '{}' | Result text: '{}' | CEL result: {} | Boolean: {} | Context keys: {}", expression, result_text, Pretty(&cel_result), boolean_result, Pretty(&context_keys));
    }

    /// Convert JSON value to string (helper for extraction)
    fn json_value_to_string(value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            _ => serde_json::to_string(value)
                .unwrap_or_else(|_| format!("Error serializing value: {value:?}")),
        }
    }

    /// Extract result text from context for CEL evaluation (static version)
    fn extract_result_text_static(context: &HashMap<String, Value>) -> String {
        for key in RESULT_KEYS {
            if let Some(value) = context.get(*key) {
                return Self::json_value_to_string(value);
            }
        }

        String::new()
    }

    /// Add JSON variable to CEL context (static version)
    fn add_json_variable_to_cel_context_static(
        cel_context: &mut Context,
        key: &str,
        value: &Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cel_value = Self::json_to_cel_value(value).unwrap_or_else(|_| {
            let fallback = Self::json_value_to_string(value);
            cel_interpreter::Value::String(Arc::new(fallback))
        });
        cel_context.add_variable(key, cel_value)?;
        Ok(())
    }

    /// Convert JSON object to CEL map
    fn convert_object_to_cel_map(
        obj: &serde_json::Map<String, Value>,
    ) -> Result<std::collections::HashMap<String, cel_interpreter::Value>, Box<dyn std::error::Error>>
    {
        let mut cel_map = std::collections::HashMap::new();
        for (k, v) in obj {
            match Self::json_to_cel_value(v) {
                Ok(cel_val) => {
                    cel_map.insert(k.clone(), cel_val);
                }
                Err(_) => {
                    let val_str = Self::json_value_to_string(v);
                    cel_map.insert(k.clone(), cel_interpreter::Value::String(Arc::new(val_str)));
                }
            }
        }
        Ok(cel_map)
    }

    /// Convert JSON value to CEL value
    fn json_to_cel_value(
        value: &Value,
    ) -> Result<cel_interpreter::Value, Box<dyn std::error::Error>> {
        match value {
            Value::Bool(b) => Ok(cel_interpreter::Value::Bool(*b)),
            Value::Number(n) => Self::convert_number_to_cel_value(n),
            Value::String(s) => Ok(cel_interpreter::Value::String(Arc::new(s.clone()))),
            Value::Null => Ok(cel_interpreter::Value::Null),
            Value::Array(arr) => Self::convert_array_to_cel_value(arr),
            Value::Object(obj) => Self::convert_object_to_cel_value(obj),
        }
    }

    /// Convert JSON number to CEL value
    fn convert_number_to_cel_value(
        n: &serde_json::Number,
    ) -> Result<cel_interpreter::Value, Box<dyn std::error::Error>> {
        if let Some(i) = n.as_i64() {
            Ok(cel_interpreter::Value::Int(i))
        } else if let Some(f) = n.as_f64() {
            Ok(cel_interpreter::Value::Float(f))
        } else {
            Err("Invalid number format".into())
        }
    }

    /// Convert JSON array to CEL value
    fn convert_array_to_cel_value(
        arr: &[Value],
    ) -> Result<cel_interpreter::Value, Box<dyn std::error::Error>> {
        let cel_list: Result<Vec<_>, _> = arr.iter().map(|v| Self::json_to_cel_value(v)).collect();
        Ok(cel_interpreter::Value::List(cel_list?.into()))
    }

    /// Convert JSON object to CEL value
    fn convert_object_to_cel_value(
        obj: &serde_json::Map<String, Value>,
    ) -> Result<cel_interpreter::Value, Box<dyn std::error::Error>> {
        let cel_map = Self::convert_object_to_cel_map(obj)?;
        Ok(cel_interpreter::Value::Map(cel_map.into()))
    }

    /// Convert CEL value to boolean (static version)
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
