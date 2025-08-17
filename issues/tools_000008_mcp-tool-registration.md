# MCP Tool Registration and Integration

Refer to /Users/wballard/github/sah-filetools/ideas/tools.md

## Objective
Register all file editing tools with the MCP server and integrate them into the existing tool registry system.

## Tasks
- [ ] Implement `register_files_tools` function following established patterns
- [ ] Update `tool_registry.rs` to include files module registration
- [ ] Create comprehensive tool descriptions for each file tool
- [ ] Implement proper JSON schema validation for all tools
- [ ] Add tools to MCP server initialization
- [ ] Verify tool names follow MCP naming conventions
- [ ] Test tool registration and availability through MCP protocol

## Implementation Details
```rust
// In files/mod.rs
pub fn register_files_tools(registry: &mut ToolRegistry) -> Result<()> {
    registry.register_tool("file_read", Box::new(ReadTool::new()))?;
    registry.register_tool("file_write", Box::new(WriteTool::new()))?;
    registry.register_tool("file_edit", Box::new(EditTool::new()))?;
    registry.register_tool("file_glob", Box::new(GlobTool::new()))?;
    registry.register_tool("file_grep", Box::new(GrepTool::new()))?;
    Ok(())
}

// Update tool_registry.rs to call register_files_tools
```

## Tool Names and Descriptions
- `file_read` - Read file contents with optional offset/limit
- `file_write` - Create new files or overwrite existing ones
- `file_edit` - Perform precise string replacements in files
- `file_glob` - Find files using glob patterns
- `file_grep` - Search file contents using regular expressions

## Integration Requirements
- [ ] Follow established naming conventions (prefix with `file_`)
- [ ] Ensure all tools have comprehensive descriptions
- [ ] Verify JSON schemas are complete and accurate
- [ ] Test MCP protocol communication for each tool
- [ ] Validate error handling across all tools
- [ ] Ensure consistent response formatting

## Testing Requirements
- [ ] Unit tests for tool registration process
- [ ] Integration tests with MCP server
- [ ] Tests for tool discovery through MCP protocol
- [ ] Validation tests for all JSON schemas
- [ ] Error handling tests for registration failures
- [ ] End-to-end tests through MCP client

## Acceptance Criteria
- [ ] All five file tools properly registered with MCP server
- [ ] Tools discoverable through MCP list_tools command
- [ ] JSON schemas validate correctly for all tools
- [ ] Tool descriptions complete and informative
- [ ] Integration tests pass for all registered tools
- [ ] No conflicts with existing tool names
- [ ] Proper error handling for registration failures