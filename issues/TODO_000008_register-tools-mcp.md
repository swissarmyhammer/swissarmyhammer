# Register Todo Tools in MCP Tool Registry

Refer to ./specification/todo_tool.md

## Overview
Integrate the todo tools into the MCP server by registering them in the tool registry following established patterns.

## Registration Requirements
The todo tools must be registered in the MCP server to be available for use:
- `todo_create` - Create new todo items
- `todo_show` - Retrieve todo items or get next item
- `todo_mark_complete` - Mark items as completed

## Implementation Tasks
1. Create `register_todo_tools()` function in `todo/mod.rs`:
   - Initialize each tool instance
   - Register with provided registry
   - Follow patterns from memoranda and issues modules

2. Update `src/mcp/tools/mod.rs`:
   - Add `pub mod todo;` declaration
   - Export todo tools for registration

3. Update main tool registry in `src/mcp/tool_registry.rs`:
   - Call `todo::register_todo_tools(registry)` 
   - Ensure tools are available in MCP server

4. Verify tool availability:
   - Tools appear in MCP tool list
   - Tools can be called successfully
   - Tool descriptions are loaded correctly

## Registration Pattern
Following established pattern:
```rust
pub fn register_todo_tools(registry: &mut ToolRegistry) {
    registry.register_tool(Box::new(CreateTodoTool::new()));
    registry.register_tool(Box::new(ShowTodoTool::new()));
    registry.register_tool(Box::new(MarkCompleteTodoTool::new()));
}
```

## Integration Points
- Tool registry system
- MCP server initialization
- Tool description loading
- Error handling integration
- Logging and tracing integration

## Testing Integration
- MCP integration tests for todo tools
- Tool registry verification tests
- End-to-end MCP protocol tests
- CLI integration tests (if applicable)

## Verification Steps
1. Start MCP server
2. List available tools - verify todo tools present
3. Call each todo tool with valid parameters
4. Verify responses and functionality
5. Test error scenarios work correctly

## Success Criteria
- All three todo tools register successfully
- Tools appear in MCP tool listings
- Tools execute correctly via MCP protocol
- Tool descriptions load from markdown files
- Error handling works through MCP layer
- Integration tests pass
- No conflicts with existing tools

## Implementation Notes
- Follow exact patterns from issues and memoranda registration
- Ensure tool names match specification exactly
- Verify description.md files are loaded properly
- Test with MCP client to confirm functionality
- Add appropriate logging for debugging
- Ensure thread safety in registration process

## Dependencies
This issue depends on completion of:
- TODO_000001 (module structure)
- TODO_000003 (create tool)
- TODO_000004 (show tool) 
- TODO_000005 (mark complete tool)