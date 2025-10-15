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

- [x] `FlowToolRequest` type correctly deserializes from JSON
- [x] `WorkflowListResponse` type correctly serializes to JSON
- [x] Schema generation includes "list" in flow_name enum
- [x] Schema generation dynamically includes workflow names
- [x] All tests pass
- [x] Code compiles without warnings

## Estimated Changes

~150 lines of code

## Proposed Solution

I will implement this in the following steps:

### Step 1: Create Module Structure
- Create the `swissarmyhammer-tools/src/mcp/tools/flow/` directory
- Create `mod.rs` with module declarations
- Update `swissarmyhammer-tools/src/mcp/tools/mod.rs` to include the flow module

### Step 2: Implement Core Types
- Create `types.rs` with all request and response types
- Add proper serde attributes for serialization/deserialization
- Add validation helper methods as needed
- Follow existing patterns from notify_types.rs and search_types.rs

### Step 3: Implement Schema Generation
- Create `generate_flow_tool_schema` function
- Ensure "list" is always first in the enum
- Make schema compatible with MCP CLI generation patterns
- Follow the schema structure used in notify/create/mod.rs

### Step 4: Write Tests
- Test FlowToolRequest deserialization with all field combinations
- Test WorkflowListResponse serialization
- Test schema generation with empty and populated workflow lists
- Test that "list" always appears in schema enum
- Use existing test patterns from notify/create/mod.rs

### Step 5: Compile and Verify
- Run `cargo build` to ensure compilation succeeds
- Run `cargo nextest run --failure-output immediate --hide-progress-bar --status-level fail --final-status-level fail` for the flow module tests
- Run `cargo fmt --all` to format the code
- Run `cargo clippy` to check for lints

## Implementation Notes

### Files Created

1. **swissarmyhammer-tools/src/mcp/tools/flow/types.rs** (693 lines)
   - `FlowToolRequest`: Main request type with builder pattern methods
   - `WorkflowListResponse`: Response wrapper for workflow list
   - `WorkflowMetadata`: Workflow information with parameters
   - `WorkflowParameter`: Parameter definition with type information
   - `generate_flow_tool_schema`: Dynamic schema generation function
   - Comprehensive validation methods
   - 25 unit tests covering all functionality

2. **swissarmyhammer-tools/src/mcp/tools/flow/mod.rs** (76 lines)
   - Module documentation explaining the architecture
   - Re-exports of commonly used types
   - Future implementation roadmap

3. **Updated swissarmyhammer-tools/src/mcp/tools/mod.rs**
   - Added `pub mod flow;` to include the new module

### Key Design Decisions

1. **Builder Pattern**: Following the pattern from `NotifyRequest`, all request types include builder methods for ergonomic construction

2. **Validation**: Added `validate()` methods to all request types to catch errors early

3. **Serde Defaults**: Used `#[serde(default)]` extensively to make all optional fields truly optional

4. **Schema Generation**: The `generate_flow_tool_schema` function ensures "list" always appears first in the enum, maintaining consistency

5. **Type Safety**: Used `schemars::JsonSchema` derive for type safety and documentation

### Test Coverage

25 comprehensive tests covering:
- Request construction and builder patterns
- Serialization/deserialization round trips
- Validation logic for empty names and invalid formats
- Schema generation with empty and populated workflow lists
- Verification that "list" always appears first in enum
- Default value handling
- Edge cases with format validation

### Build Results

- ✅ Compilation successful (0 warnings, 0 errors)
- ✅ All 25 tests pass
- ✅ Code formatted with `cargo fmt`
- ✅ No clippy warnings
- ✅ Total: 769 lines of code (implementation + tests + docs)

### Next Steps

This implementation provides the foundation for:
1. Creating the FlowTool that implements McpTool trait
2. Integrating with workflow storage and execution
3. Adding MCP notification support for long-running workflows
4. Generating CLI commands dynamically from workflows