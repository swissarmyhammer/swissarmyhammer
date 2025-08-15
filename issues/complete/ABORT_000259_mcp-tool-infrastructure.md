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

## Proposed Solution

Based on my analysis of the codebase and the existing MCP tool patterns, I will implement the abort tool infrastructure using the established noun/verb directory pattern. This creates the foundation for the file-based abort detection system.

### Implementation Steps

1. **Create Directory Structure**: Following the MCP Tool Directory Pattern memo, establish:
   ```
   swissarmyhammer-tools/src/mcp/tools/abort/
   ├── create/
   │   ├── mod.rs         # Tool implementation  
   │   └── description.md # Tool description
   └── mod.rs            # Module exports
   ```

2. **Implement Basic Abort Tool**: 
   - Parameter validation for required `reason` field
   - Tool name: `abort_create` (following existing naming convention)
   - Basic file creation logic for `.swissarmyhammer/.abort`
   - Error handling framework using existing patterns

3. **Register Tool in Main Registry**: 
   - Add abort module to `swissarmyhammer-tools/src/mcp/tools/mod.rs`
   - Create `register_abort_tools()` function following existing patterns
   - Add registration call in `tool_registry.rs`

4. **Follow Established Patterns**:
   - Use `McpTool` trait implementation
   - Implement `BaseToolImpl` utilities for response handling
   - Use existing error handling patterns from other tools
   - Include comprehensive JSON schema for parameter validation

This foundational infrastructure will enable the subsequent issues to build the complete file-based abort system without disrupting existing functionality.

## Implementation Complete ✅

Successfully implemented the MCP tool infrastructure for the abort tool following all established patterns and conventions.

### Completed Tasks

1. ✅ **Directory Structure Created**: 
   ```
   swissarmyhammer-tools/src/mcp/tools/abort/
   ├── create/
   │   ├── mod.rs         # Tool implementation
   │   └── description.md # Tool description  
   └── mod.rs            # Module exports
   ```

2. ✅ **AbortCreateTool Implementation**:
   - Implements `McpTool` trait with proper async execution
   - Parameter validation for required `reason` field
   - File creation logic for `.swissarmyhammer/.abort`
   - Rate limiting integration
   - Comprehensive error handling using existing patterns

3. ✅ **Tool Registration**:
   - Added `abort` module to `swissarmyhammer-tools/src/mcp/tools/mod.rs`
   - Created `register_abort_tools()` function in `tool_registry.rs`
   - Integrated registration into MCP server initialization
   - Updated module exports in `mcp/mod.rs`

4. ✅ **Comprehensive Testing**:
   - 8 unit tests covering all functionality
   - Schema validation tests
   - File creation and directory management tests  
   - Tool registration verification tests
   - All tests passing with no warnings

5. ✅ **Validation Criteria Met**:
   - ✅ Directory structure matches established pattern
   - ✅ Tool is registered in main tools module
   - ✅ Basic parameter validation works
   - ✅ Tool can be discovered by MCP protocol
   - ✅ Follows existing code patterns and conventions

### Technical Details

- **Tool Name**: `abort_create`
- **Parameters**: `reason` (required string)
- **Behavior**: Creates `.swissarmyhammer/.abort` file with reason text
- **Integration**: Fully integrated with MCP server and registry system
- **Testing**: 8 comprehensive unit tests, all passing

The foundational infrastructure is now complete and ready for the subsequent issues to build upon. The tool follows all established patterns and is fully functional within the MCP ecosystem.