//! Integration tests for actions module

use std::{collections::HashMap, time::Duration};

use serde_json::Value;

use crate::{
    Action, ActionError, LogAction, PromptAction, SetVariableAction, WaitAction,
    WorkflowTemplateContext,
};

#[tokio::test]
async fn test_action_execution_context_preservation() {
    // Test that actions properly preserve and modify context
    let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

    // Set initial value as workflow variable
    let mut initial_vars = HashMap::new();
    initial_vars.insert(
        "initial_value".to_string(),
        Value::String("initial".to_string()),
    );
    context.set_workflow_vars(initial_vars);

    // Execute a set variable action
    let set_action = SetVariableAction::new("new_var".to_string(), "new_value".to_string());
    let _result = set_action.execute(&mut context).await.unwrap();

    // Verify context was modified
    assert_eq!(
        context.get("new_var"),
        Some(&Value::String("new_value".to_string()))
    );
    assert_eq!(
        context.get("initial_value"),
        Some(&Value::String("initial".to_string()))
    );
    assert_eq!(context.get("last_action_result"), Some(&Value::Bool(true)));

    // Execute a log action that uses the new variable
    let log_action = LogAction::info("Value: ${new_var}".to_string());
    let result = log_action.execute(&mut context).await.unwrap();

    // Verify substitution worked
    assert_eq!(result, Value::String("Value: new_value".to_string()));
}

#[tokio::test]
async fn test_multiple_actions_sequence() {
    let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

    // Execute sequence of actions
    let actions: Vec<Box<dyn Action>> = vec![
        Box::new(SetVariableAction::new(
            "step1".to_string(),
            "completed".to_string(),
        )),
        Box::new(LogAction::info("Step 1: ${step1}".to_string())),
        Box::new(SetVariableAction::new(
            "step2".to_string(),
            "also_completed".to_string(),
        )),
        Box::new(LogAction::info("Step 2: ${step2}".to_string())),
    ];

    for action in actions {
        let result = action.execute(&mut context).await;
        assert!(result.is_ok());
    }

    // Verify final context state
    assert_eq!(
        context.get("step1"),
        Some(&Value::String("completed".to_string()))
    );
    assert_eq!(
        context.get("step2"),
        Some(&Value::String("also_completed".to_string()))
    );
}

#[tokio::test]
async fn test_action_error_propagation() {
    let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

    // Test that parse errors are properly propagated
    let action = SetVariableAction::new("test".to_string(), "value".to_string());
    let result = action.execute(&mut context).await;
    assert!(result.is_ok());

    // Add an invalid key to prompt action to test error propagation
    let prompt_action = PromptAction::new("test".to_string())
        .with_argument("invalid key!".to_string(), "value".to_string());

    let result = prompt_action.execute(&mut context).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ActionError::ParseError(msg) => {
            assert!(msg.contains("Invalid argument key"));
        }
        _ => panic!("Expected ParseError"),
    }
}

#[tokio::test]
async fn test_action_timeout_behavior() {
    // Test timeout behavior with wait action
    let action = WaitAction::new_duration(Duration::from_millis(50));
    let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

    let start = std::time::Instant::now();
    let result = action.execute(&mut context).await;
    let elapsed = start.elapsed();

    assert!(result.is_ok());
    assert!(elapsed >= Duration::from_millis(40)); // Allow some tolerance
    assert!(elapsed < Duration::from_millis(100)); // Should not be too slow
}

#[tokio::test]
async fn test_action_context_key_constants() {
    let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

    // Test that actions use the correct context keys
    let action = LogAction::info("Test message".to_string());
    let result = action.execute(&mut context).await;
    assert!(result.is_ok());

    // Verify the constant keys are used
    assert!(context.contains_key("last_action_result"));
    assert_eq!(context.get("last_action_result"), Some(&Value::Bool(true)));
}
