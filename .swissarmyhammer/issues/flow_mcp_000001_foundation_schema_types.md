# Step 1: Create Flow Tool Schema Types

Refer to ideas/flow_mcp.md

## Objective

Create foundational data structures and schema generation for the flow MCP tool.

## Context

This is the first step in implementing dynamic workflow execution via MCP. We need to define the data structures that will be used by the flow MCP tool for both workflow discovery and execution.

## Tasks

### 1. Create Flow Tool Request Types

Create `swissarmyhammer-tools/src/mcp/tools/flow/types.rs` with:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowToolRequest {
    pub flow_name: String,
    #[serde(default)]
    pub parameters: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub format: Option<String>,  // For list: json, yaml, table
    #[serde(default)]
    pub verbose: bool,
    #[serde(default)]
    pub interactive: bool,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub quiet: bool,
}
```

### 2. Create Workflow Discovery Response Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowListResponse {
    pub workflows: Vec<WorkflowMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMetadata {
    pub name: String,
    pub description: String,
    pub source: String,  // "builtin", "project", "user"
    pub parameters: Vec<WorkflowParameter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub description: String,
    pub required: bool,
}
```

### 3. Add Schema Generation Utilities

Add utility functions to generate JSON schema dynamically:

```rust
pub fn generate_flow_tool_schema(workflow_names: Vec<String>) -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "flow_name": {
                "type": "string",
                "description": "Name of the workflow to execute, or 'list' to show all workflows",
                "enum": workflow_names
            },
            "parameters": {
                "type": "object",
                "description": "Workflow-specific parameters as key-value pairs",
                "additionalProperties": true
            },
            // ... other properties
        },
        "required": ["flow_name"]
    })
}
```

### 4. Add Tests

Create `swissarmyhammer-tools/src/mcp/tools/flow/types_tests.rs`:

- Test schema generation includes "list" in enum
- Test schema generation includes workflow names
- Test request deserialization with various parameter combinations
- Test response serialization

## Files to Create

- `swissarmyhammer-tools/src/mcp/tools/flow/types.rs`
- `swissarmyhammer-tools/src/mcp/tools/flow/types_tests.rs`
- `swissarmyhammer-tools/src/mcp/tools/flow/mod.rs` (initial structure)

## Acceptance Criteria

- [ ] `FlowToolRequest` type correctly deserializes from JSON
- [ ] `WorkflowListResponse` type correctly serializes to JSON
- [ ] Schema generation includes "list" in flow_name enum
- [ ] Schema generation dynamically includes workflow names
- [ ] All tests pass
- [ ] Code compiles without warnings

## Estimated Changes

~150 lines of code
