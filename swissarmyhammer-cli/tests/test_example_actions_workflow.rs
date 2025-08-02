//! Tests for the example-actions builtin workflow
//!
//! This module tests the branching functionality in the example-actions workflow,
//! including conditional transitions, success/failure paths, and choice states.

use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use swissarmyhammer::workflow::{
    MermaidParser, StateId, WorkflowExecutor, WorkflowRun, WorkflowRunStatus,
};

/// Helper function to load the example-actions workflow
fn load_example_actions_workflow() -> Result<String> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let project_root = Path::new(&manifest_dir).parent().unwrap();
    let workflow_path = project_root.join("builtin/workflows/example-actions.md");

    let content = fs::read_to_string(workflow_path)?;
    Ok(content)
}

#[tokio::test]
async fn test_example_actions_workflow_loads() -> Result<()> {
    // Test that the workflow can be loaded and parsed
    let workflow_content = load_example_actions_workflow()?;
    let workflow = MermaidParser::parse(&workflow_content, "example-actions")?;

    assert_eq!(workflow.name.as_str(), "example-actions");

    // Verify that all the new states exist
    assert!(workflow.states.contains_key(&StateId::new("CheckValue")));
    assert!(workflow.states.contains_key(&StateId::new("SuccessPath")));
    assert!(workflow.states.contains_key(&StateId::new("FailurePath")));
    assert!(workflow.states.contains_key(&StateId::new("HandleError")));
    assert!(workflow
        .states
        .contains_key(&StateId::new("BranchDecision")));
    assert!(workflow.states.contains_key(&StateId::new("Branch1")));
    assert!(workflow.states.contains_key(&StateId::new("Branch2")));
    assert!(workflow.states.contains_key(&StateId::new("DefaultBranch")));

    Ok(())
}

#[tokio::test]
async fn test_success_branch_execution() -> Result<()> {
    // Test that the success branch is taken when actions succeed
    let workflow_content = load_example_actions_workflow()?;
    let workflow = MermaidParser::parse(&workflow_content, "example-actions")?;

    let mut executor = WorkflowExecutor::new();

    // Start workflow from the beginning to let it set up context properly
    let mut run = WorkflowRun::new(workflow);

    // Execute the workflow from the start
    let result = executor.execute_state(&mut run).await;

    // Verify that workflow completed successfully
    if let Err(e) = &result {
        eprintln!("Error executing workflow: {e:?}");
    }
    assert!(result.is_ok());

    // Check history for visited states - should follow success path
    let visited_states: Vec<StateId> = run
        .history
        .iter()
        .map(|(state_id, _)| state_id.clone())
        .collect();
    assert!(visited_states.contains(&StateId::new("SuccessPath")));

    // Verify context variables were set properly by the workflow
    assert!(run.context.contains_key("example_var"));
    assert_eq!(
        run.context.get("example_var"),
        Some(&json!("Hello from workflow"))
    );

    Ok(())
}

