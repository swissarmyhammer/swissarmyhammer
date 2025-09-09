//! Tests for LogAction

use crate::workflow::actions::*;
use crate::workflow::actions_tests::common::*;
use serde_json::Value;
use std::collections::HashMap;

#[test]
fn test_log_action_creation() {
    let action = LogAction::new("Test message".to_string(), LogLevel::Info);
    assert_eq!(action.message, "Test message");
    assert!(matches!(action.level, LogLevel::Info));
}

#[test]
fn test_log_action_convenience_methods() {
    let info_action = LogAction::info("Info message".to_string());
    assert!(matches!(info_action.level, LogLevel::Info));

    let warning_action = LogAction::warning("Warning message".to_string());
    assert!(matches!(warning_action.level, LogLevel::Warning));

    let error_action = LogAction::error("Error message".to_string());
    assert!(matches!(error_action.level, LogLevel::Error));
}

#[test]
fn test_log_action_description() {
    let action = LogAction::info("Test message".to_string());
    assert_eq!(action.description(), "Log message: Test message");
}

#[test]
fn test_log_action_type() {
    let action = LogAction::info("Test message".to_string());
    assert_eq!(action.action_type(), "log");
}

#[tokio::test]
async fn test_log_action_execution_info() {
    let action = LogAction::info("Test info message".to_string());
    let mut context = HashMap::new();

    let result = action.execute(&mut context).await;
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap(),
        Value::String("Test info message".to_string())
    );
    assert_eq!(context.get("last_action_result"), Some(&Value::Bool(true)));
}

#[tokio::test]
async fn test_log_action_execution_warning() {
    let action = LogAction::warning("Test warning message".to_string());
    let mut context = HashMap::new();

    let result = action.execute(&mut context).await;
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap(),
        Value::String("Test warning message".to_string())
    );
}

#[tokio::test]
async fn test_log_action_execution_error() {
    let action = LogAction::error("Test error message".to_string());
    let mut context = HashMap::new();

    let result = action.execute(&mut context).await;
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap(),
        Value::String("Test error message".to_string())
    );
}

#[tokio::test]
async fn test_log_action_variable_substitution() {
    let action = LogAction::info("File: ${current_file}, User: ${user_name}".to_string());
    let mut context = create_test_context();

    let result = action.execute(&mut context).await;
    assert!(result.is_ok());
    let message = result.unwrap();
    assert_eq!(
        message,
        Value::String("File: test.rs, User: testuser".to_string())
    );
}

#[tokio::test]
async fn test_log_action_with_special_characters() {
    let action = LogAction::info("Special: ${special_chars}".to_string());
    let mut context = create_context_with_special_chars();

    let result = action.execute(&mut context).await;
    assert!(result.is_ok());
    let message = result.unwrap();
    assert_eq!(
        message,
        Value::String("Special: hello\"world'test".to_string())
    );
}