//! Condition evaluation and validation functionality
//!
//! This module provides comprehensive condition evaluation and validation for workflow
//! transitions, with a focus on security and performance. It supports multiple condition
//! types including JavaScript expressions for complex logic.
//!
//! # Architecture
//!
//! The module is organized around the following key components:
//! - **Security Validation**: Prevents injection attacks and resource exhaustion
//! - **Expression Evaluation**: Uses rquickjs (QuickJS-NG) for JavaScript evaluation
//! - **Context Management**: Converts workflow data to JS-compatible formats
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
//! ## Custom JavaScript Expressions
//! - `Custom`: Evaluates user-provided JavaScript expressions
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
//! - **Timeout Protection**: Limits expression execution time via interrupt handler
//! - **Resource Limits**: Memory and stack size limits on JS runtime
//! - **Sandboxed Execution**: Fresh JS runtime per evaluation, isolated from global state

use super::core::WorkflowExecutor;
use super::{ExecutionEventType, ExecutorError, ExecutorResult, LAST_ACTION_RESULT_KEY};
use crate::{ConditionType, StateId, TransitionCondition, WorkflowRun};
use rquickjs::{CatchResultExt, CaughtError, Context, Object, Runtime};
use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use swissarmyhammer_common::Pretty;

// Security constants for expression evaluation
const MAX_EXPRESSION_LENGTH: usize = 500;
const MAX_EXECUTION_TIME: Duration = Duration::from_millis(100);
const DEFAULT_VARIABLE_NAME: &str = "default";
const RESULT_VARIABLE_NAME: &str = "result";

