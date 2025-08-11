# ABORT_000253: Project Setup and MCP Tool Infrastructure

Refer to ./specification/abort.md

## Objective

Set up the foundation for the abort MCP tool following the established MCP tool directory pattern. This step creates the basic structure without implementation to establish the foundation for the file-based abort system.

## Context

The specification calls for replacing the brittle string-based "ABORT ERROR" detection system with a robust MCP tool that uses file-based abort state management. This step establishes the basic tool structure following the noun/verb pattern used by other MCP tools.

## Tasks

### 1. Create Abort Tool Directory Structure

Create the abort tool directory following the established pattern:
```
swissarmyhammer-tools/src/mcp/tools/abort/
├── mod.rs                    # Tool exports and module definition
└── create/                   # The action verb for creating abort state
    ├── mod.rs               # Tool implementation stub
    └── description.md       # Tool description for MCP
```

### 2. Implement Basic Tool Structure

**File: `swissarmyhammer-tools/src/mcp/tools/abort/mod.rs`**
```rust
pub mod create;

pub use create::abort_create_tool;
```

**File: `swissarmyhammer-tools/src/mcp/tools/abort/create/mod.rs`**
```rust
use serde_json::{json, Value};
use std::collections::HashMap;
use crate::mcp::responses::ToolResponse;
use crate::mcp::error_handling::McpError;

pub async fn abort_create_tool(
    _args: HashMap<String, Value>
) -> Result<ToolResponse, McpError> {
    // Implementation placeholder - will be completed in next step
    Ok(ToolResponse::success("Tool structure created".to_string()))
}
```

### 3. Add Tool Description

**File: `swissarmyhammer-tools/src/mcp/tools/abort/create/description.md`**
```markdown
Create an abort file to signal immediate termination of workflows and prompts.

## Parameters

- `reason` (required): String containing the abort reason/message

## Examples

Create an abort signal with reason:
```json
{
  "reason": "User cancelled the destructive operation"
}
```

## Returns

Returns confirmation that the abort file has been created with the specified reason. This will cause any running workflows to terminate immediately when they check for the abort condition.
```

### 4. Register Tool in Module Hierarchy

**Update: `swissarmyhammer-tools/src/mcp/tools/mod.rs`**
Add the abort module to the exports:
```rust
pub mod abort;
// ... existing modules
```

### 5. Add Tool to Registry

**Update: `swissarmyhammer-tools/src/mcp/tool_registry.rs`**
Add abort tool to the registry (exact implementation will depend on current registry structure):
```rust
// Add to tool registration
register_tool("abort", tools::abort::create::abort_create_tool);
```

## Success Criteria

- [ ] Abort tool directory structure created following MCP tool pattern
- [ ] Basic tool module structure in place with stubs
- [ ] Tool description file created with proper documentation
- [ ] Tool registered in module hierarchy
- [ ] Tool registered in MCP tool registry
- [ ] Project compiles without errors
- [ ] Basic tool can be called (even if it only returns placeholder response)

## Testing

- Run `cargo check` to ensure no compilation errors
- Run `cargo test` to ensure no regressions
- Verify tool appears in MCP tool listings

## Notes

- This step only creates the structure and stubs - actual file-based abort functionality will be implemented in the next step
- Follow the established MCP tool patterns exactly to maintain consistency
- The tool will use the name `abort` (not `abort_create`) in the MCP interface
- Ensure proper error handling patterns are followed even in stub implementation

## Next Steps

After completion, proceed to ABORT_000254 for core abort tool implementation with file creation logic.