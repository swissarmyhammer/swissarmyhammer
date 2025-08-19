//! Tests for the workflow executor module

use super::*;
use crate::workflow::test_helpers::*;
use crate::workflow::{
    ConditionType, ErrorContext, StateId, StateType, Transition, TransitionCondition, Workflow,
    WorkflowName, WorkflowRun, WorkflowRunStatus,
};
use serde_json::Value;
use serial_test::serial;
use std::collections::HashMap;

#[cfg(test)]
use crate::test_utils::IsolatedTestEnvironment;

fn create_test_workflow() -> Workflow {
    let mut workflow = Workflow::new(
        WorkflowName::new("Test Workflow"),
        "A test workflow".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state("processing", "Processing state", false));
    workflow.add_state(create_state("end", "End state", true));

    workflow.add_transition(create_transition(
        "start",
        "processing",
        ConditionType::Always,
    ));

    workflow.add_transition(create_transition(
        "processing",
        "end",
        ConditionType::OnSuccess,
    ));

    workflow
}

#[tokio::test]
async fn test_start_workflow() {
    let mut executor = WorkflowExecutor::new();
    let workflow = create_test_workflow();

    let run = executor.start_and_execute_workflow(workflow).await.unwrap();

    assert_eq!(run.workflow.name.as_str(), "Test Workflow");
    // The workflow executes through to completion immediately
    assert_eq!(run.status, WorkflowRunStatus::Completed);
    assert_eq!(run.current_state, StateId::new("end"));
    assert!(!executor.get_history().is_empty());
}

#[tokio::test]
async fn test_workflow_execution_to_completion() {
    let mut executor = WorkflowExecutor::new();
    let workflow = create_test_workflow();

    let run = executor.start_and_execute_workflow(workflow).await.unwrap();

    // The workflow should have executed through to completion
    assert_eq!(run.status, WorkflowRunStatus::Completed);
    assert_eq!(run.current_state, StateId::new("end"));

    // Check execution history
    let history = executor.get_history();
    assert!(history
        .iter()
        .any(|e| matches!(e.event_type, ExecutionEventType::Started)));
    assert!(history
        .iter()
        .any(|e| matches!(e.event_type, ExecutionEventType::Completed)));
}

#[test]
fn test_evaluate_transitions_always_condition() {
    let mut executor = WorkflowExecutor::new();
    let workflow = create_test_workflow();
    let run = WorkflowRun::new(workflow);

    let next_state = executor.evaluate_transitions(&run).unwrap();
    assert_eq!(next_state, Some(StateId::new("processing")));
}

#[tokio::test]
async fn test_resume_completed_workflow_fails() {
    let mut executor = WorkflowExecutor::new();
    let workflow = create_test_workflow();
    let mut run = WorkflowRun::new(workflow);
    run.complete();

    let result = executor.resume_workflow(run).await;
    assert!(matches!(result, Err(ExecutorError::WorkflowCompleted)));
}

#[tokio::test]
async fn test_transition_to_invalid_state() {
    let mut executor = WorkflowExecutor::new();
    let workflow = create_test_workflow();
    let mut run = WorkflowRun::new(workflow);

    let result = executor
        .transition_to(&mut run, StateId::new("non_existent"))
        .await;

    assert!(matches!(result, Err(ExecutorError::StateNotFound(_))));
}

