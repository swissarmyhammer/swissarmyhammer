# Step 2: Implement Flow MCP Tool Structure

Refer to ideas/flow_mcp.md

## Objective

Create the basic structure for the `FlowTool` MCP tool and register it in the tool registry.

## Context

Building on Step 1's types, we now create the actual MCP tool implementation that will handle both workflow discovery and execution.

## Tasks

### 1. Create FlowTool Struct

Create `swissarmyhammer-tools/src/mcp/tools/flow/tool.rs`:

```rust
use crate::mcp::tool_registry::{McpTool, ToolContext, CallToolResult};
use crate::mcp::tools::flow::types::*;
use async_trait::async_trait;

pub struct FlowTool {
    // Will add fields in later steps
}

#[async_trait]
impl McpTool for FlowTool {
    fn name(&self) -> &'static str {
        "flow"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        // Use schema generation from types.rs
        // Will populate with actual workflow names in later steps
        generate_flow_tool_schema(vec!["list".to_string()])
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> Result<CallToolResult, McpError> {
        // Parse request
        let request: FlowToolRequest = serde_json::from_value(
            serde_json::Value::Object(arguments)
        )?;

        // Stub implementation - will fill in later steps
        Err(McpError::internal_error("Not yet implemented"))
    }
}
```

### 2. Create Tool Description

Create `swissarmyhammer-tools/src/mcp/tools/flow/description.md`:

```markdown
# Flow Tool

Execute or list workflows dynamically via MCP.

## Usage

### List Available Workflows

Set `flow_name` to "list" to discover available workflows:

{
  "flow_name": "list",
  "format": "json",
  "verbose": true
}

### Execute Workflow

Set `flow_name` to a workflow name and provide parameters:

{
  "flow_name": "plan",
  "parameters": {
    "plan_filename": "spec.md"
  },
  "interactive": false
}
```

### 3. Register Tool

Update `swissarmyhammer-tools/src/mcp/tools/flow/mod.rs`:

```rust
mod types;
mod tool;

pub use tool::FlowTool;

use crate::mcp::tool_registry::ToolRegistry;

pub fn register_flow_tools(registry: &mut ToolRegistry) {
    registry.register(Box::new(FlowTool {}));
}
```

### 4. Update Tool Registry

Update `swissarmyhammer-tools/src/mcp/tool_registry.rs` to call `register_flow_tools()`.

Update `swissarmyhammer-tools/src/mcp/tools/mod.rs` to add `pub mod flow;`.

## Files to Create/Modify

- `swissarmyhammer-tools/src/mcp/tools/flow/tool.rs` (create)
- `swissarmyhammer-tools/src/mcp/tools/flow/description.md` (create)
- `swissarmyhammer-tools/src/mcp/tools/flow/mod.rs` (update)
- `swissarmyhammer-tools/src/mcp/tools/mod.rs` (update)
- `swissarmyhammer-tools/src/mcp/tool_registry.rs` (update)

## Acceptance Criteria

- [ ] FlowTool struct implements McpTool trait
- [ ] Tool appears in MCP tools/list
- [ ] Tool description loads from description.md
- [ ] Tool schema includes flow_name parameter
- [ ] Registration function works correctly
- [ ] Code compiles without warnings

## Estimated Changes

~200 lines of code
