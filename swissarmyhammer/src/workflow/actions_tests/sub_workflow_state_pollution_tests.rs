//! Tests for state name pollution in nested workflows
//!
//! This module tests that nested workflows with the same state names
//! don't interfere with each other during execution.

use crate::test_utils::IsolatedTestEnvironment;
use crate::workflow::actions::{clear_test_storage, set_test_storage};
use crate::workflow::storage::{
    MemoryWorkflowRunStorage, MemoryWorkflowStorage, WorkflowStorageBackend,
};
use crate::workflow::{MermaidParser, WorkflowExecutor, WorkflowStorage};
use serde_json::Value;

use std::sync::Arc;

/// Helper function to set up test storage with workflows
fn setup_test_storage_with_workflows(workflows: &[(&str, &str)]) -> Arc<WorkflowStorage> {
    let mut workflow_storage = MemoryWorkflowStorage::new();
    let run_storage = MemoryWorkflowRunStorage::new();

    // Parse and store workflows
    for (name, content) in workflows {
        let workflow = MermaidParser::parse(content, *name).unwrap();
        workflow_storage.store_workflow(workflow).unwrap();
    }

    Arc::new(WorkflowStorage::new(
        Arc::new(workflow_storage),
        Arc::new(run_storage),
    ))
}

#[tokio::test]
async fn test_nested_workflow_state_name_pollution() {
    let _guard =
        IsolatedTestEnvironment::new().expect("Failed to create isolated test environment");
    // This test verifies that when a parent workflow calls a sub-workflow,
    // and both workflows have states with the same names (1, 2, 3),
    // the sub-workflow's state transitions don't interfere with the parent's state management.

    // Create a parent workflow with states 1, 2, 3
    let parent_workflow_content = r#"---
name: workflow-a
title: Workflow A
description: Parent workflow with states 1, 2, 3
---

# Workflow A

```mermaid
stateDiagram-v2
    [*] --> 1
    1 --> 2
    2 --> 3
    3 --> [*]
```

## Actions

- 1: Set parent_state="parent_1"
- 2: Run workflow "workflow-b" with result="sub_result"
- 3: Log "Parent workflow state 3: parent_state=${parent_state}, sub_result=${sub_result}"
"#;

    // Create a child workflow with the same state names 1, 2, 3
    let child_workflow_content = r#"---
name: workflow-b
title: Workflow B
description: Child workflow with states 1, 2, 3
---

# Workflow B

```mermaid
stateDiagram-v2
    [*] --> 1
    1 --> 2
    2 --> 3
    3 --> [*]
```

## Actions

- 1: Set child_state="child_1"
- 2: Set child_state="child_2"
- 3: Set child_state="child_3"
"#;

    // Set up test storage with workflows
    let storage = setup_test_storage_with_workflows(&[
        ("workflow-a", parent_workflow_content),
        ("workflow-b", child_workflow_content),
    ]);
    set_test_storage(storage);

    // Parse workflows
    let parent_workflow = MermaidParser::parse(parent_workflow_content, "workflow-a").unwrap();

    // Create executor
    let mut executor = WorkflowExecutor::new();

    // Start the parent workflow
    let mut run = executor.start_workflow(parent_workflow).unwrap();

    // Add debug logging to track state executions
    run.context
        .insert("_debug_logging".to_string(), Value::Bool(true));

    // Execute the workflow
    let result = executor.execute_state(&mut run).await;

    // Check that the workflow completed successfully
    assert!(result.is_ok(), "Workflow execution failed: {result:?}");

    // Workflow execution should have completed successfully
    // The parent workflow should have executed its state 3 after the sub-workflow completed

    // Verify that parent state values were not overwritten by child workflow
    assert_eq!(
        run.context.get("parent_state"),
        Some(&Value::String("parent_1".to_string())),
        "Parent state value was overwritten"
    );

    // Verify that sub-workflow result contains child state values
    if let Some(sub_result) = run.context.get("sub_result") {
        if let Some(obj) = sub_result.as_object() {
            assert_eq!(
                obj.get("child_state"),
                Some(&Value::String("child_3".to_string())),
                "Child workflow did not complete properly"
            );
        } else {
            panic!("sub_result is not an object");
        }
    } else {
        panic!("sub_result not found in context");
    }

    // Clean up test storage
    clear_test_storage();
}

