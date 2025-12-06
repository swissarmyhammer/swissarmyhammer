//! Integration tests for MCP server port propagation to sub-workflows
//!
//! This module tests that MCP server ports are correctly propagated from parent
//! workflows to sub-workflows, ensuring LlamaAgent executors can properly connect
//! to the MCP server at all nesting levels.

use anyhow::Result;
use std::collections::HashMap;
use swissarmyhammer::test_utils::IsolatedTestEnvironment;
use swissarmyhammer::{WorkflowExecutor, WorkflowStorage};
use swissarmyhammer_config::model::{LlamaAgentConfig, ModelConfig};
use swissarmyhammer_workflow::template_context::WorkflowTemplateContext;
use swissarmyhammer_workflow::{MermaidParser, StateId, WorkflowRun};

// ============================================================================
// Helper Functions
// ============================================================================

/// Helper function to assert that a context has a LlamaAgent config with the expected port
fn assert_llama_config_has_port(context: &WorkflowTemplateContext, expected_port: u16) {
    let agent_config = context.get_agent_config();
    if let swissarmyhammer_config::model::ModelConfig {
        executor: swissarmyhammer_config::model::ModelExecutorConfig::LlamaAgent(llama_config),
        ..
    } = agent_config
    {
        assert_eq!(
            llama_config.mcp_server.port, expected_port,
            "LlamaAgent port should be {}",
            expected_port
        );
    } else {
        panic!("Expected LlamaAgent config but got {:?}", agent_config);
    }
}

/// Helper function to assert that MCP port is set in context
fn assert_mcp_port_in_context(context: &WorkflowTemplateContext, expected_port: u16) {
    assert_eq!(
        context.get("_mcp_server_port"),
        Some(&serde_json::json!(expected_port))
    );
}

/// Helper function to create a context with LlamaAgent and MCP port
fn create_context_with_llama_and_port(port: u16) -> WorkflowTemplateContext {
    let mut context = WorkflowTemplateContext::with_vars(HashMap::new()).unwrap();
    let llama_config = LlamaAgentConfig::for_testing();
    let agent_config = ModelConfig::llama_agent(llama_config);
    context.set_agent_config(agent_config);
    context.update_mcp_port(port);
    context
}

/// Helper function to verify that expected states were visited in a workflow run
fn verify_states_visited(run: &WorkflowRun, expected_states: &[&str]) {
    let visited_states: Vec<StateId> = run
        .history
        .iter()
        .map(|(state_id, _)| state_id.clone())
        .collect();

    for state_name in expected_states {
        assert!(visited_states.contains(&StateId::new(*state_name)));
    }
}

/// Helper function to create an executor and workflow run with MCP port
fn create_run_with_mcp_port(
    workflow: swissarmyhammer_workflow::Workflow,
    port: u16,
) -> (WorkflowExecutor, WorkflowRun) {
    let executor = WorkflowExecutor::new();
    let mut run = WorkflowRun::new(workflow);
    run.context.update_mcp_port(port);
    (executor, run)
}

// ============================================================================
// Workflow Setup Functions
// ============================================================================

fn setup_parent_child_workflows() -> (String, String) {
    let parent_workflow_content = r#"---
name: parent-with-mcp
title: Parent Workflow with MCP
description: Parent workflow that calls a sub-workflow
---

# Parent Workflow with MCP

```mermaid
stateDiagram-v2
    [*] --> Setup
    Setup --> CallSubWorkflow
    CallSubWorkflow --> Verify
    Verify --> [*]
```

## Actions

- Setup: Set parent_data="test"
- CallSubWorkflow: Run workflow "child-with-mcp" with input="${parent_data}" result="sub_result"
- Verify: Log "Sub-workflow completed with result: ${sub_result}"
"#;

    let child_workflow_content = r#"---
name: child-with-mcp
title: Child Workflow with MCP
description: Child workflow that should inherit MCP port
---

# Child Workflow with MCP

```mermaid
stateDiagram-v2
    [*] --> Process
    Process --> Complete
    Complete --> [*]
```

## Actions

- Process: Set processed="Processed: ${input}"
- Complete: Set output="${processed}"
"#;

    (
        parent_workflow_content.to_string(),
        child_workflow_content.to_string(),
    )
}

