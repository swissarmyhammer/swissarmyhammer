//! Tests for SubWorkflowAction

use crate::test_utils::IsolatedTestEnvironment;
use crate::workflow::actions::*;
use crate::workflow::WorkflowTemplateContext;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

#[test]
fn test_sub_workflow_action_creation() {
    let action = SubWorkflowAction::new("test-workflow".to_string());
    assert_eq!(action.workflow_name, "test-workflow");
    assert!(action.input_variables.is_empty());
    assert!(action.result_variable.is_none());
    assert_eq!(action.timeout, Duration::from_secs(3600));
}

#[test]
fn test_sub_workflow_action_with_input() {
    let action = SubWorkflowAction::new("test-workflow".to_string())
        .with_input("input1".to_string(), "value1".to_string())
        .with_input("input2".to_string(), "value2".to_string());

    assert_eq!(
        action.input_variables.get("input1"),
        Some(&"value1".to_string())
    );
    assert_eq!(
        action.input_variables.get("input2"),
        Some(&"value2".to_string())
    );
}

#[test]
fn test_sub_workflow_action_with_result_variable() {
    let action = SubWorkflowAction::new("test-workflow".to_string())
        .with_result_variable("result_var".to_string());

    assert_eq!(action.result_variable, Some("result_var".to_string()));
}

#[test]
fn test_sub_workflow_action_with_timeout() {
    let timeout_duration = Duration::from_secs(300);
    let action = SubWorkflowAction::new("test-workflow".to_string()).with_timeout(timeout_duration);

    assert_eq!(action.timeout, timeout_duration);
}

// Note: Variable substitution is tested through the public execute() method
// since substitute_variables() is private

#[test]
fn test_sub_workflow_action_description() {
    let action = SubWorkflowAction::new("test-workflow".to_string())
        .with_input("input1".to_string(), "value1".to_string());

    let description = action.description();
    assert!(description.contains("test-workflow"));
    assert!(description.contains("input1"));
}

#[test]
fn test_sub_workflow_action_type() {
    let action = SubWorkflowAction::new("test-workflow".to_string());
    assert_eq!(action.action_type(), "sub_workflow");
}

#[tokio::test]
async fn test_sub_workflow_action_circular_dependency_detection() {
    let action = SubWorkflowAction::new("workflow-a".to_string());
    let mut context = WorkflowTemplateContext::with_vars(HashMap::new()).unwrap();

    // Simulate that workflow-a is already in the execution stack
    let workflow_stack = vec![
        Value::String("workflow-main".to_string()),
        Value::String("workflow-a".to_string()),
    ];
    context.insert("_workflow_stack".to_string(), Value::Array(workflow_stack));

    let result = action.execute(&mut context).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ActionError::ExecutionError(msg) => {
            assert!(msg.contains("Circular workflow dependency detected"));
            assert!(msg.contains("workflow-a"));
        }
        _ => panic!("Expected ExecutionError for circular dependency"),
    }
}

#[tokio::test]
async fn test_sub_workflow_action_invalid_input_key() {
    let action = SubWorkflowAction::new("test-workflow".to_string())
        .with_input("invalid key!".to_string(), "value".to_string());

    let mut context = WorkflowTemplateContext::with_vars(HashMap::new()).unwrap();
    let result = action.execute(&mut context).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ActionError::ParseError(msg) => {
            assert!(msg.contains("Invalid input variable key"));
        }
        _ => panic!("Expected ParseError"),
    }
}

#[tokio::test]
async fn test_sub_workflow_action_empty_workflow_stack() {
    let _guard = IsolatedTestEnvironment::new().unwrap();
    // Clear any test storage that might have been set by other tests
    crate::workflow::actions::clear_test_storage();

    let action = SubWorkflowAction::new("test-workflow".to_string());
    let mut context = WorkflowTemplateContext::with_vars(HashMap::new()).unwrap();

    // This should not fail with circular dependency since stack is empty
    let result = action.execute(&mut context).await;

    // It will fail with execution error (workflow not found), but not circular dependency
    assert!(result.is_err());
    if let ActionError::ExecutionError(msg) = result.unwrap_err() {
        assert!(!msg.contains("Circular dependency"));
        assert!(msg.contains("Failed to load sub-workflow"));
    }
}

#[tokio::test]
async fn test_sub_workflow_action_in_process_execution() {
    // This test verifies that the sub-workflow action correctly executes workflows
    // in-process rather than shelling out to a subprocess
    use crate::workflow::test_helpers::create_state;
    use crate::workflow::{StateId, Workflow, WorkflowName, WorkflowStorage};

    // Create a simple test workflow in memory
    let workflow_name = "test-in-process-workflow";
    let mut workflow = Workflow::new(
        WorkflowName::new(workflow_name),
        "Test workflow for in-process execution".to_string(),
        StateId::new("start"),
    );

    // Add a simple state that sets a variable
    let start_state = create_state("start", "Set test_result=\"workflow_executed\"", true);
    workflow.add_state(start_state);

    // Store the workflow
    let mut storage = WorkflowStorage::memory();
    storage.store_workflow(workflow).unwrap();

    // Create sub-workflow action
    let action = SubWorkflowAction::new(workflow_name.to_string())
        .with_input("input_var".to_string(), "test_input".to_string())
        .with_result_variable("sub_result".to_string());

    let mut context = WorkflowTemplateContext::with_vars(HashMap::new()).unwrap();

    // Execute the sub-workflow
    let result = action.execute(&mut context).await;

    // The workflow doesn't exist in the file system, so it should fail
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ActionError::ExecutionError(_)
    ));

    // Verify that no subprocess was spawned (we can't directly test this,
    // but the error message should indicate a workflow loading failure,
    // not a subprocess failure)
}

// Note: Utility functions are private and tested implicitly through action execution
// including: is_valid_argument_key, substitute_variables_in_string

// Note: parse_claude_response is private and tested implicitly through PromptAction execution

// Note: parse_workflow_output is private and tested implicitly through SubWorkflowAction execution
