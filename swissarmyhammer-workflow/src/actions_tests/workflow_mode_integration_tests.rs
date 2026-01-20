//! Integration tests for workflow mode flow
//!
//! These tests verify that the workflow mode correctly flows from:
//! 1. Workflow definition (frontmatter)
//! 2. Through WorkflowRun creation
//! 3. Into WorkflowTemplateContext
//! 4. Available for action execution

use crate::parser::MermaidParser;
use crate::run::WorkflowRun;
use crate::test_helpers::*;
use crate::WorkflowTemplateContext;
use std::collections::HashMap;

/// Test that mode flows from workflow frontmatter through to context
#[test]
fn test_mode_flows_from_frontmatter_to_context() {
    let input = r#"---
title: Test Workflow with Mode
description: Tests mode flow
mode: planner
---

```mermaid
stateDiagram-v2
    [*] --> Process
    Process --> [*]
```
"#;

    // Parse the workflow
    let workflow = MermaidParser::parse_with_metadata(
        input,
        "test_workflow",
        Some("Test Workflow with Mode".to_string()),
        Some("Tests mode flow".to_string()),
    )
    .expect("Should parse workflow");

    // Verify mode is in workflow
    assert_eq!(workflow.mode, Some("planner".to_string()));

    // Create a WorkflowRun
    let run = WorkflowRun::new(workflow);

    // Verify mode is in context
    assert_eq!(run.context.get_workflow_mode(), Some("planner".to_string()));
}

/// Test that workflow without mode has None in context
#[test]
fn test_no_mode_results_in_none_context() {
    let input = r#"---
title: Test Workflow without Mode
description: Tests no mode flow
---

```mermaid
stateDiagram-v2
    [*] --> Process
    Process --> [*]
```
"#;

    let workflow = MermaidParser::parse_with_metadata(
        input,
        "test_workflow",
        Some("Test Workflow without Mode".to_string()),
        Some("Tests no mode flow".to_string()),
    )
    .expect("Should parse workflow");

    assert_eq!(workflow.mode, None);

    let run = WorkflowRun::new(workflow);
    assert_eq!(run.context.get_workflow_mode(), None);
}

/// Test that different modes are correctly propagated
#[test]
fn test_various_modes_flow_correctly() {
    let modes = vec![
        ("planner", "Planning specialist"),
        ("implementer", "Implementation specialist"),
        ("reviewer", "Review specialist"),
        ("tester", "Testing specialist"),
        ("committer", "Commit specialist"),
        ("rule-checker", "Rule checking agent"),
    ];

    for (mode_id, description) in modes {
        let input = format!(
            r#"---
title: {} Workflow
description: {}
mode: {}
---

```mermaid
stateDiagram-v2
    [*] --> Work
    Work --> [*]
```
"#,
            mode_id, description, mode_id
        );

        let workflow_name = format!("{}_workflow", mode_id);
        let workflow = MermaidParser::parse_with_metadata(
            &input,
            workflow_name,
            Some(format!("{} Workflow", mode_id)),
            Some(description.to_string()),
        )
        .unwrap_or_else(|_| panic!("Should parse {} workflow", mode_id));

        assert_eq!(
            workflow.mode,
            Some(mode_id.to_string()),
            "Workflow should have mode '{}'",
            mode_id
        );

        let run = WorkflowRun::new(workflow);
        assert_eq!(
            run.context.get_workflow_mode(),
            Some(mode_id.to_string()),
            "Context should have mode '{}'",
            mode_id
        );
    }
}

/// Test that mode is preserved when context is cloned
#[test]
fn test_mode_preserved_on_context_clone() {
    let mut workflow = create_workflow("Test Workflow", "Test description", "start");
    workflow.add_state(create_state("start", "Start state", false));
    workflow.mode = Some("implementer".to_string());

    let run = WorkflowRun::new(workflow);

    // Clone the context
    let cloned_context = run.context.clone();

    // Mode should be preserved
    assert_eq!(
        cloned_context.get_workflow_mode(),
        Some("implementer".to_string())
    );
}

