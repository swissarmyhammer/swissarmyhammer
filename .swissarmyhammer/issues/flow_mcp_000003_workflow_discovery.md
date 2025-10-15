# Step 3: Implement Workflow Discovery (list special case)

Refer to ideas/flow_mcp.md

## Objective

Implement the workflow discovery mechanism that returns metadata when `flow_name="list"`.

## Context

The flow tool needs a special case: when `flow_name="list"`, it returns workflow metadata instead of executing a workflow. This enables MCP clients to discover available workflows.

## BLOCKED: Circular Dependency Issue

**Cannot proceed**: This step requires `swissarmyhammer-tools` to depend on `swissarmyhammer-workflow` to access `WorkflowStorage`, but:
- `swissarmyhammer-workflow` already depends on `swissarmyhammer-tools` (line 30 of workflow/Cargo.toml)
- Adding the reverse dependency creates a circular dependency

**Architectural Solutions Required**:
1. Move `WorkflowStorage` trait and types to `swissarmyhammer-common` (both crates can depend on it)
2. Create new crate `swissarmyhammer-workflow-storage` that both crates depend on
3. Remove `swissarmyhammer-tools` dependency from `swissarmyhammer-workflow`
4. Use dynamic loading or runtime dependency injection

**Related Issues**:
- Check for existing circular dependency issue in the repo
- This blocks steps 3, 4, 8 (all requiring workflow access from tools)

## Tasks (ON HOLD until circular dependency resolved)

### 1. Add Workflow Storage Access

Update `FlowTool` struct to include workflow storage:

```rust
pub struct FlowTool {
    workflow_storage: Arc<dyn WorkflowStorageTrait>,  // Using trait, not concrete type
}

impl FlowTool {
    pub fn new(workflow_storage: Arc<dyn WorkflowStorageTrait>) -> Self {
        Self { workflow_storage }
    }
}
```

### 2. Implement list_workflows Method

Add to `FlowTool` impl:

```rust
async fn list_workflows(
    &self,
    format: Option<String>,
    verbose: bool,
) -> Result<CallToolResult, McpError> {
    let workflows = self.workflow_storage
        .list_workflows()
        .map_err(|e| McpError::internal_error(format!("Failed to list workflows: {}", e)))?;

    let metadata: Vec<WorkflowMetadata> = workflows
        .iter()
        .map(|w| WorkflowMetadata {
            name: w.name.to_string(),
            description: w.description.clone(),
            source: "builtin".to_string(),  // TODO: Determine actual source
            parameters: extract_parameters(w, verbose),
        })
        .collect();

    let response = WorkflowListResponse {
        workflows: metadata,
    };

    // Format response based on format parameter
    let formatted = match format.as_deref() {
        Some("yaml") => serde_yaml::to_string(&response)
            .map_err(|e| McpError::internal_error(format!("YAML formatting failed: {}", e)))?,
        Some("table") => format_as_table(&response)?,
        _ => serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(format!("JSON formatting failed: {}", e)))?,
    };

    Ok(BaseToolImpl::create_success_response(formatted))
}
```

### 3. Update execute Method

Update the execute method to handle the "list" special case:

```rust
async fn execute(
    &self,
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let request: FlowToolRequest = serde_json::from_value(
        serde_json::Value::Object(arguments)
    ).map_err(|e| McpError::invalid_params(format!("Invalid arguments: {}", e)))?;

    // Special case: list workflows
    if request.flow_name == "list" {
        return self.list_workflows(request.format, request.verbose).await;
    }

    // Regular case: execute workflow (stub for now, implemented in step 4)
    Err(McpError::internal_error("Workflow execution not yet implemented"))
}
```

### 4. Add Formatting Utilities

Create helper functions for formatting workflow lists:

```rust
fn extract_parameters(workflow: &Workflow, verbose: bool) -> Vec<WorkflowParameter> {
    if !verbose {
        return vec![];
    }
    
    workflow.parameters.iter().map(|p| WorkflowParameter {
        name: p.name.clone(),
        param_type: format!("{:?}", p.parameter_type),
        description: p.description.clone(),
        required: p.required,
    }).collect()
}

fn format_as_table(response: &WorkflowListResponse) -> Result<String, McpError> {
    use tabled::{Table, Tabled};
    
    #[derive(Tabled)]
    struct WorkflowRow {
        name: String,
        description: String,
        source: String,
    }
    
    let rows: Vec<WorkflowRow> = response.workflows.iter().map(|w| WorkflowRow {
        name: w.name.clone(),
        description: w.description.clone(),
        source: w.source.clone(),
    }).collect();
    
    Ok(Table::new(rows).to_string())
}
```

### 5. Add Tests

Create tests for workflow discovery:

```rust
#[tokio::test]
async fn test_list_workflows_json() {
    // Test listing workflows in JSON format
}

#[tokio::test]
async fn test_list_workflows_verbose() {
    // Test verbose mode includes parameters
}

#[tokio::test]
async fn test_list_special_case() {
    // Test flow_name="list" triggers discovery
}
```

## Files to Modify (ON HOLD)

- `swissarmyhammer-tools/src/mcp/tools/flow/tool.rs`
- `swissarmyhammer-tools/src/mcp/tools/flow/mod.rs`
- `swissarmyhammer-tools/src/mcp/tools/flow/tests.rs` (create)

## Acceptance Criteria (ON HOLD)

- [ ] Circular dependency resolved (prerequisite)
- [ ] `flow_name="list"` returns workflow metadata
- [ ] Response includes workflow names, descriptions, and sources
- [ ] Verbose mode includes parameter information
- [ ] Format parameter supports json, yaml, table
- [ ] Non-verbose mode omits parameter details
- [ ] Tests pass for all discovery scenarios
- [ ] Code compiles without warnings

## Estimated Changes

~180 lines of code (once circular dependency resolved)

## Next Steps

1. Resolve circular dependency (architectural decision needed)
2. Then implement this step
3. OR: Implement stub that returns empty list until circular dependency fixed
