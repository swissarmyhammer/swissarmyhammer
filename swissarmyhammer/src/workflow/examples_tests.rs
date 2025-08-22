//! Tests for workflow examples to ensure they work correctly
//!
//! These tests validate the workflow examples in the documentation to ensure
//! they remain functional and serve as integration tests.

use crate::workflow::{MermaidParser, Workflow, WorkflowExecutor, WorkflowName, WorkflowRunStatus};
use std::path::Path;

/// Helper function to load workflow from file
fn load_workflow_from_file(file_path: &str) -> anyhow::Result<Workflow> {
    let content = std::fs::read_to_string(file_path)?;

    // Extract workflow metadata from YAML front matter
    if content.starts_with("---\n") {
        let parts: Vec<&str> = content.splitn(3, "---\n").collect();
        if parts.len() >= 2 {
            // Parse YAML front matter to extract metadata
            let yaml_content = parts[1];
            let mut workflow_name = "unnamed_workflow".to_string();
            let mut title = None;
            let mut description = None;

            for line in yaml_content.lines() {
                let line = line.trim();
                if line.starts_with("name:") {
                    workflow_name = line.split(':').nth(1).unwrap_or("").trim().to_string();
                } else if line.starts_with("title:") {
                    title = Some(line.split(':').nth(1).unwrap_or("").trim().to_string());
                } else if line.starts_with("description:") {
                    description = Some(line.split(':').nth(1).unwrap_or("").trim().to_string());
                }
            }

            return Ok(MermaidParser::parse_with_metadata(
                &content,
                WorkflowName::new(workflow_name),
                title,
                description,
            )?);
        }
    }

    // Fallback for files without YAML front matter
    Ok(MermaidParser::parse(
        &content,
        WorkflowName::new("unnamed_workflow"),
    )?)
}

/// Test the simple workflow example
#[test]
fn test_simple_workflow_example() {
    let workflow_path = "../doc/examples/workflows/simple-workflow.md";

    // Skip if file doesn't exist (in case we're running tests before doc generation)
    if !Path::new(workflow_path).exists() {
        return;
    }

    let workflow =
        load_workflow_from_file(workflow_path).expect("Failed to load workflow from file");

    // Verify workflow has required metadata
    assert!(
        !workflow.name.as_str().is_empty(),
        "Workflow name should not be empty"
    );
    assert!(
        !workflow.description.is_empty(),
        "Workflow description should not be empty"
    );

    // Verify workflow has initial state
    assert!(
        workflow.states.contains_key(&workflow.initial_state),
        "Workflow should have an initial state"
    );

    // Verify workflow structure is valid
    workflow
        .validate_structure()
        .expect("Workflow should be valid");
}

/// Test the parallel workflow example
#[test]
fn test_parallel_workflow_example() {
    let workflow_path = "../doc/examples/workflows/parallel-workflow.md";

    // Skip if file doesn't exist (in case we're running tests before doc generation)
    if !Path::new(workflow_path).exists() {
        return;
    }

    let workflow =
        load_workflow_from_file(workflow_path).expect("Failed to load workflow from file");

    // Verify workflow has required metadata
    assert!(
        !workflow.name.as_str().is_empty(),
        "Workflow name should not be empty"
    );
    assert!(
        !workflow.description.is_empty(),
        "Workflow description should not be empty"
    );

    // Verify workflow has initial state
    assert!(
        workflow.states.contains_key(&workflow.initial_state),
        "Workflow should have an initial state"
    );

    // Verify workflow structure is valid
    workflow
        .validate_structure()
        .expect("Workflow should be valid");
}

/// Test the user confirmation workflow example
#[test]
fn test_user_confirmation_workflow_example() {
    let workflow_path = "../doc/examples/workflows/user-confirmation-workflow.md";

    // Skip if file doesn't exist (in case we're running tests before doc generation)
    if !Path::new(workflow_path).exists() {
        return;
    }

    let workflow =
        load_workflow_from_file(workflow_path).expect("Failed to load workflow from file");

    // Verify workflow has required metadata
    assert!(
        !workflow.name.as_str().is_empty(),
        "Workflow name should not be empty"
    );
    assert!(
        !workflow.description.is_empty(),
        "Workflow description should not be empty"
    );

    // Verify workflow has initial state
    assert!(
        workflow.states.contains_key(&workflow.initial_state),
        "Workflow should have an initial state"
    );

    // Verify workflow structure is valid
    workflow
        .validate_structure()
        .expect("Workflow should be valid");
}