#[tokio::test]
async fn test_failure_branch_execution() -> Result<()> {
    // Test the failure branch by simulating a failure scenario
    let workflow_content = load_example_actions_workflow()?;
    let mut workflow = MermaidParser::parse(&workflow_content, "example-actions")?;

    // Modify the PromptExample state to use a non-existent prompt to force failure
    if let Some(state) = workflow.states.get_mut(&StateId::new("PromptExample")) {
        state.description =
            "Execute prompt \"non-existent-prompt\" with result=\"greeting\"".to_string();
    }

    let mut executor = WorkflowExecutor::new();

    // Run workflow from the start - this will create is_error=true due to Claude prompt failure
    let mut run = WorkflowRun::new(workflow);
    let result = executor.execute_state(&mut run).await;
    assert!(result.is_ok());

    // Debug: Print all context variables
    println!("Final context variables:");
    for (key, value) in &run.context {
        println!("  {key}: {value:?}");
    }

    // The workflow should have is_error=true from the failed Claude prompt
    assert!(run.context.contains_key("is_error"));
    assert_eq!(run.context.get("is_error"), Some(&json!(true)));

    // Change example_var so Branch1 condition is false
    run.context
        .insert("example_var".to_string(), json!("No Hello here"));

    // Navigate to BranchDecision to test the conditions
    run.current_state = StateId::new("BranchDecision");

    // Reset workflow status to Running so it can execute again
    run.status = WorkflowRunStatus::Running;

    // Clear history to test only the BranchDecision transition
    run.history.clear();

    // Debug: Print context before BranchDecision
    println!("Context before BranchDecision:");
    for (key, value) in &run.context {
        println!("  {key}: {value:?}");
    }
    println!("Current state: {}", run.current_state);

    // Debug: Check if BranchDecision is detected as a choice state
    if let Some(state) = run.workflow.states.get(&StateId::new("BranchDecision")) {
        println!("BranchDecision state type: {:?}", state.state_type);
    }

    // Debug: Check the actual transition conditions from BranchDecision
    let transitions: Vec<_> = run
        .workflow
        .transitions
        .iter()
        .filter(|t| t.from_state.as_str() == "BranchDecision")
        .collect();

    println!("Transitions from BranchDecision:");
    for transition in &transitions {
        println!("  -> {}: {:?}", transition.to_state, transition.condition);
        let condition_result = executor.evaluate_condition(&transition.condition, &run.context);
        println!("    Evaluates to: {condition_result:?}");
    }

    // Execute from BranchDecision - should go to Branch2 due to is_error=true
    let result = executor.execute_single_cycle(&mut run).await;
    println!("execute_single_cycle result: {result:?}");
    match &result {
        Ok(transition_performed) => println!("Transition performed: {transition_performed}"),
        Err(e) => println!("Error: {e}"),
    }
    assert!(result.is_ok());

    // Debug: Print where we ended up
    println!("After BranchDecision execution:");
    println!("Current state: {}", run.current_state);

    // Verify we went to Branch2
    let visited_states: Vec<StateId> = run
        .history
        .iter()
        .map(|(state_id, _)| state_id.clone())
        .collect();
    println!("Visited states: {visited_states:?}");
    assert!(visited_states.contains(&StateId::new("Branch2")));

    // Verify context variables were set properly
    assert!(run.context.contains_key("example_var"));
    assert!(run.context.contains_key("is_error"));
    assert_eq!(
        run.context.get("example_var"),
        Some(&json!("No Hello here"))
    );

    Ok(())
}

#[tokio::test]
async fn test_branch_decision_condition1() -> Result<()> {
    // Test that Branch1 is selected when example_var contains "Hello"
    let workflow_content = load_example_actions_workflow()?;
    let workflow = MermaidParser::parse(&workflow_content, "example-actions")?;

    let mut executor = WorkflowExecutor::new();
    let mut context = HashMap::new();

    // Set up context to trigger Branch1
    context.insert("example_var".to_string(), json!("Hello from workflow")); // Branch1 condition will be true
    context.insert("is_error".to_string(), json!(false)); // Branch2 condition will be false - this variable is required

    // Create a workflow run and manually set the starting state
    let mut run = WorkflowRun::new(workflow);
    run.current_state = StateId::new("BranchDecision");
    run.context = context;

    // Execute the workflow from the BranchDecision state
    let result = executor.execute_state(&mut run).await;

    // Verify Branch1 was visited
    assert!(result.is_ok());
    let visited_states: Vec<StateId> = run
        .history
        .iter()
        .map(|(state_id, _)| state_id.clone())
        .collect();
    assert!(visited_states.contains(&StateId::new("Branch1")));

    Ok(())
}

#[tokio::test]
async fn test_branch_decision_condition2() -> Result<()> {
    // Test that Branch2 is selected when failure is true
    let workflow_content = load_example_actions_workflow()?;
    let workflow = MermaidParser::parse(&workflow_content, "example-actions")?;

    let mut executor = WorkflowExecutor::new();

    // Run workflow normally to set up context variables properly
    let mut run = WorkflowRun::new(workflow);
    let result = executor.execute_state(&mut run).await;
    assert!(result.is_ok());

    // Set is_error=true and example_var to something that doesn't start with "Hello"
    run.context.insert("is_error".to_string(), json!(true)); // Branch2 condition
    run.context
        .insert("example_var".to_string(), json!("Some other value")); // Branch1 condition should be false

    // Navigate to BranchDecision state to test the CEL condition
    run.current_state = StateId::new("BranchDecision");

    // Reset workflow status to Running so it can execute again
    run.status = WorkflowRunStatus::Running;

    // Execute from BranchDecision - should go to Branch2 due to failure=true
    let result = executor.execute_single_cycle(&mut run).await;
    assert!(result.is_ok());

    // Verify Branch2 was visited
    let visited_states: Vec<StateId> = run
        .history
        .iter()
        .map(|(state_id, _)| state_id.clone())
        .collect();
    assert!(visited_states.contains(&StateId::new("Branch2")));

    Ok(())
}

