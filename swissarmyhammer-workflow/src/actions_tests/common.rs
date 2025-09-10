//! Common test utilities and helpers for action tests

use serde_json::Value;
use std::collections::HashMap;

/// Helper function to create a test context with common variables
#[allow(dead_code)]
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
#[allow(dead_code)]
pub fn create_context_with_special_chars() -> HashMap<String, Value> {
    let mut context = HashMap::new();
    context.insert(
        "special_chars".to_string(),
        Value::String("hello\"world'test".to_string()),
    );
    context.insert("empty_string".to_string(), Value::String("".to_string()));
    context.insert("null_value".to_string(), Value::Null);
    context
}
