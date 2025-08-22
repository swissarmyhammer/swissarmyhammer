# Extend McpTool Trait with CLI Metadata Methods

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective
Add CLI integration methods to the existing `McpTool` trait to enable dynamic command generation without breaking existing tool implementations.

## Technical Details

### Extend McpTool Trait
Add the following optional methods to `McpTool` trait in `swissarmyhammer-tools/src/mcp/tool_registry.rs`:

```rust
pub trait McpTool {
    // Existing methods...
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn schema(&self) -> serde_json::Value;
    async fn execute(&self, arguments: serde_json::Map<String, serde_json::Value>, context: &ToolContext) -> Result<CallToolResult, McpError>;
    
    // NEW CLI integration methods with default implementations
    fn cli_category(&self) -> Option<&'static str> { None }
    fn cli_name(&self) -> &'static str { self.name() }
    fn cli_about(&self) -> Option<&'static str> { None }  
    fn hidden_from_cli(&self) -> bool { false }
}
```

### Implementation Requirements
- All new methods must have sensible default implementations
- Existing tools continue working without modification
- CLI category follows noun-based grouping (issue, memo, file, search, etc.)
- CLI name defaults to tool name but can be customized
- CLI about text provides brief description for command help

### Examples of Expected Usage
- `memo_create` → category: "memo", name: "create"  
- `issue_work` → category: "issue", name: "work"
- `files_read` → category: "file", name: "read"
- `search_query` → category: "search", name: "query"

## Acceptance Criteria
- [ ] McpTool trait extended with CLI methods
- [ ] All existing tools compile without changes
- [ ] Default implementations provide reasonable values
- [ ] CLI category mapping follows consistent noun-verb pattern
- [ ] Unit tests verify trait extension works correctly

## Implementation Notes
- Start with conservative default implementations
- Focus on backward compatibility
- Pattern should support all existing MCP tools
- Consider how tool names map to CLI command structure