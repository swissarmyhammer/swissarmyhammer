//! Test modules for workflow actions
//!
//! This directory contains organized test modules for the actions system:
//! - `action_parsing_tests` - Tests for parsing actions from descriptions
//! - `claude_output_formatting_tests` - Tests for formatting Claude output as YAML
//! - `claude_retry_tests` - Tests for verifying Claude's built-in retry mechanism is used
//! - `concurrent_action_tests` - Tests for concurrent action execution
//! - `error_handling_tests` - Tests for error handling in actions
//! - `integration_tests` - Integration tests for action execution
//! - `resource_cleanup_tests` - Tests for resource cleanup and error recovery

// Common test utilities module
#[cfg(test)]
mod common;

// Re-export common test utilities from parent module
use crate::workflow::actions::*;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// Helper function to create a test context with common variables
pub fn create_test_context() -> HashMap<String, Value> {
    let mut context = HashMap::new();
    context.insert(
        "test_var".to_string(),
        Value::String("test_value".to_string()),
    );
    context.insert("number_var".to_string(), Value::Number(42.into()));
    context.insert("bool_var".to_string(), Value::Bool(true));
    context.insert(
        "current_file".to_string(),
        Value::String("test.rs".to_string()),
    );
    context.insert(
        "user_name".to_string(),
        Value::String("testuser".to_string()),
    );
    context
}

/// Helper function to create a test context with special characters
pub fn _create_context_with_special_chars() -> HashMap<String, Value> {
    let mut context = HashMap::new();
    context.insert(
        "special_chars".to_string(),
        Value::String("hello\"world'test".to_string()),
    );
    context.insert("empty_string".to_string(), Value::String("".to_string()));
    context.insert("null_value".to_string(), Value::Null);
    context
}

// Include all test modules
#[cfg(test)]
mod action_parsing_tests;

#[cfg(test)]
mod claude_output_formatting_tests;
#[cfg(test)]
mod concurrent_action_tests;

#[cfg(test)]
mod error_handling_tests;

#[cfg(test)]
mod integration_tests;

#[cfg(test)]
mod resource_cleanup_tests;

#[cfg(test)]
mod sub_workflow_action_tests;

#[cfg(test)]
mod sub_workflow_state_pollution_tests;

#[cfg(test)]
mod simple_state_pollution_test;

#[cfg(test)]
mod prompt_action_tests;

#[cfg(test)]
mod shell_action_tests;

#[cfg(test)]
mod shell_action_integration_tests;