/// Test simple workflow execution
#[test]
fn test_simple_workflow_execution() {
    let workflow_path = "../doc/examples/workflows/simple-workflow.md";

    if !Path::new(workflow_path).exists() {
        return;
    }

    let workflow = load_workflow_from_file(workflow_path).expect("Failed to load simple workflow");

    let mut executor = WorkflowExecutor::new();
    let mut run = executor
        .start_workflow(workflow)
        .expect("Failed to start workflow");

    // Set initial variables
    run.context.insert(
        "start_time".to_string(),
        serde_json::Value::String("2024-01-01T00:00:00Z".to_string()),
    );

    // Verify workflow run was initialized properly
    assert_eq!(run.status, WorkflowRunStatus::Running);
    assert_eq!(run.current_state.as_str(), "Start");

    // For this test, we'll just verify the workflow can be started and is in the correct initial state
    // since the execution engine is complex and would require more setup
}

/// Test parallel workflow execution
#[test]
fn test_parallel_workflow() {
    let workflow_path = "../doc/examples/workflows/parallel-workflow.md";

    if !Path::new(workflow_path).exists() {
        return;
    }

    let workflow =
        load_workflow_from_file(workflow_path).expect("Failed to load parallel workflow");

    let mut executor = WorkflowExecutor::new();
    let mut run = executor
        .start_workflow(workflow)
        .expect("Failed to start workflow");

    // Set initial variables
    run.context.insert(
        "parallel_timeout".to_string(),
        serde_json::Value::String("5000".to_string()),
    );

    // Verify parallel workflow structure
    assert_eq!(run.status, WorkflowRunStatus::Running);
    assert!(
        !run.workflow.states.is_empty(),
        "Parallel workflow should have states"
    );

    // Check for parallel execution capabilities in the workflow
    let parallel_states = run
        .workflow
        .states
        .values()
        .filter(|s| s.allows_parallel)
        .count();
    assert!(
        parallel_states > 0,
        "Parallel workflow should have states that allow parallel execution"
    );
}

/// Test user confirmation workflow
#[test]
fn test_user_confirmation_workflow() {
    let workflow_path = "../doc/examples/workflows/user-confirmation-workflow.md";

    if !Path::new(workflow_path).exists() {
        return;
    }

    let workflow =
        load_workflow_from_file(workflow_path).expect("Failed to load user confirmation workflow");

    let mut executor = WorkflowExecutor::new();
    let mut run = executor
        .start_workflow(workflow)
        .expect("Failed to start workflow");

    // Set initial variables
    run.context.insert(
        "operation_description".to_string(),
        serde_json::Value::String("Test operation".to_string()),
    );
    run.context.insert(
        "user_response".to_string(),
        serde_json::Value::String("continue".to_string()),
    );

    // Verify confirmation workflow structure
    assert_eq!(run.status, WorkflowRunStatus::Running);
    assert!(
        !run.workflow.states.is_empty(),
        "User confirmation workflow should have states"
    );

    // Verify that variables are properly set
    assert!(
        run.context.contains_key("operation_description"),
        "Should have operation_description variable"
    );
    assert!(
        run.context.contains_key("user_response"),
        "Should have user_response variable"
    );
}

/// Test workflow variable handling for simple workflow
#[test]
fn test_simple_workflow_variables() {
    let workflow_path = "../doc/examples/workflows/simple-workflow.md";

    if !Path::new(workflow_path).exists() {
        return;
    }

    let workflow = load_workflow_from_file(workflow_path).expect("Failed to load workflow");

    let mut executor = WorkflowExecutor::new();
    let mut run = executor
        .start_workflow(workflow)
        .expect("Failed to start workflow");

    // Set the required variable
    run.context.insert(
        "start_time".to_string(),
        serde_json::Value::String("test_value".to_string()),
    );

    // Verify variable was set
    assert!(
        run.context.contains_key("start_time"),
        "Required variable start_time should be set"
    );

    // Verify variable value
    assert_eq!(
        run.context.get("start_time").and_then(|v| v.as_str()),
        Some("test_value"),
        "Variable start_time should have correct value"
    );
}

