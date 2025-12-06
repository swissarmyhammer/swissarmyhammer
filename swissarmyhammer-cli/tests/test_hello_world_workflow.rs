//! Tests for the hello-world builtin workflow
//!
//! This module ensures that the hello-world workflow correctly parses and executes
//! all actions without any "No action could be parsed" warnings.

use anyhow::Result;
use std::fs;
use std::path::Path;
use swissarmyhammer::WorkflowExecutor;
use swissarmyhammer_workflow::{MermaidParser, StateId, WorkflowRun};

/// Helper function to load the hello-world workflow
fn load_hello_world_workflow() -> Result<String> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let project_root = Path::new(&manifest_dir).parent().unwrap();
    let workflow_path = project_root.join("builtin/workflows/hello-world.md");

    let content = fs::read_to_string(workflow_path)?;
    Ok(content)
}

#[tokio::test]
async fn test_hello_world_workflow_loads() -> Result<()> {
    // Test that the workflow can be loaded and parsed
    let workflow_content = load_hello_world_workflow()?;
    let workflow = MermaidParser::parse(&workflow_content, "hello-world")?;

    assert_eq!(workflow.name.as_str(), "hello-world");

    // Verify that all states exist
    assert!(workflow.states.contains_key(&StateId::new("Start")));
    assert!(workflow.states.contains_key(&StateId::new("Greeting")));
    assert!(workflow.states.contains_key(&StateId::new("Complete")));

    Ok(())
}

#[tokio::test]
async fn test_hello_world_all_actions_parse() -> Result<()> {
    // This test verifies that all actions in the hello-world workflow
    // can be successfully parsed without warnings

    let workflow_content = load_hello_world_workflow()?;
    let workflow = MermaidParser::parse(&workflow_content, "hello-world")?;

    // Check each state's action description can be parsed
    let start_state = workflow.states.get(&StateId::new("Start")).unwrap();
    let greeting_state = workflow.states.get(&StateId::new("Greeting")).unwrap();
    let complete_state = workflow.states.get(&StateId::new("Complete")).unwrap();

    println!("Start action: {}", start_state.description);
    println!("Greeting action: {}", greeting_state.description);
    println!("Complete action: {}", complete_state.description);

    // Try to parse each action
    use std::collections::HashMap;
    use swissarmyhammer_workflow::parse_action_from_description_with_context;

    let context = HashMap::new();

    // Start action should parse successfully
    let start_action =
        parse_action_from_description_with_context(&start_state.description, &context)?;
    assert!(
        start_action.is_some(),
        "Start action should parse successfully: '{}'",
        start_state.description
    );

    // Greeting action should parse successfully
    let greeting_action =
        parse_action_from_description_with_context(&greeting_state.description, &context)?;
    assert!(
        greeting_action.is_some(),
        "Greeting action should parse successfully: '{}'",
        greeting_state.description
    );

    // Complete action should parse successfully
    // Note: This may contain variable interpolation, but the action keyword should still be recognized
    let complete_action =
        parse_action_from_description_with_context(&complete_state.description, &context)?;
    assert!(
        complete_action.is_some(),
        "Complete action should parse successfully: '{}'",
        complete_state.description
    );

    Ok(())
}

#[tokio::test]
async fn test_hello_world_execution_without_claude() -> Result<()> {
    // Test the hello-world workflow execution by mocking the greeting result
    let workflow_content = load_hello_world_workflow()?;
    let workflow = MermaidParser::parse(&workflow_content, "hello-world")?;

    let mut executor = WorkflowExecutor::new();
    let mut run = WorkflowRun::new(workflow);

    // Mock the greeting result to avoid calling Claude
    run.context.insert(
        "greeting_output".to_string(),
        serde_json::json!({
            "content": "Hello, World! This is a test greeting.",
            "metadata": null,
            "response_type": "Success"
        }),
    );

    // Start from the Complete state to test the log action with variable interpolation
    run.current_state = StateId::new("Complete");

    // Execute the Complete state
    let result = executor.execute_single_cycle(&mut run).await;

    // The execution should succeed without parse errors
    assert!(
        result.is_ok(),
        "Complete state should execute without errors"
    );

    Ok(())
}

#[tokio::test]
async fn test_hello_world_log_with_interpolation() -> Result<()> {
    // Test that log actions with variable interpolation work correctly
    use std::collections::HashMap;
    use swissarmyhammer_workflow::{Action, LogAction, LogLevel, WorkflowTemplateContext};

    // Create a log action with variable interpolation
    let log_action = LogAction::new(
        "Workflow completed! Greeting result: ${greeting_output.content}".to_string(),
        LogLevel::Info,
    );

    // Create context with the greeting result
    let workflow_vars = HashMap::from([(
        "greeting_output".to_string(),
        serde_json::json!({
            "content": "Hello from Swiss Army Hammer!",
            "metadata": null,
            "response_type": "Success"
        }),
    )]);

    let mut context = WorkflowTemplateContext::with_vars(HashMap::new()).unwrap();
    context.set_workflow_vars(workflow_vars);

    // Execute the log action
    let result = log_action.execute(&mut context).await?;

    // Verify the result
    println!("Log action result: {}", result.as_str().unwrap());

    Ok(())
}
