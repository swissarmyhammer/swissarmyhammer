# Set up MCP Tool Infrastructure for Abort Tool

Refer to ./specification/abort.md

## Objective
Create the foundational MCP tool directory structure and registration infrastructure for the new abort tool following the established noun/verb pattern.

## Context
The SwissArmyHammer project uses a clean directory-based MCP tool pattern located in `swissarmyhammer-tools/src/mcp/tools/` with noun/verb organization. We need to create the `abort/` tool directory following this established pattern.

## Tasks

### 1. Create Directory Structure
Following the MCP Tool Directory Pattern memo, create:
```
swissarmyhammer-tools/src/mcp/tools/abort/
├── create/
│   ├── mod.rs         # Tool implementation
│   └── description.md # Tool description
└── mod.rs            # Module exports
```

### 2. Tool Registration
- Add abort tool to the main tools registry in `swissarmyhammer-tools/src/mcp/tools/mod.rs`
- Follow existing patterns from issues, memoranda, outline, and search tools
- Ensure proper module exports and tool registration

### 3. Basic Tool Structure
Create the basic abort tool structure with:
- Parameter validation for required `reason` field
- Tool registration name: `abort_create`
- Basic error handling framework

## Implementation Details

### File Structure
```rust
// tools/abort/mod.rs
pub mod create;

pub use create::AbortCreateTool;
```

### Tool Parameters
```rust
#[derive(Serialize, Deserialize, Debug)]
pub struct AbortCreateParameters {
    pub reason: String,
}
```

## Validation Criteria
- [ ] Directory structure matches established pattern
- [ ] Tool is registered in main tools module
- [ ] Basic parameter validation works
- [ ] Tool can be discovered by MCP protocol
- [ ] Follows existing code patterns and conventions

## Dependencies
- None (foundational setup)

## Follow-up Issues
- ABORT_000260_core-abort-tool-implementation
- ABORT_000261_workflowrun-cleanup-integration