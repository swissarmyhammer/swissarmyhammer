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
    assert!(workflow.states.contains_key(&StateId::new("start")));
    assert!(workflow.states.contains_key(&StateId::new("greet")));
    assert!(workflow.states.contains_key(&StateId::new("farewell")));

    Ok(())
}

#[tokio::test]
async fn test_hello_world_all_actions_parse() -> Result<()> {
    // This test verifies that all actions in the hello-world workflow
    // can be successfully parsed without warnings

    let workflow_content = load_hello_world_workflow()?;
    let workflow = MermaidParser::parse(&workflow_content, "hello-world")?;

    // Check each state's action description can be parsed
    let start_state = workflow.states.get(&StateId::new("start")).unwrap();
    let greet_state = workflow.states.get(&StateId::new("greet")).unwrap();
    let farewell_state = workflow.states.get(&StateId::new("farewell")).unwrap();

    println!("Start action: {}", start_state.description);
    println!("Greet action: {}", greet_state.description);
    println!("Farewell action: {}", farewell_state.description);

    // Try to parse each action
    use std::collections::HashMap;
    use swissarmyhammer_workflow::parse_action_from_description_with_context;

    // Create context with default parameter values for template rendering
    let mut context = HashMap::new();
    context.insert("person_name".to_string(), serde_json::json!("World"));
    context.insert("language".to_string(), serde_json::json!("English"));
    context.insert("enthusiastic".to_string(), serde_json::json!(false));

    // Start action should parse successfully
    let start_action =
        parse_action_from_description_with_context(&start_state.description, &context)?;
    assert!(
        start_action.is_some(),
        "Start action should parse successfully: '{}'",
        start_state.description
    );

    // Greet action should parse successfully (after template rendering)
    let greet_action =
        parse_action_from_description_with_context(&greet_state.description, &context)?;
    assert!(
        greet_action.is_some(),
        "Greet action should parse successfully: '{}'",
        greet_state.description
    );

    // Farewell action should parse successfully
    // Note: This may contain variable interpolation, but the action keyword should still be recognized
    let farewell_action =
        parse_action_from_description_with_context(&farewell_state.description, &context)?;
    assert!(
        farewell_action.is_some(),
        "Farewell action should parse successfully: '{}'",
        farewell_state.description
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

    // Set person_name in context to test variable interpolation
    run.context.insert(
        "person_name".to_string(),
        serde_json::json!("World"),
    );

    // Start from the farewell state to test the log action with variable interpolation
    run.current_state = StateId::new("farewell");

    // Execute the farewell state
    let result = executor.execute_single_cycle(&mut run).await;

    // The execution should succeed without parse errors
    assert!(
        result.is_ok(),
        "Farewell state should execute without errors"
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
        "Goodbye, {{ person_name }}!".to_string(),
        LogLevel::Info,
    );

    // Create context with the person_name
    let workflow_vars = HashMap::from([(
        "person_name".to_string(),
        serde_json::json!("Swiss Army Hammer"),
    )]);

    let mut context = WorkflowTemplateContext::with_vars(HashMap::new()).unwrap();
    context.set_workflow_vars(workflow_vars);

    // Execute the log action
    let result = log_action.execute(&mut context).await?;

    // Verify the result
    println!("Log action result: {}", result.as_str().unwrap());

    Ok(())
}