/// Test that mode is accessible alongside other workflow variables
#[test]
fn test_mode_coexists_with_other_variables() {
    let mut workflow = create_workflow("Test Workflow", "Test description", "start");
    workflow.add_state(create_state("start", "Start state", false));
    workflow.mode = Some("reviewer".to_string());

    let mut run = WorkflowRun::new(workflow);

    // Set additional workflow variables
    run.context
        .set_workflow_var("custom_var".to_string(), serde_json::json!("custom_value"));
    run.context
        .set_workflow_var("another_var".to_string(), serde_json::json!(42));

    // Mode should still be accessible
    assert_eq!(
        run.context.get_workflow_mode(),
        Some("reviewer".to_string())
    );

    // Other variables should also be accessible
    assert_eq!(
        run.context.get("custom_var").and_then(|v| v.as_str()),
        Some("custom_value")
    );
    assert_eq!(
        run.context.get("another_var").and_then(|v| v.as_i64()),
        Some(42)
    );
}

/// Test that mode flows through to workflow hashmap (used by actions)
#[test]
fn test_mode_in_workflow_hashmap_for_actions() {
    let mut workflow = create_workflow("Test Workflow", "Test description", "start");
    workflow.add_state(create_state("start", "Start state", false));
    workflow.mode = Some("tester".to_string());

    let run = WorkflowRun::new(workflow);

    // Get the hashmap that would be used by actions
    let hashmap = run.context.to_workflow_hashmap();

    // Mode should be in the hashmap
    assert!(hashmap.contains_key("_workflow_mode"));
    assert_eq!(
        hashmap.get("_workflow_mode").and_then(|v| v.as_str()),
        Some("tester")
    );
}

/// Test context creation with mode set manually (simulates executor setting mode)
#[test]
fn test_manual_mode_setting_in_context() {
    let mut context = WorkflowTemplateContext::with_vars_for_test(HashMap::new());

    // Initially no mode
    assert_eq!(context.get_workflow_mode(), None);

    // Set mode as executor would do
    context.set_workflow_mode(Some("planner".to_string()));

    // Mode should be retrievable
    assert_eq!(context.get_workflow_mode(), Some("planner".to_string()));

    // Get workflow mode should return the same value
    let mode = context.get_workflow_mode();
    assert_eq!(mode, Some("planner".to_string()));
}

/// Test that mode is correctly handled in workflow with parameters
#[test]
fn test_mode_with_workflow_parameters() {
    let input = r#"---
title: Parameterized Workflow
description: Workflow with both mode and parameters
mode: implementer
parameters:
  - name: input_file
    description: Input file path
    required: true
    type: string
---

```mermaid
stateDiagram-v2
    [*] --> Process
    Process --> [*]
```
"#;

    let workflow = MermaidParser::parse_with_metadata(
        input,
        "param_workflow",
        Some("Parameterized Workflow".to_string()),
        Some("Workflow with both mode and parameters".to_string()),
    )
    .expect("Should parse workflow with parameters and mode");

    // Both mode and parameters should be present
    assert_eq!(workflow.mode, Some("implementer".to_string()));
    assert_eq!(workflow.parameters.len(), 1);
    assert_eq!(workflow.parameters[0].name, "input_file");

    let run = WorkflowRun::new(workflow);
    assert_eq!(
        run.context.get_workflow_mode(),
        Some("implementer".to_string())
    );
}

/// Integration test simulating full workflow execution setup
#[test]
fn test_full_workflow_mode_integration() {
    // This simulates the full flow from parsing to execution context setup

    // Step 1: Parse workflow with mode
    let input = r#"---
title: Full Integration Test
description: Tests complete mode flow
mode: reviewer
tags:
  - test
---

```mermaid
stateDiagram-v2
    [*] --> Review
    Review --> Complete
    Complete --> [*]
```

## Actions

- Review: log "Reviewing code"
- Complete: log "Review complete"
"#;

    let workflow = MermaidParser::parse_with_metadata(
        input,
        "integration_test",
        Some("Full Integration Test".to_string()),
        Some("Tests complete mode flow".to_string()),
    )
    .expect("Should parse integration test workflow");

    // Step 2: Verify workflow has mode
    assert_eq!(workflow.mode, Some("reviewer".to_string()));
    assert_eq!(workflow.states.len(), 2);

    // Step 3: Create WorkflowRun (as executor would)
    let run = WorkflowRun::new(workflow);

    // Step 4: Verify context has mode
    assert_eq!(
        run.context.get_workflow_mode(),
        Some("reviewer".to_string())
    );

    // Step 5: Verify mode would be available to actions via hashmap
    let action_context = run.context.to_workflow_hashmap();
    assert_eq!(
        action_context
            .get("_workflow_mode")
            .and_then(|v| v.as_str()),
        Some("reviewer")
    );

    // Step 6: Verify mode can be retrieved for ACP session setup
    let mode_for_acp = run.context.get_workflow_mode();
    assert_eq!(mode_for_acp, Some("reviewer".to_string()));
}
