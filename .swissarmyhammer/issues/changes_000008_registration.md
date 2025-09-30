# Step 8: Register Git Changes Tool

Refer to ideas/changes.md

## Objective

Wire up the git_changes tool in the MCP tool registry.

## Tasks

1. Complete `swissarmyhammer-tools/src/mcp/tools/git/mod.rs`
   - Implement `register_git_tools()` function
   - Register GitChangesTool with the registry
   - Add module documentation

2. Update `swissarmyhammer-tools/src/mcp/tool_registry.rs`
   - Import git tools module
   - Call `register_git_tools()` in tool registration
   - Ensure git tools are included in tool list

3. Verify tool is accessible:
   - Tool appears in MCP tool list
   - Tool can be invoked through MCP protocol

## Success Criteria

- Tool is registered and accessible
- Project compiles with `cargo build`
- Tool appears in tool listing
- Registration follows existing pattern

## Files to Modify

- `swissarmyhammer-tools/src/mcp/tools/git/mod.rs`
- `swissarmyhammer-tools/src/mcp/tool_registry.rs`

## Estimated Code Changes

~40 lines