#[tokio::test]
async fn test_branch_decision_default() -> Result<()> {
    // Test that DefaultBranch is selected when no conditions match
    let workflow_content = load_example_actions_workflow()?;
    let workflow = MermaidParser::parse(&workflow_content, "example-actions")?;

    let mut executor = WorkflowExecutor::new();
    let mut context = HashMap::new();

    // Set up context that doesn't match any conditions
    context.insert("example_var".to_string(), json!("No match")); // Branch1 condition will be false
    context.insert("is_error".to_string(), json!(false)); // Branch2 condition will be false

    // Create a workflow run and manually set the starting state
    let mut run = WorkflowRun::new(workflow);
    run.current_state = StateId::new("BranchDecision");
    run.context = context;

    // Execute the workflow from the BranchDecision state
    let result = executor.execute_state(&mut run).await;

    // Verify DefaultBranch was visited
    assert!(result.is_ok());
    let visited_states: Vec<StateId> = run
        .history
        .iter()
        .map(|(state_id, _)| state_id.clone())
        .collect();
    assert!(visited_states.contains(&StateId::new("DefaultBranch")));

    Ok(())
}

#[tokio::test]
async fn test_full_workflow_with_branching() -> Result<()> {
    // Test the complete workflow execution with branching
    let workflow_content = load_example_actions_workflow()?;
    let workflow = MermaidParser::parse(&workflow_content, "example-actions")?;

    let mut executor = WorkflowExecutor::new();

    // Create a mock prompt for say-hello
    let mut context = HashMap::new();
    context.insert("greeting".to_string(), json!("Hello, World!"));

    // Create a workflow run
    let mut run = WorkflowRun::new(workflow);
    run.context = context;

    // Execute the full workflow
    let result = executor.execute_state(&mut run).await;

    // Verify the workflow completed successfully
    assert!(result.is_ok());
    assert!(matches!(run.status, WorkflowRunStatus::Completed));

    // Verify that all expected variables were set
    assert!(run.context.contains_key("example_var"));
    assert!(run.context.contains_key("greeting"));

    // Verify that at least one branch was taken
    let visited_states: Vec<StateId> = run
        .history
        .iter()
        .map(|(state_id, _)| state_id.clone())
        .collect();
    let branch1_taken = visited_states.contains(&StateId::new("Branch1"));
    let branch2_taken = visited_states.contains(&StateId::new("Branch2"));
    let default_taken = visited_states.contains(&StateId::new("DefaultBranch"));

    assert!(
        branch1_taken || branch2_taken || default_taken,
        "At least one branch should have been taken"
    );

    Ok(())
}