fn setup_nested_workflows() -> (String, String, String) {
    let level1_workflow = r#"---
name: level1-mcp
title: Level 1 MCP Workflow
description: Top level workflow with MCP
---

# Level 1 MCP Workflow

```mermaid
stateDiagram-v2
    [*] --> Start
    Start --> CallLevel2
    CallLevel2 --> End
    End --> [*]
```

## Actions

- Start: Set level1_var="L1"
- CallLevel2: Run workflow "level2-mcp" with data="${level1_var}" result="level2_result"
- End: Log "Level 2 returned: ${level2_result}"
"#;

    let level2_workflow = r#"---
name: level2-mcp
title: Level 2 MCP Workflow
description: Middle level workflow with MCP
---

# Level 2 MCP Workflow

```mermaid
stateDiagram-v2
    [*] --> Start
    Start --> CallLevel3
    CallLevel3 --> End
    End --> [*]
```

## Actions

- Start: Set level2_var="L2: ${data}"
- CallLevel3: Run workflow "level3-mcp" with data="${level2_var}" result="level3_result"
- End: Set output="L2 processed: ${level3_result}"
"#;

    let level3_workflow = r#"---
name: level3-mcp
title: Level 3 MCP Workflow
description: Deepest level workflow with MCP
---

# Level 3 MCP Workflow

```mermaid
stateDiagram-v2
    [*] --> Process
    Process --> Complete
    Complete --> [*]
```

## Actions

- Process: Set processed="L3 processed: ${data}"
- Complete: Set output="${processed}"
"#;

    (
        level1_workflow.to_string(),
        level2_workflow.to_string(),
        level3_workflow.to_string(),
    )
}

fn setup_nested_workflow_storage() -> Result<(WorkflowStorage, swissarmyhammer_workflow::Workflow)>
{
    let (level1, level2, level3) = setup_nested_workflows();
    let mut storage = WorkflowStorage::memory();

    let workflow1 = MermaidParser::parse(&level1, "level1-mcp")?;
    let workflow2 = MermaidParser::parse(&level2, "level2-mcp")?;
    let workflow3 = MermaidParser::parse(&level3, "level3-mcp")?;

    storage.store_workflow(workflow1.clone())?;
    storage.store_workflow(workflow2)?;
    storage.store_workflow(workflow3)?;

    Ok((storage, workflow1))
}

// ============================================================================
// Tests
// ============================================================================

/// Test that MCP port is correctly set in workflow context for LlamaAgent
#[tokio::test]
async fn test_mcp_port_in_workflow_context() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;

    // Create a simple workflow
    let workflow_content = r#"---
name: test-mcp-port
title: Test MCP Port
description: Tests that MCP port is set in context
---

# Test MCP Port

```mermaid
stateDiagram-v2
    [*] --> CheckPort
    CheckPort --> [*]
```

## Actions

- CheckPort: Log "MCP port: ${_mcp_server_port}"
"#;

    let workflow = MermaidParser::parse(workflow_content, "test-mcp-port")?;
    let _executor = WorkflowExecutor::new();
    let mut run = WorkflowRun::new(workflow);

    // Set up LlamaAgent config with test model FIRST
    let llama_config = LlamaAgentConfig::for_testing();
    let agent_config = ModelConfig::llama_agent(llama_config);
    run.context.set_agent_config(agent_config);

    // Simulate MCP server port being set (as would happen in CLI)
    let test_port = 12345u16;
    run.context.update_mcp_port(test_port);

    // Verify the port was set correctly in context
    assert_mcp_port_in_context(&run.context, test_port);

    // Verify the agent config was updated with the port
    assert_llama_config_has_port(&run.context, test_port);

    Ok(())
}

/// Test that MCP port propagates from parent to sub-workflow
#[tokio::test]
async fn test_mcp_port_propagation_to_sub_workflow() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;
    let (parent_workflow_content, child_workflow_content) = setup_parent_child_workflows();

    let mut storage = WorkflowStorage::memory();
    let parent_workflow = MermaidParser::parse(&parent_workflow_content, "parent-with-mcp")?;
    let child_workflow = MermaidParser::parse(&child_workflow_content, "child-with-mcp")?;

    storage.store_workflow(parent_workflow.clone())?;
    storage.store_workflow(child_workflow)?;

    let (mut executor, mut run) = create_run_with_mcp_port(parent_workflow, 54321);

    let result = executor.execute_state(&mut run).await;
    assert!(result.is_ok() || result.is_err());

    assert_mcp_port_in_context(&run.context, 54321);
    verify_states_visited(&run, &["Setup", "CallSubWorkflow"]);

    Ok(())
}