// Forbidden patterns that could be dangerous in JS expressions
const FORBIDDEN_PATTERNS: &[&str] = &[
    "import",
    "require",
    "eval",
    "exec",
    "system",
    "file",
    "read",
    "write",
    "delete",
    "mkdir",
    "rmdir",
    "chmod",
    "chown",
    "kill",
    "spawn",
    "Function",
    "setTimeout",
    "setInterval",
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
                    self.evaluate_js_expression(expression, context)
                } else {
                    Err(ExecutorError::ExpressionError(
                        "Custom condition requires an expression to be specified".to_string(),
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

    /// Validate and sanitize an expression for security
    fn validate_expression(&self, expression: &str) -> ExecutorResult<()> {
        if expression.len() > MAX_EXPRESSION_LENGTH {
            return Err(ExecutorError::ExpressionError(format!(
                "Expression too long: {} characters (max {})",
                expression.len(),
                MAX_EXPRESSION_LENGTH
            )));
        }

        Self::check_forbidden_patterns(expression)?;
        Self::validate_quote_patterns(expression)?;
        Self::validate_nesting_depth(expression)?;

        Ok(())
    }

    /// Check for forbidden patterns in expression
    fn check_forbidden_patterns(expression: &str) -> ExecutorResult<()> {
        let expr_lower = expression.to_lowercase();
        for pattern in FORBIDDEN_PATTERNS {
            if expr_lower.contains(&pattern.to_lowercase()) {
                return Err(ExecutorError::ExpressionError(format!(
                    "Expression contains forbidden pattern: '{pattern}'"
                )));
            }
        }
        Ok(())
    }

    /// Validate quote patterns in expression
    fn validate_quote_patterns(expression: &str) -> ExecutorResult<()> {
        if expression.contains("\"\"\"") || expression.contains("'''") {
            return Err(ExecutorError::ExpressionError(
                "Expression contains suspicious quote patterns".to_string(),
            ));
        }
        Ok(())
    }

    /// Validate nesting depth in expression
    fn validate_nesting_depth(expression: &str) -> ExecutorResult<()> {
        let max_depth = Self::calculate_nesting_depth(expression);

        if max_depth > 10 {
            return Err(ExecutorError::ExpressionError(format!(
                "Expression has excessive nesting depth: {max_depth} (max 10)"
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

    /// Evaluate a JavaScript expression with the given context.
    ///
    /// Creates a fresh rquickjs Runtime+Context per evaluation for isolation.
    /// Global variables from JsState are copied in, plus workflow-specific variables.
    fn evaluate_js_expression(
        &mut self,
        expression: &str,
        context: &HashMap<String, Value>,
    ) -> ExecutorResult<bool> {
        let evaluation_start = Instant::now();

        self.validate_expression(expression)?;

        // Handle "default" specially - it's a JS reserved word but used as a workflow
        // fallback condition that always evaluates to true
        if expression.trim() == DEFAULT_VARIABLE_NAME {
            let total_time = evaluation_start.elapsed();
            self.log_js_evaluation(total_time, expression, context);
            return Ok(true);
        }

        // Create a fresh isolated runtime for this evaluation
        let rt = Runtime::new().map_err(|e| {
            ExecutorError::ExpressionError(format!("JS runtime creation failed: {}", e))
        })?;
        rt.set_memory_limit(10 * 1024 * 1024); // 10 MB
        rt.set_max_stack_size(512 * 1024); // 512 KB

        // Set timeout interrupt handler
        let timeout = MAX_EXECUTION_TIME;
        let start = Instant::now();
        rt.set_interrupt_handler(Some(Box::new(move || start.elapsed() > timeout)));

        let js_ctx = Context::full(&rt).map_err(|e| {
            ExecutorError::ExpressionError(format!("JS context creation failed: {}", e))
        })?;

        // Inject variables and evaluate
        let result: ExecutorResult<Value> = js_ctx.with(|ctx| {
            let globals = ctx.globals();

            // 1. Copy global JS state variables (context stacking)
            self.inject_global_js_variables(&ctx, &globals)?;

            // 2. Add default=true variable
            globals.set(DEFAULT_VARIABLE_NAME, true).map_err(|e| {
                ExecutorError::ExpressionError(format!(
                    "Failed to add '{}' variable: {}",
                    DEFAULT_VARIABLE_NAME, e
                ))
            })?;

            // 3. Add workflow context variables
            self.inject_context_variables(&ctx, &globals, context)?;

            // 4. Add result variable fallback
            self.inject_result_fallback(&ctx, &globals, context)?;

            // 5. Evaluate the expression
            let eval_result: rquickjs::Value = ctx
                .eval(expression.as_bytes())
                .catch(&ctx)
                .map_err(|e| match e {
                    CaughtError::Exception(ex) => {
                        let msg = format!("{}", ex);
                        // Undeclared variable -> treat as false
                        if msg.contains("is not defined") || msg.contains("not defined") {
                            tracing::debug!(
                                "JS expression '{}' references undefined variable, treating as false: {}",
                                expression,
                                msg
                            );
                            return ExecutorError::ExpressionError("__UNDEFINED__".to_string());
                        }
                        ExecutorError::ExpressionError(format!(
                            "JS evaluation failed: '{expression}' ({msg})"
                        ))
                    }
                    CaughtError::Value(v) => {
                        let s: std::result::Result<String, _> = v.get();
                        ExecutorError::ExpressionError(format!(
                            "JS threw: {}",
                            s.unwrap_or_else(|_| "unknown".to_string())
                        ))
                    }
                    CaughtError::Error(e) => ExecutorError::ExpressionError(format!(
                        "JS error evaluating '{expression}': {e}"
                    )),
                })?;

            // Convert to JSON
            swissarmyhammer_js::bridge::js_to_json(&ctx, eval_result)
                .map_err(|e| ExecutorError::ExpressionError(format!("Type conversion failed: {}", e)))
        });

        // Handle the undefined variable sentinel
        let json_result = match result {
            Ok(v) => v,
            Err(ExecutorError::ExpressionError(ref msg)) if msg == "__UNDEFINED__" => {
                Value::Bool(false)
            }
            Err(e) => return Err(e),
        };

        let boolean_result = Self::json_value_to_bool(&json_result, expression)?;

        let total_time = evaluation_start.elapsed();
        self.log_js_evaluation(total_time, expression, context);
        self.log_js_debug(expression, context, &json_result, boolean_result);

        Ok(boolean_result)
    }

    /// Inject global JS state variables into evaluation context
    fn inject_global_js_variables<'js>(
        &self,
        ctx: &rquickjs::Ctx<'js>,
        globals: &Object<'js>,
    ) -> ExecutorResult<()> {
        // Get variables from the async JsState.
        // The JS worker thread communicates via std::sync::mpsc channels + oneshot replies.
        // We spawn a dedicated thread with its own tokio runtime to bridge async->sync,
        // which works regardless of whether we're in a single-thread or multi-thread runtime.
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = tx.send(Err(format!("Failed to create tokio runtime: {}", e)));
                    return;
                }
            };
            let js_state = swissarmyhammer_js::JsState::global();
            let result = rt.block_on(js_state.get_all_variables());
            let _ = tx.send(result);
        });
        let global_vars = rx.recv().map_err(|e| {
            ExecutorError::ExpressionError(format!("Failed to get global JS variables: {}", e))
        })?;

        let global_vars = global_vars.map_err(|e| {
            ExecutorError::ExpressionError(format!("Failed to get global JS variables: {}", e))
        })?;

        for (name, value) in &global_vars {
            match swissarmyhammer_js::bridge::json_to_js(ctx, value) {
                Ok(js_val) => {
                    let _ = globals.set(name.as_str(), js_val);
                }
                Err(e) => {
                    tracing::warn!("Failed to inject global variable '{}': {}", name, e);
                }
            }
        }

        Ok(())
    }

    /// Inject workflow context variables into JS globals
    fn inject_context_variables<'js>(
        &self,
        ctx: &rquickjs::Ctx<'js>,
        globals: &Object<'js>,
        context: &HashMap<String, Value>,
    ) -> ExecutorResult<()> {
        for (key, value) in context {
            if key == RESULT_VARIABLE_NAME {
                tracing::debug!("Adding 'result' variable to JS context: {}", Pretty(value));
            }

            match swissarmyhammer_js::bridge::json_to_js(ctx, value) {
                Ok(js_val) => {
                    globals.set(key.as_str(), js_val).map_err(|e| {
                        ExecutorError::ExpressionError(format!(
                            "Failed to add variable '{}': {}",
                            key, e
                        ))
                    })?;
                }
                Err(e) => {
                    // Fallback: set as string
                    let fallback = Self::json_value_to_string(value);
                    let _ = globals.set(key.as_str(), fallback.as_str());
                    tracing::warn!(
                        "Variable '{}' injected as string fallback due to conversion error: {}",
                        key,
                        e
                    );
                }
            }
        }

        Ok(())
    }

    /// Inject result variable fallback if not already present
    fn inject_result_fallback<'js>(
        &self,
        _ctx: &rquickjs::Ctx<'js>,
        globals: &Object<'js>,
        context: &HashMap<String, Value>,
    ) -> ExecutorResult<()> {
        if !context.contains_key(RESULT_VARIABLE_NAME) {
            tracing::debug!("'result' not in context, adding text fallback");
            let result_text = Self::extract_result_text_static(context);
            globals
                .set(RESULT_VARIABLE_NAME, result_text.as_str())
                .map_err(|e| {
                    ExecutorError::ExpressionError(format!(
                        "Failed to add '{}' variable: {}",
                        RESULT_VARIABLE_NAME, e
                    ))
                })?;
        } else {
            tracing::debug!("'result' found in context, using object version");
        }

        Ok(())
    }

    /// Convert a JSON value to a boolean (JS-style truthiness)
    fn json_value_to_bool(value: &Value, _expression: &str) -> ExecutorResult<bool> {
        match value {
            Value::Bool(b) => Ok(*b),
            Value::Number(n) => Ok(n.as_f64().is_some_and(|f| f != 0.0)),
            Value::String(s) => Ok(!s.is_empty()),
            Value::Null => Ok(false),
            Value::Array(a) => Ok(!a.is_empty()),
            Value::Object(_) => Ok(true),
        }
    }

    /// Log JS evaluation performance
    fn log_js_evaluation(
        &mut self,
        total_time: Duration,
        expression: &str,
        context: &HashMap<String, Value>,
    ) {
        self.log_event(
            ExecutionEventType::StateExecution,
            format!(
                "JS evaluation completed: total={:?}, variables={}",
                total_time,
                context.len() + 2
            ),
        );

        if total_time > Duration::from_millis(50) {
            self.log_event(
                ExecutionEventType::StateExecution,
                format!(
                    "JS performance warning: Expression '{expression}' took {total_time:?} to evaluate (consider optimization)"
                ),
            );
        }
    }

    /// Log JS debug information
    fn log_js_debug(
        &self,
        expression: &str,
        context: &HashMap<String, Value>,
        result: &Value,
        boolean_result: bool,
    ) {
        if let Some(result_value) = context.get("result") {
            tracing::debug!(
                "JS Debug - 'result' variable in context: {:?}",
                result_value
            );
        } else {
            tracing::debug!("JS Debug - 'result' variable NOT in context");
        }

        let result_text = Self::extract_result_text_static(context);
        let context_keys: Vec<String> = context.keys().cloned().collect();

        tracing::debug!(
            "JS Debug - Expression: '{}' | Result text: '{}' | JS result: {} | Boolean: {} | Context keys: {}",
            expression,
            result_text,
            Pretty(result),
            boolean_result,
            Pretty(&context_keys)
        );
    }

    /// Convert JSON value to string (helper for extraction)
    fn json_value_to_string(value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            _ => serde_json::to_string(value)
                .unwrap_or_else(|_| format!("Error serializing value: {value:?}")),
        }
    }

    /// Extract result text from context for evaluation (static version)
    fn extract_result_text_static(context: &HashMap<String, Value>) -> String {
        for key in RESULT_KEYS {
            if let Some(value) = context.get(*key) {
                return Self::json_value_to_string(value);
            }
        }

        String::new()
    }
}