#[tokio::test]
#[serial_test::serial]
async fn test_max_transition_limit() {
    let _test_env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let mut executor = WorkflowExecutor::new();

    // Create a workflow with infinite loop
    let mut workflow = Workflow::new(
        WorkflowName::new("Infinite Loop"),
        "A workflow that loops forever".to_string(),
        StateId::new("loop_state"),
    );

    workflow.add_state(create_state(
        "loop_state",
        "State that loops to itself",
        false,
    ));

    // Add a terminal state to pass validation
    workflow.add_state(create_state("terminal", "Terminal state", true));

    workflow.add_transition(Transition {
        from_state: StateId::new("loop_state"),
        to_state: StateId::new("loop_state"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    let result = executor.start_and_execute_workflow(workflow).await;
    match result {
        Err(ExecutorError::TransitionLimitExceeded { limit }) if limit == MAX_TRANSITIONS => {
            // Test passed
        }
        other => {
            panic!(
                "Expected TransitionLimitExceeded with limit {MAX_TRANSITIONS}, but got: {other:?}"
            );
        }
    }
}

#[test]
fn test_never_condition() {
    let mut executor = WorkflowExecutor::new();
    let workflow = create_test_workflow();
    let run = WorkflowRun::new(workflow);

    let condition = TransitionCondition {
        condition_type: ConditionType::Never,
        expression: None,
    };

    let result = executor
        .evaluate_condition(&condition, &run.context)
        .unwrap();
    assert!(!result);
}

#[test]
fn test_custom_condition_without_expression() {
    let mut executor = WorkflowExecutor::new();
    let run = WorkflowRun::new(create_test_workflow());

    let condition = TransitionCondition {
        condition_type: ConditionType::Custom,
        expression: None,
    };

    let result = executor.evaluate_condition(&condition, &run.context);
    assert!(
        matches!(result, Err(ExecutorError::ExpressionError(msg)) if msg.contains("requires an expression"))
    );
}

#[test]
fn test_execution_history_limit() {
    let mut executor = WorkflowExecutor::new();
    executor.set_max_history_size(10); // Set small limit for testing

    // Add many events to trigger trimming
    for i in 0..20 {
        executor.log_event(ExecutionEventType::Started, format!("Event {i}"));
    }

    // History should be trimmed to stay under limit
    assert!(executor.get_history().len() <= 10);
}

#[tokio::test]
async fn test_fork_join_parallel_execution() {
    let _test_env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let mut executor = WorkflowExecutor::new();

    // Create a workflow with fork and join
    let mut workflow = Workflow::new(
        WorkflowName::new("Fork Join Test"),
        "Test parallel execution".to_string(),
        StateId::new("start"),
    );

    // Add states
    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state_with_type(
        "fork1",
        "Fork state",
        StateType::Fork,
        false,
    ));
    workflow.add_state(create_state("branch1", "Branch 1", false));
    workflow.add_state(create_state("branch2", "Branch 2", false));
    workflow.add_state(create_state_with_type(
        "join1",
        "Join state",
        StateType::Join,
        false,
    ));
    workflow.add_state(create_state("end", "End state", true));

    // Add transitions
    workflow.add_transition(Transition {
        from_state: StateId::new("start"),
        to_state: StateId::new("fork1"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    workflow.add_transition(Transition {
        from_state: StateId::new("fork1"),
        to_state: StateId::new("branch1"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    workflow.add_transition(Transition {
        from_state: StateId::new("fork1"),
        to_state: StateId::new("branch2"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    workflow.add_transition(Transition {
        from_state: StateId::new("branch1"),
        to_state: StateId::new("join1"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    workflow.add_transition(Transition {
        from_state: StateId::new("branch2"),
        to_state: StateId::new("join1"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    workflow.add_transition(Transition {
        from_state: StateId::new("join1"),
        to_state: StateId::new("end"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    let run = executor.start_and_execute_workflow(workflow).await.unwrap();

    // After execution, workflow should be completed
    assert_eq!(run.status, WorkflowRunStatus::Completed);
    assert_eq!(run.current_state, StateId::new("end"));

    // History should show parallel branch execution
    let history = executor.get_history();

    // Should have events for both branches
    assert!(history.iter().any(|e| e.details.contains("branch1")));
    assert!(history.iter().any(|e| e.details.contains("branch2")));
}

#[tokio::test]
async fn test_fork_join_context_merging() {
    let mut executor = WorkflowExecutor::new();

    // Create a workflow with fork and join that sets variables in parallel branches
    let mut workflow = Workflow::new(
        WorkflowName::new("Context Merge Test"),
        "Test context merging at join".to_string(),
        StateId::new("start"),
    );

    // Add states with actions that set variables
    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state_with_type(
        "fork1",
        "Fork state",
        StateType::Fork,
        false,
    ));
    workflow.add_state(create_state(
        "branch1",
        "Set branch1_result=\"success\"",
        false,
    ));
    workflow.add_state(create_state(
        "branch2",
        "Set branch2_result=\"success\"",
        false,
    ));
    workflow.add_state(create_state_with_type(
        "join1",
        "Join state",
        StateType::Join,
        false,
    ));
    workflow.add_state(create_state("end", "End state", true));

    // Add transitions (same as previous test)
    workflow.add_transition(Transition {
        from_state: StateId::new("start"),
        to_state: StateId::new("fork1"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    workflow.add_transition(Transition {
        from_state: StateId::new("fork1"),
        to_state: StateId::new("branch1"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    workflow.add_transition(Transition {
        from_state: StateId::new("fork1"),
        to_state: StateId::new("branch2"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    workflow.add_transition(Transition {
        from_state: StateId::new("branch1"),
        to_state: StateId::new("join1"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    workflow.add_transition(Transition {
        from_state: StateId::new("branch2"),
        to_state: StateId::new("join1"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    workflow.add_transition(Transition {
        from_state: StateId::new("join1"),
        to_state: StateId::new("end"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    let run = executor.start_and_execute_workflow(workflow).await.unwrap();

    // After execution, both branch variables should be in the final context
    assert!(run.context.contains_key("branch1_result"));
    assert!(run.context.contains_key("branch2_result"));
    assert_eq!(run.status, WorkflowRunStatus::Completed);
}

#[test]
fn test_on_success_condition_with_context() {
    let mut executor = WorkflowExecutor::new();
    let mut context = HashMap::new();
    context.insert(
        LAST_ACTION_RESULT_KEY.to_string(),
        serde_json::Value::Bool(true),
    );

    let condition = TransitionCondition {
        condition_type: ConditionType::OnSuccess,
        expression: None,
    };

    let result = executor.evaluate_condition(&condition, &context).unwrap();
    assert!(result);

    // Test with false result
    context.insert(
        LAST_ACTION_RESULT_KEY.to_string(),
        serde_json::Value::Bool(false),
    );
    let result = executor.evaluate_condition(&condition, &context).unwrap();
    assert!(!result);
}

#[test]
fn test_on_failure_condition_with_context() {
    let mut executor = WorkflowExecutor::new();
    let mut context = HashMap::new();
    context.insert(
        LAST_ACTION_RESULT_KEY.to_string(),
        serde_json::Value::Bool(false),
    );

    let condition = TransitionCondition {
        condition_type: ConditionType::OnFailure,
        expression: None,
    };

    let result = executor.evaluate_condition(&condition, &context).unwrap();
    assert!(result);

    // Test with true result
    context.insert(
        LAST_ACTION_RESULT_KEY.to_string(),
        serde_json::Value::Bool(true),
    );
    let result = executor.evaluate_condition(&condition, &context).unwrap();
    assert!(!result);
}

#[test]
fn test_cel_expression_evaluation() {
    let mut executor = WorkflowExecutor::new();
    let mut context = HashMap::new();
    context.insert(
        "result".to_string(),
        serde_json::Value::String("ok".to_string()),
    );

    // Test simple string comparison
    let condition = TransitionCondition {
        condition_type: ConditionType::Custom,
        expression: Some("result == \"ok\"".to_string()),
    };

    let result = executor.evaluate_condition(&condition, &context).unwrap();
    assert!(result);

    // Test default condition
    let condition = TransitionCondition {
        condition_type: ConditionType::Custom,
        expression: Some("default".to_string()),
    };

    let result = executor.evaluate_condition(&condition, &context).unwrap();
    assert!(result);
}

#[test]
fn test_cel_expression_with_variables() {
    let mut executor = WorkflowExecutor::new();
    let mut context = HashMap::new();
    context.insert(
        "count".to_string(),
        serde_json::Value::Number(serde_json::Number::from(5)),
    );
    context.insert(
        "status".to_string(),
        serde_json::Value::String("active".to_string()),
    );

    // Test numeric comparison
    let condition = TransitionCondition {
        condition_type: ConditionType::Custom,
        expression: Some("count > 3".to_string()),
    };

    let result = executor.evaluate_condition(&condition, &context).unwrap();
    assert!(result);

    // Test string comparison
    let condition = TransitionCondition {
        condition_type: ConditionType::Custom,
        expression: Some("status == \"active\"".to_string()),
    };

    let result = executor.evaluate_condition(&condition, &context).unwrap();
    assert!(result);

    // Test complex expression
    let condition = TransitionCondition {
        condition_type: ConditionType::Custom,
        expression: Some("count > 3 && status == \"active\"".to_string()),
    };

    let result = executor.evaluate_condition(&condition, &context).unwrap();
    assert!(result);
}

#[test]
fn test_cel_expression_invalid_syntax() {
    let mut executor = WorkflowExecutor::new();
    let context = HashMap::new();

    let condition = TransitionCondition {
        condition_type: ConditionType::Custom,
        expression: Some("invalid == == syntax".to_string()),
    };

    let result = executor.evaluate_condition(&condition, &context);
    assert!(matches!(result, Err(ExecutorError::ExpressionError(_))));
}

#[test]
fn test_cel_expression_suspicious_quotes() {
    let mut executor = WorkflowExecutor::new();
    let context = HashMap::new();

    // Test triple quotes
    let condition = TransitionCondition {
        condition_type: ConditionType::Custom,
        expression: Some("\"\"\"dangerous\"\"\"".to_string()),
    };

    let result = executor.evaluate_condition(&condition, &context);
    assert!(
        matches!(result, Err(ExecutorError::ExpressionError(msg)) if msg.contains("suspicious quote"))
    );
}

#[test]
fn test_choice_state_determinism_validation() {
    let mut executor = WorkflowExecutor::new();

    // Create a workflow with ambiguous choice state conditions
    let mut workflow = Workflow::new(
        WorkflowName::new("Ambiguous Choice Test"),
        "Test ambiguous choice state validation".to_string(),
        StateId::new("start"),
    );

    // Add states
    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state_with_type(
        "choice1",
        "Ambiguous choice state",
        StateType::Choice,
        false,
    ));
    workflow.add_state(create_state("success1", "Success state 1", true));
    workflow.add_state(create_state("success2", "Success state 2", true));

    // Add transition to choice state
    workflow.add_transition(Transition {
        from_state: StateId::new("start"),
        to_state: StateId::new("choice1"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    // Add two OnSuccess conditions - this should be ambiguous
    workflow.add_transition(Transition {
        from_state: StateId::new("choice1"),
        to_state: StateId::new("success1"),
        condition: TransitionCondition {
            condition_type: ConditionType::OnSuccess,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    workflow.add_transition(Transition {
        from_state: StateId::new("choice1"),
        to_state: StateId::new("success2"),
        condition: TransitionCondition {
            condition_type: ConditionType::OnSuccess,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    let mut run = WorkflowRun::new(workflow);

    // Transition to the choice state first
    run.transition_to(StateId::new("choice1"));

    let result = executor.evaluate_transitions(&run);

    // Should fail due to ambiguous conditions
    assert!(
        matches!(result, Err(ExecutorError::ExecutionFailed(msg)) if msg.contains("ambiguous conditions"))
    );
}

#[test]
fn test_choice_state_never_condition_validation() {
    let mut executor = WorkflowExecutor::new();

    // Create a workflow with Never condition in choice state
    let mut workflow = Workflow::new(
        WorkflowName::new("Never Choice Test"),
        "Test Never condition in choice state".to_string(),
        StateId::new("start"),
    );

    // Add states
    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state_with_type(
        "choice1",
        "Choice state with Never",
        StateType::Choice,
        false,
    ));
    workflow.add_state(create_state("never_state", "Never reached", true));

    // Add transition to choice state
    workflow.add_transition(Transition {
        from_state: StateId::new("start"),
        to_state: StateId::new("choice1"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    // Add Never condition - should be flagged as error
    workflow.add_transition(Transition {
        from_state: StateId::new("choice1"),
        to_state: StateId::new("never_state"),
        condition: TransitionCondition {
            condition_type: ConditionType::Never,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    let mut run = WorkflowRun::new(workflow);

    // Transition to the choice state first
    run.transition_to(StateId::new("choice1"));

    let result = executor.evaluate_transitions(&run);

    // Should fail due to Never condition in choice state
    assert!(
        matches!(result, Err(ExecutorError::ExecutionFailed(msg)) if msg.contains("Never conditions"))
    );
}

#[tokio::test]
#[serial_test::serial]
async fn test_choice_state_execution() {
    let _test_env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let mut executor = WorkflowExecutor::new();

    // Create a workflow with a choice state
    let mut workflow = Workflow::new(
        WorkflowName::new("Choice State Test"),
        "Test choice state execution".to_string(),
        StateId::new("start"),
    );

    // Add states
    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state_with_type(
        "choice1",
        "Choice state",
        StateType::Choice,
        false,
    ));
    workflow.add_state(create_state("success", "Success state", true));
    workflow.add_state(create_state("failure", "Failure state", true));

    // Add transitions
    workflow.add_transition(Transition {
        from_state: StateId::new("start"),
        to_state: StateId::new("choice1"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    // Choice state with success condition first
    workflow.add_transition(Transition {
        from_state: StateId::new("choice1"),
        to_state: StateId::new("success"),
        condition: TransitionCondition {
            condition_type: ConditionType::OnSuccess,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    // Choice state with default condition as fallback
    workflow.add_transition(Transition {
        from_state: StateId::new("choice1"),
        to_state: StateId::new("failure"),
        condition: TransitionCondition {
            condition_type: ConditionType::Custom,
            expression: Some("default".to_string()),
        },
        action: None,
        metadata: HashMap::new(),
    });

    let run = executor.start_and_execute_workflow(workflow).await.unwrap();

    // Should go to success state since OnSuccess defaults to true
    assert_eq!(run.status, WorkflowRunStatus::Completed);
    assert_eq!(run.current_state, StateId::new("success"));
}

#[tokio::test]
async fn test_choice_state_with_cel_conditions() {
    let mut executor = WorkflowExecutor::new();

    // Create a workflow with a choice state using CEL expressions
    let mut workflow = Workflow::new(
        WorkflowName::new("Choice State CEL Test"),
        "Test choice state with CEL conditions".to_string(),
        StateId::new("start"),
    );

    // Add states
    workflow.add_state(create_state("start", "Set result=\"ok\"", false));
    workflow.add_state(create_state_with_type(
        "choice1",
        "Choice state with CEL",
        StateType::Choice,
        false,
    ));
    workflow.add_state(create_state("success", "Success state", true));
    workflow.add_state(create_state("failure", "Failure state", true));

    // Add transitions
    workflow.add_transition(Transition {
        from_state: StateId::new("start"),
        to_state: StateId::new("choice1"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    // Choice state with CEL condition that checks result
    workflow.add_transition(Transition {
        from_state: StateId::new("choice1"),
        to_state: StateId::new("success"),
        condition: TransitionCondition {
            condition_type: ConditionType::Custom,
            expression: Some("result == \"ok\"".to_string()),
        },
        action: None,
        metadata: HashMap::new(),
    });

    // Choice state with default condition as fallback
    workflow.add_transition(Transition {
        from_state: StateId::new("choice1"),
        to_state: StateId::new("failure"),
        condition: TransitionCondition {
            condition_type: ConditionType::Custom,
            expression: Some("default".to_string()),
        },
        action: None,
        metadata: HashMap::new(),
    });

    let run = executor.start_and_execute_workflow(workflow).await.unwrap();

    // Should go to success state since start state sets result="ok"
    assert_eq!(run.status, WorkflowRunStatus::Completed);
    assert_eq!(run.current_state, StateId::new("success"));
}

#[tokio::test]
async fn test_choice_state_no_matching_conditions() {
    let mut executor = WorkflowExecutor::new();

    // Create a workflow with a choice state where no conditions match
    let mut workflow = Workflow::new(
        WorkflowName::new("Choice State No Match"),
        "Test choice state with no matching conditions".to_string(),
        StateId::new("start"),
    );

    // Add states
    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state_with_type(
        "choice1",
        "Choice state",
        StateType::Choice,
        false,
    ));
    workflow.add_state(create_state("success", "Success state", true));

    // Add transitions
    workflow.add_transition(Transition {
        from_state: StateId::new("start"),
        to_state: StateId::new("choice1"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    // Choice state with condition that will never match
    workflow.add_transition(Transition {
        from_state: StateId::new("choice1"),
        to_state: StateId::new("success"),
        condition: TransitionCondition {
            condition_type: ConditionType::Never,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    let result = executor.start_and_execute_workflow(workflow).await;
    assert!(matches!(result, Err(ExecutorError::ExecutionFailed(_))));
}

#[tokio::test]
async fn test_choice_state_no_transitions() {
    let mut executor = WorkflowExecutor::new();

    // Create a workflow with a choice state that has no outgoing transitions
    let mut workflow = Workflow::new(
        WorkflowName::new("Choice State No Transitions"),
        "Test choice state with no transitions".to_string(),
        StateId::new("start"),
    );

    // Add states
    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state_with_type(
        "choice1",
        "Choice state",
        StateType::Choice,
        false,
    ));
    workflow.add_state(create_state("success", "Success state", true));

    // Add transition to choice state but no transitions from it
    workflow.add_transition(Transition {
        from_state: StateId::new("start"),
        to_state: StateId::new("choice1"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    let result = executor.start_and_execute_workflow(workflow).await;
    assert!(matches!(result, Err(ExecutorError::ExecutionFailed(_))));
}

#[test]
fn test_transition_order_evaluation() {
    let mut executor = WorkflowExecutor::new();

    // Create a workflow with multiple transitions from the same state
    let mut workflow = Workflow::new(
        WorkflowName::new("Transition Order Test"),
        "Test transition order evaluation".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state("first", "First state", true));
    workflow.add_state(create_state("second", "Second state", true));

    // Add transitions in specific order - first should always win
    workflow.add_transition(Transition {
        from_state: StateId::new("start"),
        to_state: StateId::new("first"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    workflow.add_transition(Transition {
        from_state: StateId::new("start"),
        to_state: StateId::new("second"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    let run = WorkflowRun::new(workflow);
    let next_state = executor.evaluate_transitions(&run).unwrap();

    // Should select the first transition (to "first" state)
    assert_eq!(next_state, Some(StateId::new("first")));
}

#[test]
fn test_cel_expression_security_validation() {
    let mut executor = WorkflowExecutor::new();
    let context = HashMap::new();

    // Test forbidden patterns
    let forbidden_patterns = ["import", "eval", "exec", "system", "file", "delete"];

    for pattern in forbidden_patterns {
        let condition = TransitionCondition {
            condition_type: ConditionType::Custom,
            expression: Some(format!("{pattern} == true")),
        };

        let result = executor.evaluate_condition(&condition, &context);
        assert!(
            matches!(result, Err(ExecutorError::ExpressionError(msg)) if msg.contains("forbidden pattern"))
        );
    }
}

#[test]
fn test_cel_expression_length_limits() {
    let mut executor = WorkflowExecutor::new();
    let context = HashMap::new();

    // Test expression length validation
    let long_expression = "a == ".repeat(200) + "\"test\"";
    let condition = TransitionCondition {
        condition_type: ConditionType::Custom,
        expression: Some(long_expression),
    };

    let result = executor.evaluate_condition(&condition, &context);
    assert!(matches!(result, Err(ExecutorError::ExpressionError(msg)) if msg.contains("too long")));
}

#[test]
fn test_cel_expression_nesting_limits() {
    let mut executor = WorkflowExecutor::new();
    let context = HashMap::new();

    // Test excessive nesting
    let nested_expression = "(".repeat(15) + "true" + &")".repeat(15);
    let condition = TransitionCondition {
        condition_type: ConditionType::Custom,
        expression: Some(nested_expression),
    };

    let result = executor.evaluate_condition(&condition, &context);
    assert!(
        matches!(result, Err(ExecutorError::ExpressionError(msg)) if msg.contains("excessive nesting"))
    );
}

#[test]
fn test_cel_expression_caching_behavior() {
    let mut executor = WorkflowExecutor::new();
    let context = HashMap::new();

    let condition = TransitionCondition {
        condition_type: ConditionType::Custom,
        expression: Some("default".to_string()),
    };

    // Evaluate the same expression multiple times
    let result1 = executor.evaluate_condition(&condition, &context);
    let result2 = executor.evaluate_condition(&condition, &context);

    assert!(result1.is_ok());
    assert!(result2.is_ok());

    // Verify the expression is cached
    assert!(executor.get_compiled_cel_program("default").is_ok());
}

#[test]
fn test_cel_expression_complex_json_handling() {
    let mut executor = WorkflowExecutor::new();
    let mut context = HashMap::new();

    // Add complex JSON structures
    let array_value = serde_json::Value::Array(vec![
        serde_json::Value::Number(serde_json::Number::from(1)),
        serde_json::Value::Number(serde_json::Number::from(2)),
    ]);
    context.insert("numbers".to_string(), array_value);

    let mut nested_object = serde_json::Map::new();
    nested_object.insert(
        "key".to_string(),
        serde_json::Value::String("value".to_string()),
    );
    context.insert(
        "nested".to_string(),
        serde_json::Value::Object(nested_object),
    );

    // Test that complex structures are handled gracefully
    let condition = TransitionCondition {
        condition_type: ConditionType::Custom,
        expression: Some("numbers != null".to_string()),
    };

    let result = executor.evaluate_condition(&condition, &context);
    // Should either work or fail gracefully
    match result {
        Ok(_) => {}                                  // Success
        Err(ExecutorError::ExpressionError(_)) => {} // Expected for some cases
        _ => panic!("Unexpected error type"),
    }
}

// ========== Error Handling and Recovery Tests ==========

#[tokio::test]
async fn test_retry_with_exponential_backoff() {
    let mut executor = WorkflowExecutor::new();
    let mut workflow = Workflow::new(
        WorkflowName::new("Retry Test"),
        "Test retry with backoff".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    // Use an invalid prompt that will fail
    workflow.add_state(create_state(
        "failing",
        "Execute prompt \"nonexistent-prompt\" with test=\"value\"",
        false,
    ));
    workflow.add_state(create_state("end", "End state", true));

    // Add transition without retry policy (retry logic removed)
    let metadata = HashMap::new();

    workflow.add_transition(Transition {
        from_state: StateId::new("start"),
        to_state: StateId::new("failing"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata,
    });

    workflow.add_transition(create_transition(
        "failing",
        "end",
        ConditionType::OnSuccess,
    ));

    let result = executor.start_and_execute_workflow(workflow).await;

    // Should fail after retries
    assert!(result.is_err());

    // Check that retries occurred
    let history = executor.get_history();
    let retry_events: Vec<_> = history
        .iter()
        .filter(|e| e.details.contains("Retry attempt"))
        .collect();

    // Should have 0 retry attempts (retry logic removed)
    assert_eq!(retry_events.len(), 0);

    // Should not have any backoff timing messages
    assert!(!history.iter().any(|e| e.details.contains("waiting")));
}

#[tokio::test]
async fn test_fallback_state_on_error() {
    let mut executor = WorkflowExecutor::new();
    let mut workflow = Workflow::new(
        WorkflowName::new("Fallback Test"),
        "Test fallback state".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state(
        "primary",
        "Execute prompt \"nonexistent-prompt\"",
        false,
    )); // This will fail
    workflow.add_state(create_state(
        "fallback",
        "Log \"Executing fallback\"",
        false,
    ));
    workflow.add_state(create_state("end", "End state", true));

    workflow.add_transition(create_transition("start", "primary", ConditionType::Always));
    workflow.add_transition(create_transition(
        "primary",
        "end",
        ConditionType::OnSuccess,
    ));
    workflow.add_transition(create_transition(
        "primary",
        "fallback",
        ConditionType::OnFailure,
    ));
    workflow.add_transition(create_transition("fallback", "end", ConditionType::Always));

    let run = executor.start_and_execute_workflow(workflow).await.unwrap();

    // Should have executed through fallback path
    assert_eq!(run.status, WorkflowRunStatus::Completed);

    // Check that fallback was executed
    let history = executor.get_history();
    assert!(history.iter().any(|e| e.details.contains("fallback")));
}

#[tokio::test]
async fn test_error_handler_state() {
    let mut executor = WorkflowExecutor::new();
    let mut workflow = Workflow::new(
        WorkflowName::new("Error Handler Test"),
        "Test error handler state".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state(
        "process",
        "Execute prompt \"nonexistent-prompt\"",
        false,
    )); // This will fail
    workflow.add_state(create_state_with_type(
        "process_error",
        "Handle error",
        StateType::Normal,
        false,
    ));
    workflow.add_state(create_state("end", "End state", true));

    workflow.add_transition(create_transition("start", "process", ConditionType::Always));
    workflow.add_transition(create_transition(
        "process",
        "end",
        ConditionType::OnSuccess,
    ));
    workflow.add_transition(create_transition(
        "process",
        "process_error",
        ConditionType::OnFailure,
    ));
    workflow.add_transition(create_transition(
        "process_error",
        "end",
        ConditionType::Always,
    ));

    let run = executor.start_and_execute_workflow(workflow).await.unwrap();

    assert_eq!(run.status, WorkflowRunStatus::Completed);

    // Verify error handler was executed
    let history = executor.get_history();
    assert!(history.iter().any(|e| e.details.contains("process_error")));
}

#[tokio::test]
async fn test_compensation_rollback() {
    let mut executor = WorkflowExecutor::new();
    let mut workflow = Workflow::new(
        WorkflowName::new("Compensation Test"),
        "Test compensation/rollback".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state("step1", "Log \"Step 1 executed\"", false));
    workflow.add_state(create_state(
        "step2",
        "Execute prompt \"nonexistent-prompt\"",
        false,
    )); // This will fail
    workflow.add_state(create_state(
        "compensate_step1",
        "Log \"Compensating step 1\"",
        false,
    ));
    workflow.add_state(create_state("failed", "Failed state", true));

    // Define compensation metadata
    let mut comp_metadata = HashMap::new();
    comp_metadata.insert(
        "compensation_state".to_string(),
        "compensate_step1".to_string(),
    );

    workflow.add_transition(Transition {
        from_state: StateId::new("start"),
        to_state: StateId::new("step1"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: comp_metadata,
    });

    workflow.add_transition(create_transition(
        "step1",
        "step2",
        ConditionType::OnSuccess,
    ));
    workflow.add_transition(create_transition(
        "step2",
        "failed",
        ConditionType::OnFailure,
    ));
    workflow.add_transition(create_transition(
        "compensate_step1",
        "failed",
        ConditionType::Always,
    ));

    let _run = executor.start_and_execute_workflow(workflow).await.unwrap();

    // Verify compensation was executed
    let history = executor.get_history();
    assert!(history
        .iter()
        .any(|e| e.details.contains("compensate_step1")));
}

#[tokio::test]
async fn test_error_context_capture() {
    let mut executor = WorkflowExecutor::new();
    let mut workflow = Workflow::new(
        WorkflowName::new("Error Context Test"),
        "Test error context capture".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state(
        "failing",
        "Execute prompt \"nonexistent-prompt\"",
        false,
    ));
    workflow.add_state(create_state(
        "error_handler",
        "Log \"Handling error\"",
        false,
    ));
    workflow.add_state(create_state("end", "End state", true));

    workflow.add_transition(create_transition("start", "failing", ConditionType::Always));
    workflow.add_transition(create_transition(
        "failing",
        "end",
        ConditionType::OnSuccess,
    ));
    workflow.add_transition(create_transition(
        "failing",
        "error_handler",
        ConditionType::OnFailure,
    ));
    workflow.add_transition(create_transition(
        "error_handler",
        "end",
        ConditionType::Always,
    ));

    let run = executor.start_and_execute_workflow(workflow).await.unwrap();

    // Check error context was captured
    assert!(run.context.contains_key(ErrorContext::CONTEXT_KEY));

    // Verify error context structure
    if let Some(error_context_value) = run.context.get(ErrorContext::CONTEXT_KEY) {
        let error_context: ErrorContext = serde_json::from_value(error_context_value.clone())
            .expect("Should be able to deserialize error context");
        assert!(!error_context.error_message.is_empty());
        assert_eq!(error_context.error_state, StateId::new("failing"));
        assert!(!error_context.error_timestamp.is_empty());
    }
}

#[tokio::test]
#[serial]
async fn test_manual_intervention_recovery() {
    let _test_env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let mut executor = WorkflowExecutor::new();
    let mut workflow = Workflow::new(
        WorkflowName::new("Manual Recovery Test"),
        "Test manual intervention".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state("process", "Process data", false));

    // Manual intervention state
    let mut metadata = HashMap::new();
    metadata.insert(
        "requires_manual_intervention".to_string(),
        "true".to_string(),
    );

    let mut intervention_state =
        create_state("manual_check", "Manual intervention required", false);
    intervention_state.metadata = metadata;
    workflow.add_state(intervention_state);

    workflow.add_state(create_state("end", "End state", true));

    workflow.add_transition(create_transition("start", "process", ConditionType::Always));
    workflow.add_transition(create_transition(
        "process",
        "manual_check",
        ConditionType::Always,
    ));
    workflow.add_transition(create_transition(
        "manual_check",
        "end",
        ConditionType::Always,
    ));

    let mut run = executor.start_and_execute_workflow(workflow).await.unwrap();

    // Should pause at manual intervention
    assert_eq!(run.status, WorkflowRunStatus::Running);
    assert_eq!(run.current_state, StateId::new("manual_check"));

    // Simulate manual approval
    run.context
        .insert("manual_approval".to_string(), Value::Bool(true));

    // Resume workflow
    let completed_run = executor.resume_workflow(run).await.unwrap();
    assert_eq!(completed_run.status, WorkflowRunStatus::Completed);
}

#[tokio::test]
async fn test_skip_failed_state() {
    let _test_env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let mut executor = WorkflowExecutor::new();
    let mut workflow = Workflow::new(
        WorkflowName::new("Skip Failed Test"),
        "Test skip failed state".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state(
        "optional_step",
        "Execute prompt \"nonexistent-prompt\"",
        false,
    ));
    workflow.add_state(create_state(
        "continue",
        "Log \"Continuing after skip\"",
        false,
    ));
    workflow.add_state(create_state("end", "End state", true));

    // Mark optional step as skippable on failure
    let mut metadata = HashMap::new();
    metadata.insert("skip_on_failure".to_string(), "true".to_string());

    workflow.add_transition(Transition {
        from_state: StateId::new("start"),
        to_state: StateId::new("optional_step"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata,
    });

    workflow.add_transition(create_transition(
        "optional_step",
        "continue",
        ConditionType::Always,
    ));
    workflow.add_transition(create_transition("continue", "end", ConditionType::Always));

    let run = executor.start_and_execute_workflow(workflow).await.unwrap();

    // Should complete despite failure in optional step
    assert_eq!(run.status, WorkflowRunStatus::Completed);

    // Verify skip was recorded
    let history = executor.get_history();
    assert!(history
        .iter()
        .any(|e| e.details.contains("Skipped failed state")));
}

#[tokio::test]
async fn test_dead_letter_state() {
    let mut executor = WorkflowExecutor::new();
    let mut workflow = Workflow::new(
        WorkflowName::new("Dead Letter Test"),
        "Test dead letter state".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state(
        "process",
        "Execute prompt \"nonexistent-prompt\"",
        false,
    ));
    workflow.add_state(create_state_with_type(
        "dead_letter",
        "Log \"Message sent to dead letter queue\"",
        StateType::Normal,
        true,
    ));

    // Configure dead letter (retry logic removed)
    let mut metadata = HashMap::new();
    metadata.insert("dead_letter_state".to_string(), "dead_letter".to_string());

    workflow.add_transition(Transition {
        from_state: StateId::new("start"),
        to_state: StateId::new("process"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata,
    });

    let mut run = executor.start_and_execute_workflow(workflow).await.unwrap();

    // Should have transitioned to dead letter state
    assert_eq!(run.current_state, StateId::new("dead_letter"));

    // Verify error details are preserved (retry_attempts no longer exists)
    assert!(run.context.contains_key("dead_letter_reason"));
    // retry_attempts is no longer part of the context since retry logic was removed

    // Resume to complete the dead letter state execution
    run = executor.resume_workflow(run).await.unwrap();
    assert_eq!(run.status, WorkflowRunStatus::Completed);
}

#[tokio::test]
async fn test_say_hello_workflow() {
    let mut executor = WorkflowExecutor::new();

    // Create a simple workflow that outputs the hello message
    let mut workflow = Workflow::new(
        WorkflowName::new("Say Hello Test"),
        "Test that outputs hello message".to_string(),
        StateId::new("start"),
    );

    // Add states
    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state(
        "say_hello",
        "Log \"Hello from Swiss Army Hammer! The workflow system is working correctly.\"",
        false,
    ));
    workflow.add_state(create_state("end", "End state", true));

    // Add transitions
    workflow.add_transition(create_transition(
        "start",
        "say_hello",
        ConditionType::Always,
    ));
    workflow.add_transition(create_transition("say_hello", "end", ConditionType::Always));

    // Execute the workflow
    let run = executor.start_and_execute_workflow(workflow).await.unwrap();

    // Verify the workflow completed successfully
    assert_eq!(run.status, WorkflowRunStatus::Completed);
    assert_eq!(run.current_state, StateId::new("end"));

    // Check that the hello message was logged in the execution history
    let history = executor.get_history();
    assert!(history.iter().any(|e| e
        .details
        .contains("Hello from Swiss Army Hammer! The workflow system is working correctly.")));
}

#[tokio::test]
#[serial_test::serial]
async fn test_abort_file_detection() {
    let _test_env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let mut executor = WorkflowExecutor::new();

    // Create a workflow with multiple states that would normally complete
    let mut workflow = Workflow::new(
        WorkflowName::new("Abort Test"),
        "Test workflow abort detection".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state("step1", "Step 1 state", false));
    workflow.add_state(create_state("step2", "Step 2 state", false));
    workflow.add_state(create_state("step3", "Step 3 state", false));
    workflow.add_state(create_state("end", "End state", true));

    workflow.add_transition(create_transition("start", "step1", ConditionType::Always));
    workflow.add_transition(create_transition("step1", "step2", ConditionType::Always));
    workflow.add_transition(create_transition("step2", "step3", ConditionType::Always));
    workflow.add_transition(create_transition("step3", "end", ConditionType::Always));

    // Start the workflow run manually to get past the cleanup
    let mut run = executor.start_workflow(workflow).unwrap();

    // Create the abort file in the isolated test directory
    std::fs::create_dir_all(".swissarmyhammer")
        .or_else(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(e)
            }
        })
        .unwrap();
    std::fs::write(".swissarmyhammer/.abort", "Test abort reason").unwrap();

    // Now execute with the abort file present
    let result = executor
        .execute_state_with_limit(&mut run, 1000)
        .await
        .map(|_| run);

    // Should return an abort error
    assert!(matches!(result, Err(ExecutorError::Abort(_))));

    if let Err(ExecutorError::Abort(reason)) = result {
        assert_eq!(reason, "Test abort reason");
    }
}

#[tokio::test]
#[serial_test::serial]
async fn test_abort_file_detection_with_read_error() {
    let _test_env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let mut executor = WorkflowExecutor::new();

    // Create a simple workflow
    let mut workflow = Workflow::new(
        WorkflowName::new("Abort Read Error Test"),
        "Test workflow abort with read error".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state("end", "End state", true));

    workflow.add_transition(create_transition("start", "end", ConditionType::Always));

    // Start the workflow run manually to get past the cleanup
    let mut run = executor.start_workflow(workflow).unwrap();

    // Create abort file but make it unreadable (simulate read error)
    std::fs::create_dir_all(".swissarmyhammer")
        .or_else(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(e)
            }
        })
        .unwrap();
    std::fs::write(".swissarmyhammer/.abort", "").unwrap();

    let result = executor
        .execute_state_with_limit(&mut run, 1000)
        .await
        .map(|_| run);

    // Should return abort error with fallback message when file exists but is empty/unreadable
    assert!(matches!(result, Err(ExecutorError::Abort(_))));
}

#[tokio::test]
#[serial_test::serial]
async fn test_abort_file_detection_during_multiple_state_transitions() {
    let _test_env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let mut executor = WorkflowExecutor::new();

    // Create a simple workflow (same as working test)
    let mut workflow = Workflow::new(
        WorkflowName::new("Multi State Abort Test"),
        "Test abort during multiple transitions".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state("end", "End state", true));

    workflow.add_transition(create_transition("start", "end", ConditionType::Always));

    // Start the workflow run manually to get past the cleanup
    let mut run = executor.start_workflow(workflow).unwrap();

    // Create the abort file in the isolated test directory
    std::fs::create_dir_all(".swissarmyhammer")
        .or_else(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(e)
            }
        })
        .unwrap();
    std::fs::write(".swissarmyhammer/.abort", "Mid-execution abort").unwrap();

    // Now execute with the abort file present
    let result = executor
        .execute_state_with_limit(&mut run, 1000)
        .await
        .map(|_| run);

    // Should return an abort error
    assert!(matches!(result, Err(ExecutorError::Abort(_))));

    if let Err(ExecutorError::Abort(reason)) = result {
        assert_eq!(reason, "Mid-execution abort");
    }
}

#[tokio::test]
#[serial_test::serial]
async fn test_abort_file_detection_with_unicode_reason() {
    let _test_env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let mut executor = WorkflowExecutor::new();

    let mut workflow = Workflow::new(
        WorkflowName::new("Unicode Abort Test"),
        "Test abort with unicode content".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state("end", "End state", true));
    workflow.add_transition(create_transition("start", "end", ConditionType::Always));

    let mut run = executor.start_workflow(workflow).unwrap();

    let unicode_reason = "中文测试 🚫 Abort with émojis and ñoñ-ASCII";
    std::fs::create_dir_all(".swissarmyhammer")
        .or_else(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(e)
            }
        })
        .unwrap();
    std::fs::write(".swissarmyhammer/.abort", unicode_reason).unwrap();

    let result = executor
        .execute_state_with_limit(&mut run, 1000)
        .await
        .map(|_| run);

    assert!(matches!(result, Err(ExecutorError::Abort(_))));
    if let Err(ExecutorError::Abort(reason)) = result {
        assert_eq!(reason, unicode_reason);
    }
}

#[tokio::test]
#[serial_test::serial]
async fn test_abort_file_detection_with_large_reason() {
    let _test_env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let mut executor = WorkflowExecutor::new();

    let mut workflow = Workflow::new(
        WorkflowName::new("Large Reason Abort Test"),
        "Test abort with large content".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state("end", "End state", true));
    workflow.add_transition(create_transition("start", "end", ConditionType::Always));

    let mut run = executor.start_workflow(workflow).unwrap();

    let large_reason = "x".repeat(5000);
    std::fs::create_dir_all(".swissarmyhammer")
        .or_else(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(e)
            }
        })
        .unwrap();
    std::fs::write(".swissarmyhammer/.abort", &large_reason).unwrap();

    let result = executor
        .execute_state_with_limit(&mut run, 1000)
        .await
        .map(|_| run);

    assert!(matches!(result, Err(ExecutorError::Abort(_))));
    if let Err(ExecutorError::Abort(reason)) = result {
        assert_eq!(reason, large_reason);
    }
}

#[tokio::test]
#[serial_test::serial]
async fn test_abort_file_detection_with_newlines() {
    let _test_env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let mut executor = WorkflowExecutor::new();

    let mut workflow = Workflow::new(
        WorkflowName::new("Newline Abort Test"),
        "Test abort with newlines".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state("end", "End state", true));
    workflow.add_transition(create_transition("start", "end", ConditionType::Always));

    let mut run = executor.start_workflow(workflow).unwrap();

    let reason_with_newlines = "Line 1\nLine 2\r\nLine 3\n";
    std::fs::create_dir_all(".swissarmyhammer")
        .or_else(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(e)
            }
        })
        .unwrap();
    std::fs::write(".swissarmyhammer/.abort", reason_with_newlines).unwrap();

    let result = executor
        .execute_state_with_limit(&mut run, 1000)
        .await
        .map(|_| run);

    assert!(matches!(result, Err(ExecutorError::Abort(_))));
    if let Err(ExecutorError::Abort(reason)) = result {
        assert_eq!(reason, reason_with_newlines);
    }
}

#[tokio::test]
#[serial_test::serial]
async fn test_abort_file_performance_impact() {
    use std::time::Instant;

    let test_env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let mut executor = WorkflowExecutor::new();

    // Create a simple fast workflow
    let mut workflow = Workflow::new(
        WorkflowName::new("Performance Test"),
        "Test abort performance impact".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state("end", "End state", true));
    workflow.add_transition(create_transition("start", "end", ConditionType::Always));

    // Time execution without abort file
    let start_without_abort = Instant::now();
    for _ in 0..100 {
        let mut run = executor.start_workflow(workflow.clone()).unwrap();
        let _ = executor.execute_state_with_limit(&mut run, 1000).await;
    }
    let duration_without_abort = start_without_abort.elapsed();

    // Create abort file in the isolated test environment
    let abort_file_path = test_env.swissarmyhammer_dir().join(".abort");
    std::fs::write(&abort_file_path, "Performance test abort").unwrap();

    // Time execution with abort file (will fail but we measure time to first check)
    let start_with_abort = Instant::now();
    for _ in 0..10 {
        // Recreate the abort file each time since it gets detected and errors
        std::fs::create_dir_all(".swissarmyhammer")
            .or_else(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    Ok(())
                } else {
                    Err(e)
                }
            })
            .unwrap();
        std::fs::write(&abort_file_path, "Performance test abort").unwrap();
        let mut run = executor.start_workflow(workflow.clone()).unwrap();
        let _ = executor.execute_state_with_limit(&mut run, 1000).await;
    }
    let duration_with_abort = start_with_abort.elapsed();

    // Abort checking should not significantly impact performance
    // Allow up to 10x overhead (very generous, should be much less)
    let max_acceptable_overhead = duration_without_abort * 10;
    assert!(
        duration_with_abort < max_acceptable_overhead,
        "Abort checking overhead too high: {duration_with_abort:?} vs {duration_without_abort:?}"
    );
}

#[tokio::test]
#[serial_test::serial]
async fn test_abort_file_detection_zero_transitions_limit() {
    let _test_env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let mut executor = WorkflowExecutor::new();

    let mut workflow = Workflow::new(
        WorkflowName::new("Zero Limit Test"),
        "Test abort with zero transition limit".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state("end", "End state", true));
    workflow.add_transition(create_transition("start", "end", ConditionType::Always));

    let mut run = executor.start_workflow(workflow).unwrap();

    std::fs::create_dir_all(".swissarmyhammer")
        .or_else(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(e)
            }
        })
        .unwrap();
    std::fs::write(".swissarmyhammer/.abort", "Zero limit abort").unwrap();

    // Execute with 1 transition limit - should check for abort before transitions
    let result = executor
        .execute_state_with_limit(&mut run, 1)
        .await
        .map(|_| run);

    // Should detect abort before hitting limit
    assert!(matches!(result, Err(ExecutorError::Abort(_))));
    if let Err(ExecutorError::Abort(reason)) = result {
        assert_eq!(reason, "Zero limit abort");
    }
}

#[tokio::test]
#[serial_test::serial]
async fn test_abort_file_not_present_normal_execution() {
    let _test_env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let mut executor = WorkflowExecutor::new();

    let mut workflow = Workflow::new(
        WorkflowName::new("Normal Execution Test"),
        "Test normal execution without abort file".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state("step1", "Step 1", false));
    workflow.add_state(create_state("end", "End state", true));

    workflow.add_transition(create_transition("start", "step1", ConditionType::Always));
    workflow.add_transition(create_transition("step1", "end", ConditionType::Always));

    let mut run = executor.start_workflow(workflow).unwrap();

    // Execute normally without abort file
    let result = executor.execute_state_with_limit(&mut run, 1000).await;

    // Should complete successfully
    assert!(result.is_ok());
    assert_eq!(run.current_state.as_str(), "end");
    assert_eq!(run.status, WorkflowRunStatus::Completed);
}
