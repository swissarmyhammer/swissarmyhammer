# Step 4: Implement Workflow Execution

Refer to ideas/flow_mcp.md

## Objective

Implement workflow execution in the flow MCP tool, handling parameter mapping and execution flags.

## Context

With discovery working, we now need to implement actual workflow execution. The tool should map MCP parameters to workflow variables and handle execution options like interactive, dry_run, and quiet.

## BLOCKED: Circular Dependency Issue

**Cannot proceed**: This step requires `swissarmyhammer-tools` to depend on `swissarmyhammer-workflow` to access `WorkflowExecutor`, but:
- `swissarmyhammer-workflow` already depends on `swissarmyhammer-tools`
- Adding the reverse dependency creates a circular dependency

**Blocked by**: Same circular dependency as issue 000003

**Architectural Solutions Required**: See issue 000003

## Tasks (ON HOLD until circular dependency resolved)

### 1. Implement execute_workflow Method

Add to `FlowTool` impl:

```rust
async fn execute_workflow(
    &self,
    flow_name: &str,
    parameters: serde_json::Map<String, serde_json::Value>,
    interactive: bool,
    dry_run: bool,
    quiet: bool,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    // Get workflow definition
    let workflow = self.workflow_storage
        .get_workflow(&WorkflowName::new(flow_name))
        .map_err(|e| McpError::invalid_params(
            format!("Workflow '{}' not found: {}", flow_name, e)
        ))?;

    // Validate required parameters
    validate_required_parameters(&workflow, &parameters)?;

    // Create workflow executor
    let executor = WorkflowExecutor::new();
    
    // Execute workflow
    let result = executor
        .execute(
            workflow,
            parameters,
            interactive,
            dry_run,
            quiet,
        )
        .await
        .map_err(|e| McpError::internal_error(
            format!("Workflow execution failed: {}", e)
        ))?;

    // Format result
    let output = format_workflow_result(&result)?;
    Ok(BaseToolImpl::create_success_response(output))
}
```

### 2. Add Parameter Validation

```rust
fn validate_required_parameters(
    workflow: &Workflow,
    parameters: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), McpError> {
    for param in &workflow.parameters {
        if param.required && !parameters.contains_key(&param.name) {
            return Err(McpError::invalid_params(
                format!("Missing required parameter: {}", param.name)
            ));
        }
    }
    Ok(())
}
```

### 3. Update execute Method

Update to route to execution:

```rust
async fn execute(
    &self,
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let request: FlowToolRequest = serde_json::from_value(
        serde_json::Value::Object(arguments)
    )?;

    // Special case: list workflows
    if request.flow_name == "list" {
        return self.list_workflows(request.format, request.verbose).await;
    }

    // Regular case: execute workflow
    self.execute_workflow(
        &request.flow_name,
        request.parameters,
        request.interactive,
        request.dry_run,
        request.quiet,
        context,
    ).await
}
```

### 4. Add Result Formatting

```rust
fn format_workflow_result(result: &WorkflowResult) -> Result<String, McpError> {
    let output = serde_json::json!({
        "status": "completed",
        "workflow": result.workflow_name,
        "duration_ms": result.duration.as_millis(),
        "states_executed": result.states.len(),
    });
    
    serde_json::to_string_pretty(&output)
        .map_err(|e| McpError::internal_error(format!("Failed to format result: {}", e)))
}
```

### 5. Add Tests

```rust
#[tokio::test]
async fn test_execute_workflow_success() {
    // Test successful workflow execution
}

#[tokio::test]
async fn test_execute_workflow_missing_params() {
    // Test error when required parameters missing
}

#[tokio::test]
async fn test_execute_workflow_interactive_mode() {
    // Test interactive flag is passed through
}

#[tokio::test]
async fn test_execute_workflow_dry_run() {
    // Test dry run mode
}
```

## Files to Modify (ON HOLD)

- `swissarmyhammer-tools/src/mcp/tools/flow/tool.rs`
- `swissarmyhammer-tools/src/mcp/tools/flow/tests.rs`

## Acceptance Criteria (ON HOLD)

- [ ] Circular dependency resolved (prerequisite)
- [ ] Workflow execution works with valid parameters
- [ ] Required parameter validation works
- [ ] Interactive, dry_run, and quiet flags are passed through
- [ ] Error handling for missing workflows
- [ ] Error handling for invalid parameters
- [ ] Result formatting includes execution details
- [ ] All tests pass
- [ ] Code compiles without warnings

## Estimated Changes

~200 lines of code (once circular dependency resolved)

## Next Steps

1. Resolve circular dependency (architectural decision needed)
2. Then implement workflow execution