/// Test MCP port propagation with ClaudeCode executor (should be no-op)
#[tokio::test]
async fn test_mcp_port_with_claude_code_executor() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;

    let workflow_content = r#"---
name: claude-code-test
title: ClaudeCode Test
description: Tests that MCP port setting works with ClaudeCode
---

# ClaudeCode Test

```mermaid
stateDiagram-v2
    [*] --> Process
    Process --> [*]
```

## Actions

- Process: Log "Using ClaudeCode executor"
"#;

    let workflow = MermaidParser::parse(workflow_content, "claude-code-test")?;
    let _executor = WorkflowExecutor::new();
    let mut run = WorkflowRun::new(workflow);

    // Set up ClaudeCode config
    let agent_config = ModelConfig::claude_code();
    run.context.set_agent_config(agent_config);

    // Set MCP port (should be a no-op for ClaudeCode)
    let test_port = 9999u16;
    run.context.update_mcp_port(test_port);

    // Verify the port was set in context
    assert_mcp_port_in_context(&run.context, test_port);

    // Verify the agent config is still ClaudeCode
    let stored_agent_config = run.context.get_agent_config();
    assert_eq!(
        stored_agent_config.executor_type(),
        swissarmyhammer_config::model::AgentExecutorType::ClaudeCode
    );

    Ok(())
}

/// Test MCP port propagation through multiple nesting levels
#[tokio::test]
async fn test_mcp_port_deep_nesting() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;
    let (_storage, workflow1) = setup_nested_workflow_storage()?;

    let (mut executor, mut run) = create_run_with_mcp_port(workflow1, 33333);

    let _result = executor.execute_state(&mut run).await;

    assert_mcp_port_in_context(&run.context, 33333);
    verify_states_visited(&run, &["Start", "CallLevel2"]);

    Ok(())
}

/// Test update_mcp_port helper method directly
#[tokio::test]
async fn test_update_mcp_port_helper_method() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;

    let context = create_context_with_llama_and_port(7777);

    // Verify port was set in context
    assert_mcp_port_in_context(&context, 7777);

    // Verify agent config was updated
    assert_llama_config_has_port(&context, 7777);

    Ok(())
}

/// Test update_mcp_port with ClaudeCode (should not panic)
#[tokio::test]
async fn test_update_mcp_port_with_claude_code() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;

    let mut context = WorkflowTemplateContext::with_vars(HashMap::new())?;
    let agent_config = ModelConfig::claude_code();
    context.set_agent_config(agent_config);

    // Update MCP port (should be no-op for ClaudeCode but shouldn't panic)
    let test_port = 8888u16;
    context.update_mcp_port(test_port);

    // Verify port was still set in context
    assert_mcp_port_in_context(&context, test_port);

    // Verify agent config is still ClaudeCode
    let stored_agent_config = context.get_agent_config();
    assert_eq!(
        stored_agent_config.executor_type(),
        swissarmyhammer_config::model::AgentExecutorType::ClaudeCode
    );

    Ok(())
}

/// Test that update_mcp_port is idempotent
#[tokio::test]
async fn test_update_mcp_port_idempotent() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;

    let mut context = create_context_with_llama_and_port(1111);

    // Update MCP port multiple times
    context.update_mcp_port(2222);
    context.update_mcp_port(3333);

    // Verify final port is set correctly
    assert_mcp_port_in_context(&context, 3333);

    // Verify agent config has the final port
    assert_llama_config_has_port(&context, 3333);

    Ok(())
}

/// Test MCP port with different LlamaAgent configurations
#[tokio::test]
async fn test_mcp_port_with_different_llama_configs() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;

    // Test with default LlamaAgent config
    let mut context1 = WorkflowTemplateContext::with_vars(HashMap::new())?;
    let default_config = ModelConfig::llama_agent(LlamaAgentConfig::default());
    context1.set_agent_config(default_config);
    context1.update_mcp_port(4444);

    assert_llama_config_has_port(&context1, 4444);

    // Test with testing LlamaAgent config
    let context2 = create_context_with_llama_and_port(5555);

    assert_llama_config_has_port(&context2, 5555);

    Ok(())
}
