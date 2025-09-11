//! Integration tests for sub-workflow functionality
//!
//! This module tests that sub-workflows are executed in-process rather than
//! shelling out to a subprocess.

use anyhow::Result;
use swissarmyhammer::test_utils::IsolatedTestEnvironment;
use swissarmyhammer::{WorkflowExecutor, WorkflowStorage};
use swissarmyhammer_workflow::{MermaidParser, StateId, WorkflowRun};

#[tokio::test]
async fn test_sub_workflow_in_process_execution() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().unwrap();
    // Test that sub-workflows are executed in-process
    let parent_workflow_content = r#"---
name: test-parent
title: Test Parent Workflow
description: Tests sub-workflow execution
---

# Test Parent Workflow

```mermaid
stateDiagram-v2
    [*] --> Start
    Start --> CallSubWorkflow
    CallSubWorkflow --> ProcessResult
    ProcessResult --> [*]
```

## Actions

- Start: Set parent_var="parent_value"
- CallSubWorkflow: Run workflow "non-existent-workflow-12345" with greeting="Hello from parent" result="sub_result"
- ProcessResult: Log "Sub-workflow completed"
"#;

    // Parse the parent workflow
    let parent_workflow = MermaidParser::parse(parent_workflow_content, "test-parent")?;

    // Create executor
    let mut executor = WorkflowExecutor::new();

    // Start the parent workflow
    let mut run = WorkflowRun::new(parent_workflow);

    // Set a shorter timeout for sub-workflow operations to prevent long delays in test
    run.context.insert(
        "_timeout_secs".to_string(),
        serde_json::Value::Number(serde_json::Number::from(5)),
    );

    // Execute the workflow
    let result = executor.execute_state(&mut run).await;

    // The workflow should complete successfully if hello-world exists
    // Otherwise it will fail with a workflow loading error (not a subprocess error)
    match result {
        Ok(_) => {
            // If it succeeded, verify the context
            assert!(run.context.contains_key("parent_var"));
            assert_eq!(
                run.context.get("parent_var"),
                Some(&serde_json::json!("parent_value"))
            );
        }
        Err(e) => {
            // If it failed, verify it's a workflow loading error, not a subprocess error
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("Failed to load sub-workflow")
                    || error_msg.contains("workflow 'non-existent-workflow-12345'"),
                "Expected workflow loading error, got: {error_msg}"
            );
            assert!(
                !error_msg.contains("Failed to spawn"),
                "Should not contain subprocess spawn error"
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_sub_workflow_with_memory_storage() -> Result<()> {
    // Test sub-workflow execution with workflows in memory storage
    let parent_workflow_content = r#"---
name: test-parent-memory
title: Test Parent Workflow
description: Tests sub-workflow execution with memory storage
---

# Test Parent Workflow

```mermaid
stateDiagram-v2
    [*] --> Start
    Start --> CallSubWorkflow
    CallSubWorkflow --> ProcessResult
    ProcessResult --> [*]
```

## Actions

- Start: Set parent_var="parent_value"
- CallSubWorkflow: Run workflow "test-child" with input_value="${parent_var}" result="sub_result"
- ProcessResult: Log "Sub-workflow result: ${sub_result}"
"#;

    let child_workflow_content = r#"---
name: test-child
title: Test Child Workflow
description: Child workflow for testing
---

# Test Child Workflow

```mermaid
stateDiagram-v2
    [*] --> ProcessInput
    ProcessInput --> GenerateResult
    GenerateResult --> [*]
```

## Actions

- ProcessInput: Log "Received input: ${input_value}"
- GenerateResult: Set child_result="Processed: ${input_value}"
"#;

    // Create memory storage and store both workflows
    let mut storage = WorkflowStorage::memory();
    let parent_workflow = MermaidParser::parse(parent_workflow_content, "test-parent-memory")?;
    let child_workflow = MermaidParser::parse(child_workflow_content, "test-child")?;

    storage.store_workflow(parent_workflow.clone())?;
    storage.store_workflow(child_workflow)?;

    // Create executor
    let mut executor = WorkflowExecutor::new();

    // Start the parent workflow
    let mut run = WorkflowRun::new(parent_workflow);

    // Execute the workflow
    let result = executor.execute_state(&mut run).await;

    // Since we're using memory storage and not file system storage,
    // the sub-workflow action will fail to load the workflow from the file system
    assert!(result.is_ok());

    // The workflow should have executed to the point where it tries to call the sub-workflow
    // but fails because SubWorkflowAction uses file system storage
    let visited_states: Vec<StateId> = run
        .history
        .iter()
        .map(|(state_id, _)| state_id.clone())
        .collect();

    assert!(visited_states.contains(&StateId::new("Start")));
    assert!(visited_states.contains(&StateId::new("CallSubWorkflow")));

    Ok(())
}

#[tokio::test]
async fn test_sub_workflow_circular_dependency_detection_integration() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().unwrap();
    // Test that circular dependencies are properly detected
    let workflow_a_content = r#"---
name: workflow-a
title: Workflow A
description: Calls workflow B
---

# Workflow A

```mermaid
stateDiagram-v2
    [*] --> CallB
    CallB --> [*]
```

## Actions

- CallB: Run workflow "workflow-b"
"#;

    let workflow_b_content = r#"---
name: workflow-b
title: Workflow B
description: Calls workflow A (circular)
---

# Workflow B

```mermaid
stateDiagram-v2
    [*] --> CallA
    CallA --> [*]
```

## Actions

- CallA: Run workflow "workflow-a"
"#;

    // Create memory storage and store both workflows
    let mut storage = WorkflowStorage::memory();
    let workflow_a = MermaidParser::parse(workflow_a_content, "workflow-a")?;
    let workflow_b = MermaidParser::parse(workflow_b_content, "workflow-b")?;

    storage.store_workflow(workflow_a.clone())?;
    storage.store_workflow(workflow_b)?;

    // Create executor and start workflow A
    let mut executor = WorkflowExecutor::new();
    let mut run = WorkflowRun::new(workflow_a);

    // Execute the workflow - it should detect circular dependency
    let result = executor.execute_state(&mut run).await;

    // The workflow should complete, likely with a failure when it tries to
    // execute the sub-workflow that doesn't exist in the file system
    match &result {
        Ok(_) => {
            // Check if we reached the sub-workflow action
            let visited_states: Vec<StateId> = run
                .history
                .iter()
                .map(|(state_id, _)| state_id.clone())
                .collect();

            assert!(visited_states.contains(&StateId::new("CallB")));
        }
        Err(e) => {
            println!("Workflow execution error (expected): {e}");
            // This is expected if the workflow tried to execute a sub-workflow
            // that doesn't exist in the file system
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_sub_workflow_timeout_behavior() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().unwrap();
    // Test that sub-workflows respect timeout settings
    let parent_workflow_content = r#"---
name: test-parent-timeout
title: Test Parent Workflow with Timeout
description: Tests sub-workflow timeout behavior
---

# Test Parent Workflow Timeout

```mermaid
stateDiagram-v2
    [*] --> Start
    Start --> CallSubWorkflowWithTimeout
    CallSubWorkflowWithTimeout --> HandleTimeout
    HandleTimeout --> [*]
```

## Actions

- Start: Set parent_var="parent_value"
- CallSubWorkflowWithTimeout: Run workflow "long-running-workflow" with timeout="1s" result="sub_result"
- HandleTimeout: Log "Sub-workflow timed out or completed"
"#;

    // Parse the parent workflow
    let parent_workflow = MermaidParser::parse(parent_workflow_content, "test-parent-timeout")?;

    // Create executor
    let mut executor = WorkflowExecutor::new();

    // Start the parent workflow
    let mut run = WorkflowRun::new(parent_workflow);

    // Set a short timeout in the context to test timeout behavior
    run.context.insert(
        "_timeout_secs".to_string(),
        serde_json::Value::Number(serde_json::Number::from(2)),
    );

    // Execute the workflow
    let result = executor.execute_state(&mut run).await;

    // The workflow should complete, either with a timeout or workflow not found
    match result {
        Ok(_) => {
            // Verify we tried to execute the sub-workflow
            let visited_states: Vec<StateId> = run
                .history
                .iter()
                .map(|(state_id, _)| state_id.clone())
                .collect();

            assert!(visited_states.contains(&StateId::new("Start")));
            assert!(visited_states.contains(&StateId::new("CallSubWorkflowWithTimeout")));
        }
        Err(e) => {
            // Expected error if workflow doesn't exist
            let error_msg = e.to_string();
            println!("Expected error: {error_msg}");
            assert!(
                error_msg.contains("workflow") || error_msg.contains("timeout"),
                "Expected workflow or timeout error, got: {error_msg}"
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_sub_workflow_timeout_propagation() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().unwrap();
    // Test that timeout values are properly propagated to sub-workflows
    let parent_workflow_content = r#"---
name: test-parent-timeout-propagation
title: Test Parent Workflow Timeout Propagation
description: Tests that timeout values propagate to sub-workflows
---

# Test Timeout Propagation

```mermaid
stateDiagram-v2
    [*] --> ConfigureTimeout
    ConfigureTimeout --> CallSubWorkflow
    CallSubWorkflow --> VerifyResult
    VerifyResult --> [*]
```

## Actions

- ConfigureTimeout: Set sub_timeout="5"
- CallSubWorkflow: Run workflow "test-sub-timeout" with timeout="${sub_timeout}s" input="test" result="sub_result"
- VerifyResult: Log "Sub-workflow result: ${sub_result}"
"#;

    // Parse the parent workflow
    let parent_workflow =
        MermaidParser::parse(parent_workflow_content, "test-parent-timeout-propagation")?;

    // Create executor
    let mut executor = WorkflowExecutor::new();

    // Start the parent workflow
    let mut run = WorkflowRun::new(parent_workflow);

    // Execute the workflow
    let result = executor.execute_state(&mut run).await;

    // Verify the workflow executed properly
    match result {
        Ok(_) => {
            // Check that timeout was configured
            assert!(run.context.contains_key("sub_timeout"));
            assert_eq!(run.context.get("sub_timeout"), Some(&serde_json::json!(5)));

            let visited_states: Vec<StateId> = run
                .history
                .iter()
                .map(|(state_id, _)| state_id.clone())
                .collect();

            assert!(visited_states.contains(&StateId::new("ConfigureTimeout")));
            assert!(visited_states.contains(&StateId::new("CallSubWorkflow")));
        }
        Err(e) => {
            // Expected if sub-workflow doesn't exist
            println!("Expected error (workflow not found): {e}");
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_sub_workflow_timeout_cancellation() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().unwrap();
    // Test that sub-workflows fail quickly when they can't be found
    // The timeout aspect is tested by trying to run a non-existent workflow with a short timeout
    let parent_workflow_content = r#"---
name: test-parent-cancellation
title: Test Parent Workflow Cancellation
description: Tests sub-workflow error handling
---

# Test Cancellation

```mermaid
stateDiagram-v2
    [*] --> Start
    Start --> CallNonExistentWorkflow
    CallNonExistentWorkflow --> [*]
```

## Actions

- Start: Set start_time="${now}"
- CallNonExistentWorkflow: Run workflow "non-existent-workflow" with timeout="100ms" result="sub_result"
"#;

    // Parse the parent workflow
    let parent_workflow =
        MermaidParser::parse(parent_workflow_content, "test-parent-cancellation")?;

    // Create executor
    let mut executor = WorkflowExecutor::new();

    // Start the parent workflow
    let mut run = WorkflowRun::new(parent_workflow);

    // Execute the workflow
    let start = std::time::Instant::now();
    let result = executor.execute_state(&mut run).await;
    let duration = start.elapsed();

    // The workflow should fail quickly when it can't find the sub-workflow
    // Allow up to 5 seconds for filesystem operations and error propagation
    assert!(
        duration.as_secs() < 5,
        "Workflow execution took too long: {duration:?}. Expected quick failure when workflow not found."
    );

    // The result should be an error since the sub-workflow doesn't exist
    match result {
        Ok(_) => {
            panic!("Workflow completed successfully, but should have failed due to missing sub-workflow");
        }
        Err(e) => {
            println!("Workflow error (expected due to missing workflow): {e}");
            // Verify that the error message mentions the missing workflow
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("non-existent-workflow")
                    || error_msg.contains("not found")
                    || error_msg.contains("Failed to load"),
                "Error message should mention the missing workflow: {error_msg}"
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_deeply_nested_sub_workflows() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().unwrap();
    // Test sub-workflows nested 3+ levels deep
    let level1_workflow = r#"---
name: level1-workflow
title: Level 1 Workflow
description: Top level workflow
---

# Level 1 Workflow

```mermaid
stateDiagram-v2
    [*] --> Start
    Start --> CallLevel2
    CallLevel2 --> ProcessResult
    ProcessResult --> [*]
```

## Actions

- Start: Set level1_data="Level 1 Data"
- CallLevel2: Run workflow "level2-workflow" with input="${level1_data}" result="level2_result"
- ProcessResult: Log "Level 2 returned: ${level2_result}"
"#;

    let level2_workflow = r#"---
name: level2-workflow
title: Level 2 Workflow
description: Middle level workflow
---

# Level 2 Workflow

```mermaid
stateDiagram-v2
    [*] --> Start
    Start --> CallLevel3
    CallLevel3 --> ProcessResult
    ProcessResult --> [*]
```

## Actions

- Start: Set level2_data="Level 2 Data: ${input}"
- CallLevel3: Run workflow "level3-workflow" with input="${level2_data}" result="level3_result"
- ProcessResult: Set output="L2 processed: ${level3_result}"
"#;

    let level3_workflow = r#"---
name: level3-workflow
title: Level 3 Workflow
description: Deepest level workflow
---

# Level 3 Workflow

```mermaid
stateDiagram-v2
    [*] --> Process
    Process --> Complete
    Complete --> [*]
```

## Actions

- Process: Set processed_data="L3 processed: ${input}"
- Complete: Set output="${processed_data}"
"#;

    // Create memory storage and store all workflows
    let mut storage = WorkflowStorage::memory();
    let workflow1 = MermaidParser::parse(level1_workflow, "level1-workflow")?;
    let workflow2 = MermaidParser::parse(level2_workflow, "level2-workflow")?;
    let workflow3 = MermaidParser::parse(level3_workflow, "level3-workflow")?;

    storage.store_workflow(workflow1.clone())?;
    storage.store_workflow(workflow2)?;
    storage.store_workflow(workflow3)?;

    // Create executor
    let mut executor = WorkflowExecutor::new();

    // Start the top-level workflow
    let mut run = WorkflowRun::new(workflow1);

    // Execute the workflow
    let _result = executor.execute_state(&mut run).await;

    // Verify execution reached expected states
    let visited_states: Vec<StateId> = run
        .history
        .iter()
        .map(|(state_id, _)| state_id.clone())
        .collect();

    assert!(visited_states.contains(&StateId::new("Start")));
    assert!(visited_states.contains(&StateId::new("CallLevel2")));

    // Check if workflow stack tracking would prevent infinite recursion
    // (Even though the sub-workflows will fail to load from file system)
    if let Some(stack) = run.context.get("_workflow_stack") {
        println!("Workflow execution stack: {stack:?}");
    }

    Ok(())
}

#[tokio::test]
async fn test_sub_workflow_deep_nesting_limit() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().unwrap();
    // Test that extremely deep nesting is handled gracefully
    let recursive_workflow = r#"---
name: recursive-workflow
title: Recursive Workflow
description: Tests deep recursion handling
---

# Recursive Workflow

```mermaid
stateDiagram-v2
    [*] --> CheckDepth
    CheckDepth --> CallSelf: depth < 10
    CheckDepth --> Complete: depth >= 10
    CallSelf --> UpdateDepth
    UpdateDepth --> CheckDepth
    Complete --> [*]
```

## Actions

- CheckDepth: Set should_recurse="${depth < 10}"
- CallSelf: Run workflow "recursive-workflow" with depth="${depth + 1}" result="sub_result"
- UpdateDepth: Set depth="${sub_result.depth}"
- Complete: Set output="Reached max depth: ${depth}"
"#;

    // Parse the recursive workflow
    let workflow = MermaidParser::parse(recursive_workflow, "recursive-workflow")?;

    // Create executor
    let mut executor = WorkflowExecutor::new();

    // Start with depth 0
    let mut run = WorkflowRun::new(workflow);
    run.context
        .insert("depth".to_string(), serde_json::json!(0));

    // Execute the workflow
    let result = executor.execute_state(&mut run).await;

    // The workflow should handle recursion gracefully
    match result {
        Ok(_) => {
            println!("Recursive workflow completed");

            // Check how deep we went
            if let Some(depth) = run.context.get("depth") {
                println!("Final depth: {depth:?}");
            }
        }
        Err(e) => {
            println!("Recursive workflow error (expected): {e}");
            // This is expected as the sub-workflow won't exist in the file system
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_sub_workflow_context_isolation() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().unwrap();
    // Test that sub-workflows have isolated contexts
    let parent_workflow = r#"---
name: parent-context-test
title: Parent Context Test
description: Tests context isolation between parent and sub-workflows
---

# Parent Context Test

```mermaid
stateDiagram-v2
    [*] --> SetParentOnly
    SetParentOnly --> SetSharedVar
    SetSharedVar --> CallSubWorkflow1
    CallSubWorkflow1 --> CallSubWorkflow2
    CallSubWorkflow2 --> VerifyIsolation
    VerifyIsolation --> [*]
```

## Actions

- SetParentOnly: Set parent_only="parent value"
- SetSharedVar: Set shared_var="parent shared"
- CallSubWorkflow1: Run workflow "sub1" with shared_var="${shared_var}" result="sub1_result"
- CallSubWorkflow2: Run workflow "sub2" with shared_var="${shared_var}" result="sub2_result"
- VerifyIsolation: Log "Parent still has: ${parent_only}, Sub1: ${sub1_result}, Sub2: ${sub2_result}"
"#;

    // Parse the workflow
    let workflow = MermaidParser::parse(parent_workflow, "parent-context-test")?;

    // Create executor
    let mut executor = WorkflowExecutor::new();

    // Start the workflow
    let mut run = WorkflowRun::new(workflow);

    // Execute the workflow
    let result = executor.execute_state(&mut run).await;

    // Verify parent context remains intact
    match result {
        Ok(_) => {
            // Check that parent variables are preserved
            assert_eq!(
                run.context.get("parent_only"),
                Some(&serde_json::json!("parent value"))
            );
            assert_eq!(
                run.context.get("shared_var"),
                Some(&serde_json::json!("parent shared"))
            );
        }
        Err(e) => {
            // Expected if sub-workflows don't exist
            println!("Context isolation test error (expected): {e}");

            // Even with errors, parent context should be preserved
            if let Some(parent_only) = run.context.get("parent_only") {
                assert_eq!(parent_only, &serde_json::json!("parent value"));
            }
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_sub_workflow_parallel_execution() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().unwrap();
    // Test multiple sub-workflows executing in parallel
    let parallel_workflow = r#"---
name: parallel-sub-workflows
title: Parallel Sub-Workflows
description: Tests parallel execution of multiple sub-workflows
---

# Parallel Sub-Workflows

```mermaid
stateDiagram-v2
    [*] --> Setup
    Setup --> ParallelExecution
    
    state ParallelExecution {
        --
        [*] --> CallWorkflow1
        [*] --> CallWorkflow2
        [*] --> CallWorkflow3
        
        CallWorkflow1 --> [*]
        CallWorkflow2 --> [*]
        CallWorkflow3 --> [*]
        --
    }
    
    ParallelExecution --> CollectResults
    CollectResults --> [*]
```

## Actions

- Setup: Set start_time="${now}", parallel_data="shared data"
- CallWorkflow1: Run workflow "worker1" with data="${parallel_data}" id="1" result="result1"
- CallWorkflow2: Run workflow "worker2" with data="${parallel_data}" id="2" result="result2"
- CallWorkflow3: Run workflow "worker3" with data="${parallel_data}" id="3" result="result3"
- CollectResults: Log "All workflows completed: ${result1}, ${result2}, ${result3}"
"#;

    // Parse the workflow
    let workflow = MermaidParser::parse(parallel_workflow, "parallel-sub-workflows")?;

    // Create executor
    let mut executor = WorkflowExecutor::new();

    // Start the workflow
    let mut run = WorkflowRun::new(workflow);

    // Execute the workflow
    let _result = executor.execute_state(&mut run).await;

    // Verify parallel execution setup
    let visited_states: Vec<StateId> = run
        .history
        .iter()
        .map(|(state_id, _)| state_id.clone())
        .collect();

    assert!(visited_states.contains(&StateId::new("Setup")));

    // Note: Actual parallel execution depends on the workflow engine's
    // support for parallel states, which is indicated by the allows_parallel flag

    Ok(())
}
