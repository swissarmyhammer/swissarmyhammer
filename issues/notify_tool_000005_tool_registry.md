# Integrate Notify Tool with MCP Tool Registry

Refer to /Users/wballard/github/swissarmyhammer/ideas/notify_tool.md

## Objective
Integrate the NotifyTool with the existing MCP tool registry system to make it available for use.

## Tasks
1. Update `swissarmyhammer-tools/src/mcp/tools/mod.rs` to include notify module
2. Create registration function in the notify module
3. Integrate with the tool registry system following existing patterns
4. Ensure proper module exports and visibility

## Integration Requirements

### Module Declaration
Add to `tools/mod.rs`:
```rust
pub mod notify;
```

### Registration Function
Create in `notify/mod.rs`:
```rust
pub fn register_notify_tools(registry: &mut ToolRegistry) -> Result<()> {
    registry.register(Box::new(create::NotifyTool::new()))?;
    Ok(())
}
```

### Tool Registry Integration
Follow the pattern used by other tools (issues, memoranda, etc.) for consistent registration and availability.

## Implementation Notes
- Follow existing patterns from `issues/mod.rs` and `memoranda/mod.rs`
- Ensure proper error handling in registration
- Use consistent naming conventions
- Verify tool is discoverable through MCP protocol

## Verification Steps
1. Confirm tool appears in MCP tool list
2. Verify tool can be called through MCP protocol
3. Check that registration doesn't break existing tools
4. Ensure proper error handling during registration

## Success Criteria
- NotifyTool is properly registered in tool registry
- Tool is discoverable through MCP protocol
- Registration follows established patterns
- No existing functionality is broken
- Tool can be executed through MCP interface

## Dependencies
- Build on logging implementation from step 000004