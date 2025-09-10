//! Simple test to demonstrate state name pollution in nested workflows

use crate::{
    ConditionType, State, StateId, StateType, Transition, TransitionCondition, Workflow,
    WorkflowExecutor, WorkflowName, WorkflowRun, WorkflowStorage,
};
use serde_json::Value;
use std::collections::HashMap;

#[tokio::test]
async fn test_state_pollution_manual() {
    // Create workflows manually to have full control
    let mut parent_workflow = Workflow::new(
        WorkflowName::new("parent"),
        "Parent workflow".to_string(),
        StateId::new("start"),
    );

    // Add states to parent workflow
    parent_workflow.states.insert(
        StateId::new("start"),
        State {
            id: StateId::new("start"),
            description: "Set parent_var=\"parent_start\"".to_string(),
            is_terminal: false,
            state_type: StateType::Normal,
            allows_parallel: false,
            metadata: HashMap::new(),
        },
    );
    parent_workflow.states.insert(
        StateId::new("process"),
        State {
            id: StateId::new("process"),
            description: "Run workflow \"child\" with result=\"child_result\"".to_string(),
            is_terminal: false,
            state_type: StateType::Normal,
            allows_parallel: false,
            metadata: HashMap::new(),
        },
    );
    parent_workflow.states.insert(
        StateId::new("end"),
        State {
            id: StateId::new("end"),
            description: "Log \"Parent end state\"".to_string(),
            is_terminal: true,
            state_type: StateType::Normal,
            allows_parallel: false,
            metadata: HashMap::new(),
        },
    );

    // Add transitions
    parent_workflow.transitions.push(Transition {
        from_state: StateId::new("start"),
        to_state: StateId::new("process"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });
    parent_workflow.transitions.push(Transition {
        from_state: StateId::new("process"),
        to_state: StateId::new("end"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    // Create child workflow with conflicting state names
    let mut child_workflow = Workflow::new(
        WorkflowName::new("child"),
        "Child workflow".to_string(),
        StateId::new("start"),
    );

    // Add states to child workflow - same names as parent!
    child_workflow.states.insert(
        StateId::new("start"),
        State {
            id: StateId::new("start"),
            description: "Set child_var=\"child_start\"".to_string(),
            is_terminal: false,
            state_type: StateType::Normal,
            allows_parallel: false,
            metadata: HashMap::new(),
        },
    );
    child_workflow.states.insert(
        StateId::new("process"),
        State {
            id: StateId::new("process"),
            description: "Set child_var=\"child_process\"".to_string(),
            is_terminal: false,
            state_type: StateType::Normal,
            allows_parallel: false,
            metadata: HashMap::new(),
        },
    );
    child_workflow.states.insert(
        StateId::new("end"),
        State {
            id: StateId::new("end"),
            description: "Log \"Child end state\"".to_string(),
            is_terminal: true,
            state_type: StateType::Normal,
            allows_parallel: false,
            metadata: HashMap::new(),
        },
    );

    // Add transitions
    child_workflow.transitions.push(Transition {
        from_state: StateId::new("start"),
        to_state: StateId::new("process"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });
    child_workflow.transitions.push(Transition {
        from_state: StateId::new("process"),
        to_state: StateId::new("end"),
        condition: TransitionCondition {
            condition_type: ConditionType::Always,
            expression: None,
        },
        action: None,
        metadata: HashMap::new(),
    });

    // Store workflows
    let mut storage = WorkflowStorage::memory();
    storage.store_workflow(parent_workflow.clone()).unwrap();
    storage.store_workflow(child_workflow).unwrap();

    // Create executor
    let mut executor = WorkflowExecutor::new();

    // Start workflow and verify state isolation
    let mut run = WorkflowRun::new(parent_workflow);

    // Execute states one by one to track execution
    eprintln!("Starting parent workflow execution");

    // Execute first state (parent start)
    let result = executor.execute_single_state(&mut run).await;
    assert!(result.is_ok());
    eprintln!("After parent start - context: {:?}", run.context);
    assert_eq!(
        run.context.get("parent_var"),
        Some(&Value::String("parent_start".to_string()))
    );

    // Transition to next state
    executor
        .perform_transition(&mut run, StateId::new("process"))
        .unwrap();

    // Execute process state (should run child workflow)
    eprintln!("Executing parent process state (sub-workflow call)");
    let result = executor.execute_single_state(&mut run).await;
    eprintln!("Sub-workflow execution result: {result:?}");
    eprintln!("After sub-workflow - context: {:?}", run.context);

    // The test passes if:
    // 1. Parent variables are not overwritten by child
    // 2. Child workflow executes its own states correctly
    // 3. No state confusion occurs between parent and child

    // Verify parent state wasn't affected
    assert_eq!(
        run.context.get("parent_var"),
        Some(&Value::String("parent_start".to_string())),
        "Parent variable should not be overwritten by child workflow"
    );
}