#[tokio::test]
async fn test_all_branches_are_reachable() -> Result<()> {
    // Test that all branches in the workflow can be reached with different contexts
    let workflow_content = load_example_actions_workflow()?;
    let workflow = MermaidParser::parse(&workflow_content, "example-actions")?;

    let mut executor = WorkflowExecutor::new();

    // Test scenario 1: Branch1 (example_var contains "Hello")
    {
        let mut run = WorkflowRun::new(workflow.clone());

        // Set up context directly without running the full workflow
        run.context
            .insert("example_var".to_string(), json!("Hello from workflow"));
        run.context.insert("is_error".to_string(), json!(false));
        run.current_state = StateId::new("BranchDecision");

        let result = executor.execute_single_cycle(&mut run).await;
        assert!(result.is_ok());

        let visited_states: Vec<StateId> = run
            .history
            .iter()
            .map(|(state_id, _)| state_id.clone())
            .collect();
        assert!(
            visited_states.contains(&StateId::new("Branch1")),
            "Expected Branch1 to be visited"
        );
    }

    // Test scenario 2: Branch2 (is_error == true)
    {
        let mut run = WorkflowRun::new(workflow.clone());

        // Set up context directly without running the full workflow
        run.context.insert("is_error".to_string(), json!(true));
        run.context
            .insert("example_var".to_string(), json!("No Hello here"));
        run.current_state = StateId::new("BranchDecision");

        let result = executor.execute_single_cycle(&mut run).await;
        assert!(result.is_ok());

        let visited_states: Vec<StateId> = run
            .history
            .iter()
            .map(|(state_id, _)| state_id.clone())
            .collect();
        assert!(
            visited_states.contains(&StateId::new("Branch2")),
            "Expected Branch2 to be visited"
        );
    }

    // Test scenario 3: DefaultBranch (no conditions match)
    {
        let mut run = WorkflowRun::new(workflow.clone());

        // Set up context directly without running the full workflow
        run.context.insert("is_error".to_string(), json!(false));
        run.context
            .insert("example_var".to_string(), json!("No Hello here"));
        run.current_state = StateId::new("BranchDecision");

        let result = executor.execute_single_cycle(&mut run).await;
        assert!(result.is_ok());

        let visited_states: Vec<StateId> = run
            .history
            .iter()
            .map(|(state_id, _)| state_id.clone())
            .collect();
        assert!(
            visited_states.contains(&StateId::new("DefaultBranch")),
            "Expected DefaultBranch to be visited"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_debug_cel_expressions() -> Result<()> {
    // Debug test to understand CEL expression evaluation
    let workflow_content = load_example_actions_workflow()?;
    let workflow = MermaidParser::parse(&workflow_content, "example-actions")?;

    let mut executor = WorkflowExecutor::new();

    // Run workflow to set up context
    let mut run = WorkflowRun::new(workflow);
    let result = executor.execute_state(&mut run).await;
    assert!(result.is_ok());

    // Debug: Print current context
    println!("Context after workflow execution:");
    for (key, value) in &run.context {
        println!("  {key}: {value:?}");
    }

    // Test different values for error_handled
    let test_values = vec![
        ("string_true", json!("true")),
        ("bool_true", json!(true)),
        ("string_false", json!("false")),
        ("bool_false", json!(false)),
    ];

    for (label, value) in test_values {
        run.context
            .insert("error_handled".to_string(), value.clone());
        run.context
            .insert("example_var".to_string(), json!("No Hello"));
        run.current_state = StateId::new("BranchDecision");

        println!("\nTesting {label} with error_handled = {value:?}");

        // Get the transitions from BranchDecision
        let transitions: Vec<_> = run
            .workflow
            .transitions
            .iter()
            .filter(|t| t.from_state.as_str() == "BranchDecision")
            .collect();

        for transition in transitions {
            let condition_result = executor.evaluate_condition(&transition.condition, &run.context);
            println!(
                "  Transition to {}: condition = {:?}, result = {:?}",
                transition.to_state, transition.condition, condition_result
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_branch1_liquid_template_rendering() -> Result<()> {
    // Test that the Branch1 log message properly renders {{branch_value}} with liquid templating
    use swissarmyhammer::workflow::{Action, LogAction, LogLevel};

    // Create the exact log action from Branch1 in example-actions.md
    let log_action = LogAction::new(
        "Branch 1 selected: {{branch_value}} contains Hello".to_string(),
        LogLevel::Info,
    );

    // Create context that simulates what the BranchDecision state would have set
    let mut context = HashMap::new();
    context.insert("branch_value".to_string(), json!("Hello from workflow"));
    context.insert("example_var".to_string(), json!("Hello from workflow"));
    context.insert("is_error".to_string(), json!(false));

    // Execute the log action
    let result = log_action.execute(&mut context).await?;

    // Verify the liquid template was rendered correctly
    let expected_message = "Branch 1 selected: Hello from workflow contains Hello";
    assert_eq!(result.as_str().unwrap(), expected_message);

    println!("✅ Liquid template rendering test passed!");
    println!("Original: Branch 1 selected: {{{{branch_value}}}} contains Hello");
    println!("Rendered: {}", result.as_str().unwrap());

    Ok(())
}