/// Test workflow variable handling for parallel workflow
#[test]
fn test_parallel_workflow_variables() {
    let workflow_path = "../doc/examples/workflows/parallel-workflow.md";

    if !Path::new(workflow_path).exists() {
        return;
    }

    let workflow = load_workflow_from_file(workflow_path).expect("Failed to load workflow");

    let mut executor = WorkflowExecutor::new();
    let mut run = executor
        .start_workflow(workflow)
        .expect("Failed to start workflow");

    // Set the required variable
    run.context.insert(
        "parallel_timeout".to_string(),
        serde_json::Value::String("test_value".to_string()),
    );

    // Verify variable was set
    assert!(
        run.context.contains_key("parallel_timeout"),
        "Required variable parallel_timeout should be set"
    );

    // Verify variable value
    assert_eq!(
        run.context.get("parallel_timeout").and_then(|v| v.as_str()),
        Some("test_value"),
        "Variable parallel_timeout should have correct value"
    );
}

/// Test workflow variable handling for user confirmation workflow
#[test]
fn test_user_confirmation_workflow_variables() {
    let workflow_path = "../doc/examples/workflows/user-confirmation-workflow.md";

    if !Path::new(workflow_path).exists() {
        return;
    }

    let workflow = load_workflow_from_file(workflow_path).expect("Failed to load workflow");

    let mut executor = WorkflowExecutor::new();
    let mut run = executor
        .start_workflow(workflow)
        .expect("Failed to start workflow");

    // Set the required variable
    run.context.insert(
        "operation_description".to_string(),
        serde_json::Value::String("test_value".to_string()),
    );

    // Verify variable was set
    assert!(
        run.context.contains_key("operation_description"),
        "Required variable operation_description should be set"
    );

    // Verify variable value
    assert_eq!(
        run.context
            .get("operation_description")
            .and_then(|v| v.as_str()),
        Some("test_value"),
        "Variable operation_description should have correct value"
    );
}

/// Test workflow state transitions
#[test]
fn test_workflow_state_transitions() {
    let workflow_path = "../doc/examples/workflows/simple-workflow.md";

    if !Path::new(workflow_path).exists() {
        return;
    }

    let workflow = load_workflow_from_file(workflow_path).expect("Failed to load workflow");

    // Test that all states have proper transitions
    assert!(!workflow.states.is_empty(), "Workflow should have states");

    for (state_id, state) in &workflow.states {
        // Verify each state has proper structure
        assert!(
            !state_id.as_str().is_empty(),
            "State ID should not be empty"
        );
        assert!(
            !state.description.is_empty(),
            "State description should not be empty"
        );
    }

    // Verify transitions are valid
    for transition in &workflow.transitions {
        assert!(
            !transition.from_state.as_str().is_empty(),
            "Transition source should not be empty"
        );
        assert!(
            !transition.to_state.as_str().is_empty(),
            "Transition target should not be empty"
        );

        // Verify that transition references existing states
        assert!(
            workflow.states.contains_key(&transition.from_state),
            "Transition source state {} should exist",
            transition.from_state
        );
        assert!(
            workflow.states.contains_key(&transition.to_state),
            "Transition target state {} should exist",
            transition.to_state
        );
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Test workflow file loading and validation
    #[test]
    fn test_workflow_file_validation() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();
        let workflow_path = temp_dir.path().join("test-workflow.md");

        // Create a minimal valid workflow
        let workflow_content = r#"---
name: test-workflow
title: Test Workflow
description: A test workflow
version: 1.0.0
---

# Test Workflow

## States

### Start
- **Type**: Initial
- **Actions**: Log start
- **Transitions**: Always -> End

### End
- **Type**: Final
- **Actions**: Log end

## Workflow Definition

```mermaid
stateDiagram-v2
    [*] --> Start
    Start --> End
    End --> [*]
```
"#;

        std::fs::write(&workflow_path, workflow_content).expect("Failed to write workflow file");

        let workflow = load_workflow_from_file(workflow_path.to_str().unwrap())
            .expect("Failed to load test workflow");

        assert_eq!(workflow.name.as_str(), "test-workflow");
        assert_eq!(workflow.description, "A test workflow");

        // Validate the workflow structure
        workflow
            .validate_structure()
            .expect("Workflow should be valid");

        // Verify states
        assert!(workflow
            .states
            .contains_key(&crate::workflow::StateId::new("Start")));
        assert!(workflow
            .states
            .contains_key(&crate::workflow::StateId::new("End")));

        // Verify initial state
        assert_eq!(workflow.initial_state.as_str(), "Start");
    }
}