#[tokio::test]
async fn test_nested_workflow_correct_action_execution() {
    let _guard =
        IsolatedTestEnvironment::new().expect("Failed to create isolated test environment");
    // Create a more complex test where both workflows have conflicting state names
    // and we verify that each workflow executes its own actions
    let parent_workflow_content = r#"---
name: workflow-parent
title: Parent Workflow
description: Parent workflow that calls child workflow
---

# Parent Workflow

```mermaid
stateDiagram-v2
    [*] --> Init
    Init --> Process
    Process --> CallChild
    CallChild --> Verify
    Verify --> [*]
```

## Actions

- Init: Set execution_log="parent:Init"
- Process: Set execution_log="${execution_log},parent:Process"
- CallChild: Run workflow "workflow-child" with parent_log="${execution_log}" result="child_result"
- Verify: Set execution_log="${execution_log},parent:Verify"
"#;

    let child_workflow_content = r#"---
name: workflow-child
title: Child Workflow
description: Child workflow with potentially conflicting state names
---

# Child Workflow

```mermaid
stateDiagram-v2
    [*] --> Init
    Init --> Process
    Process --> Complete
    Complete --> [*]
```

## Actions

- Init: Set child_log="child:Init,received:${parent_log}"
- Process: Set child_log="${child_log},child:Process"
- Complete: Set child_log="${child_log},child:Complete"
"#;

    // Set up test storage with workflows
    let storage = setup_test_storage_with_workflows(&[
        ("workflow-parent", parent_workflow_content),
        ("workflow-child", child_workflow_content),
    ]);
    set_test_storage(storage);

    // Parse workflows
    let parent_workflow = MermaidParser::parse(parent_workflow_content, "workflow-parent").unwrap();

    // Create executor
    let mut executor = WorkflowExecutor::new();

    // Start and execute the parent workflow
    let run = executor
        .start_and_execute_workflow(parent_workflow)
        .await
        .unwrap();

    // Verify execution log shows correct sequence
    assert_eq!(
        run.context.get("execution_log"),
        Some(&Value::String(
            "parent:Init,parent:Process,parent:Verify".to_string()
        )),
        "Parent workflow did not execute in correct order"
    );

    // Verify child workflow executed correctly
    if let Some(child_result) = run.context.get("child_result") {
        if let Some(obj) = child_result.as_object() {
            assert_eq!(
                obj.get("child_log"),
                Some(&Value::String(
                    "child:Init,received:parent:Init,parent:Process,child:Process,child:Complete"
                        .to_string()
                )),
                "Child workflow did not execute correctly"
            );
        } else {
            panic!("child_result is not an object");
        }
    } else {
        panic!("child_result not found in context");
    }

    // Clean up test storage
    clear_test_storage();
}

#[tokio::test]
async fn test_deeply_nested_workflows_state_isolation() {
    let _guard =
        IsolatedTestEnvironment::new().expect("Failed to create isolated test environment");
    // Test with 3 levels of nesting to ensure state isolation works at depth
    let workflow_a_content = r#"---
name: workflow-level-a
title: Level A Workflow
description: Top level workflow
---

# Level A Workflow

```mermaid
stateDiagram-v2
    [*] --> StateA
    StateA --> CallB
    CallB --> FinalA
    FinalA --> [*]
```

## Actions

- StateA: Set level_a_data="A-executed"
- CallB: Run workflow "workflow-level-b" with a_data="${level_a_data}" result="b_result"
- FinalA: Set final_a="${level_a_data},${b_result}"
"#;

    let workflow_b_content = r#"---
name: workflow-level-b
title: Level B Workflow
description: Middle level workflow
---

# Level B Workflow

```mermaid
stateDiagram-v2
    [*] --> StateA
    StateA --> CallC
    CallC --> FinalB
    FinalB --> [*]
```

## Actions

- StateA: Set level_b_data="B-executed,got:${a_data}"
- CallC: Run workflow "workflow-level-c" with b_data="${level_b_data}" result="c_result"
- FinalB: Set final_b="${level_b_data},${c_result}"
"#;

    let workflow_c_content = r#"---
name: workflow-level-c
title: Level C Workflow
description: Deepest level workflow
---

# Level C Workflow

```mermaid
stateDiagram-v2
    [*] --> StateA
    StateA --> FinalC
    FinalC --> [*]
```

## Actions

- StateA: Set level_c_data="C-executed,got:${b_data}"
- FinalC: Set final_c="${level_c_data}"
"#;

    // Set up test storage with workflows
    let storage = setup_test_storage_with_workflows(&[
        ("workflow-level-a", workflow_a_content),
        ("workflow-level-b", workflow_b_content),
        ("workflow-level-c", workflow_c_content),
    ]);
    set_test_storage(storage);

    // Parse workflows
    let workflow_a = MermaidParser::parse(workflow_a_content, "workflow-level-a").unwrap();

    // Execute top-level workflow
    let mut executor = WorkflowExecutor::new();
    let run = executor
        .start_and_execute_workflow(workflow_a)
        .await
        .unwrap();

    // Verify that all levels executed correctly with proper state isolation
    assert_eq!(
        run.context.get("level_a_data"),
        Some(&Value::String("A-executed".to_string())),
        "Level A data incorrect"
    );

    // Check that nested results are properly propagated
    if let Some(b_result) = run.context.get("b_result") {
        if let Some(b_obj) = b_result.as_object() {
            // Level B should have its own data
            assert!(
                b_obj
                    .get("level_b_data")
                    .and_then(|v| v.as_str())
                    .map(|s| s.contains("B-executed,got:A-executed"))
                    .unwrap_or(false),
                "Level B did not receive correct data from A"
            );

            // Level C result should be nested in B's result
            if let Some(c_result) = b_obj.get("c_result") {
                if let Some(c_obj) = c_result.as_object() {
                    assert!(
                        c_obj
                            .get("level_c_data")
                            .and_then(|v| v.as_str())
                            .map(|s| s.contains("C-executed,got:B-executed"))
                            .unwrap_or(false),
                        "Level C did not receive correct data from B"
                    );
                }
            }
        }
    }

    // Clean up test storage
    clear_test_storage();
}
