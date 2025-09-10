//! Test helper functions for workflow module
//!
//! This module provides common test utilities to reduce code duplication
//! across workflow tests.

use crate::{
    ConditionType, State, StateId, StateType, Transition, TransitionCondition, Workflow,
    WorkflowName,
};

/// Test helper to create a basic state
pub fn create_state(id: &str, description: &str, is_terminal: bool) -> State {
    State {
        id: StateId::new(id),
        description: description.to_string(),
        state_type: StateType::Normal,
        is_terminal,
        allows_parallel: false,
        metadata: Default::default(),
    }
}

/// Test helper to create a state with custom parallel setting and state type
#[allow(dead_code)]
pub fn create_state_with_parallel(
    id: &str,
    description: &str,
    is_terminal: bool,
    allows_parallel: bool,
) -> State {
    State {
        id: StateId::new(id),
        description: description.to_string(),
        state_type: StateType::Normal,
        is_terminal,
        allows_parallel,
        metadata: Default::default(),
    }
}

/// Test helper to create a state with specific type
#[allow(dead_code)]
pub fn create_state_with_type(
    id: &str,
    description: &str,
    state_type: StateType,
    is_terminal: bool,
) -> State {
    State {
        id: StateId::new(id),
        description: description.to_string(),
        state_type,
        is_terminal,
        allows_parallel: matches!(state_type, StateType::Fork | StateType::Join),
        metadata: Default::default(),
    }
}

/// Test helper to create a basic transition
pub fn create_transition(from: &str, to: &str, condition_type: ConditionType) -> Transition {
    Transition {
        from_state: StateId::new(from),
        to_state: StateId::new(to),
        condition: TransitionCondition {
            condition_type,
            expression: None,
        },
        action: None,
        metadata: Default::default(),
    }
}

/// Test helper to create a transition with custom action
#[allow(dead_code)]
pub fn create_transition_with_action(
    from: &str,
    to: &str,
    condition_type: ConditionType,
    action: Option<String>,
) -> Transition {
    Transition {
        from_state: StateId::new(from),
        to_state: StateId::new(to),
        condition: TransitionCondition {
            condition_type,
            expression: None,
        },
        action,
        metadata: Default::default(),
    }
}

/// Test helper to create a basic workflow with start and end states
pub fn create_basic_workflow() -> Workflow {
    let mut workflow = Workflow::new(
        WorkflowName::new("Test Workflow"),
        "A test workflow".to_string(),
        StateId::new("start"),
    );

    workflow.add_state(create_state("start", "Start state", false));
    workflow.add_state(create_state("end", "End state", true));
    workflow.add_transition(create_transition("start", "end", ConditionType::Always));

    workflow
}

/// Test helper to create a workflow with custom name and description
pub fn create_workflow(name: &str, description: &str, initial_state: &str) -> Workflow {
    Workflow::new(
        WorkflowName::new(name),
        description.to_string(),
        StateId::new(initial_state),
    )
}

/// Test helper to create a minimal valid workflow for testing
#[allow(dead_code)]
pub fn create_minimal_workflow() -> Workflow {
    let mut workflow = create_workflow("Minimal Test", "Minimal workflow for testing", "start");
    workflow.add_state(create_state("start", "Start", true));
    workflow
}
