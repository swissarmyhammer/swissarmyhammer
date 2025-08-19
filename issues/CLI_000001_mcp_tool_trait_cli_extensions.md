# Extend McpTool Trait with CLI Metadata Methods

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective

Add CLI integration methods to the existing McpTool trait to support dynamic command generation from MCP tool schemas.

## Implementation Tasks

### 1. Extend McpTool Trait
Update `swissarmyhammer-tools/src/mcp/tool_registry.rs` to add CLI metadata methods:

```rust
pub trait McpTool {
    // Existing methods (unchanged)
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str; 
    fn schema(&self) -> serde_json::Value;
    async fn execute(&self, arguments: serde_json::Map<String, serde_json::Value>, context: &ToolContext) -> Result<CallToolResult, McpError>;
    
    // New CLI integration methods
    fn cli_category(&self) -> Option<&'static str> { None }
    fn cli_name(&self) -> &'static str { self.name() }
    fn cli_about(&self) -> Option<&'static str> { None }
    fn hidden_from_cli(&self) -> bool { false }
}
```

### 2. Method Specifications

**cli_category()**
- Returns the CLI category for grouping tools (e.g., "issue", "memo", "file")
- Used to organize tools into subcommands
- Default None means tool appears at root level

**cli_name()**  
- Returns the CLI command name (defaults to MCP tool name)
- Allows customization of command names for CLI UX
- Should follow kebab-case CLI conventions

**cli_about()**
- Returns CLI-specific help text
- Allows override of description() for CLI context
- Default None uses description()

**hidden_from_cli()**
- Returns true if tool should not appear in CLI
- Useful for MCP-only tools or internal tools
- Default false makes tools visible

### 3. Update Tool Implementations

Update 2-3 sample MCP tools to implement the new methods:

#### issues/create/mod.rs
```rust
impl McpTool for IssueCreateTool {
    // ... existing methods ...
    
    fn cli_category(&self) -> Option<&'static str> { Some("issue") }
    fn cli_name(&self) -> &'static str { "create" }  
    fn cli_about(&self) -> Option<&'static str> { 
        Some("Create a new issue with automatic numbering")
    }
}
```

#### memoranda/list/mod.rs
```rust
impl McpTool for MemoListTool {
    // ... existing methods ...
    
    fn cli_category(&self) -> Option<&'static str> { Some("memo") }
    fn cli_name(&self) -> &'static str { "list" }
    fn cli_about(&self) -> Option<&'static str) {
        Some("List all available memos with metadata")
    }
}
```

### 4. Testing

- Add unit tests for new trait methods
- Verify default implementations work correctly
- Test categorization logic

## Success Criteria

- [ ] McpTool trait extended with 4 new CLI methods
- [ ] Default implementations provided for backward compatibility
- [ ] 2-3 sample tools implement new methods correctly
- [ ] All existing tests pass
- [ ] New trait methods have unit test coverage

## Architecture Notes

- Maintains backward compatibility with existing tools
- Uses default implementations to avoid breaking changes
- Prepares foundation for dynamic CLI generation
- Follows existing trait design patterns in codebase