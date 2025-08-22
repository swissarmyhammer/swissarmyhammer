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

## Proposed Solution

Based on my analysis of the existing codebase, I will extend the `McpTool` trait in `swissarmyhammer-tools/src/mcp/tool_registry.rs` with the following CLI metadata methods:

```rust
pub trait McpTool: Send + Sync {
    // Existing required methods
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn schema(&self) -> serde_json::Value;
    async fn execute(&self, arguments: serde_json::Map<String, serde_json::Value>, context: &ToolContext) -> Result<CallToolResult, McpError>;
    
    // NEW CLI integration methods with default implementations
    fn cli_category(&self) -> Option<&'static str> { 
        // Extract category from tool name by taking prefix before first underscore
        let name = self.name();
        if let Some(underscore_pos) = name.find('_') {
            match &name[..underscore_pos] {
                "memo" => Some("memo"),
                "issue" => Some("issue"), 
                "file" | "files" => Some("file"),
                "search" => Some("search"),
                "web" => Some("web"),
                "shell" => Some("shell"),
                "todo" => Some("todo"),
                "outline" => Some("outline"),
                "notify" => Some("notify"),
                "abort" => Some("abort"),
                _ => None
            }
        } else {
            None
        }
    }
    
    fn cli_name(&self) -> &'static str { 
        // Extract action from tool name by taking suffix after first underscore
        let name = self.name();
        if let Some(underscore_pos) = name.find('_') {
            &name[underscore_pos + 1..]
        } else {
            name
        }
    }
    
    fn cli_about(&self) -> Option<&'static str> { 
        // Use first line of description as brief about text
        let desc = self.description();
        desc.lines().next()
    }
    
    fn hidden_from_cli(&self) -> bool { 
        false 
    }
}
```

### Implementation Strategy

1. **Conservative Default Implementations**: All new methods have sensible defaults based on existing tool properties
2. **Automatic Category Detection**: Extract categories from tool names using the established naming pattern
3. **Backward Compatibility**: All existing tools continue working without modification
4. **Smart CLI Name Extraction**: Automatically extract action names from the tool naming convention

### Category Mapping

The default implementation will map tool prefixes to CLI categories:
- `memo_*` → "memo" category
- `issue_*` → "issue" category  
- `files_*` → "file" category
- `search_*` → "search" category
- `web_*` → "web" category
- etc.

### Testing Approach

1. Verify trait compiles with new methods
2. Ensure all existing tools work unchanged  
3. Test default implementations return correct values
4. Validate category extraction logic
5. Test CLI name extraction logic

This solution maintains full backward compatibility while enabling dynamic CLI command generation.
## Implementation Completed

Successfully extended the `McpTool` trait with CLI integration methods as planned. All acceptance criteria have been met:

### ✅ Implementation Details

1. **Extended McpTool Trait**: Added 4 new methods with default implementations:
   - `cli_category()` - Returns CLI category based on tool name prefix
   - `cli_name()` - Returns CLI command name (action portion)
   - `cli_about()` - Returns brief help text from first line of description
   - `hidden_from_cli()` - Controls CLI visibility (defaults to visible)

2. **Smart Default Implementations**:
   - **Category Detection**: Automatically maps known prefixes (`memo_`, `issue_`, `files_`, etc.) to appropriate CLI categories
   - **Name Extraction**: Extracts action name from `{category}_{action}` pattern
   - **Help Text**: Uses first line of description as brief CLI help text
   - **Backward Compatibility**: All existing tools work without modification

3. **Comprehensive Testing**: Added 13 new unit tests covering:
   - Category extraction for all known tool types
   - CLI name extraction logic
   - Help text extraction from descriptions
   - Edge cases (unknown categories, no underscores, multi-line descriptions)
   - Files/file category aliasing

### ✅ Verification Results

- **Build Success**: `cargo build` completes without errors
- **Lint Clean**: `cargo clippy` produces no warnings
- **All Tests Pass**: 16/16 tests pass including new CLI integration tests
- **Backward Compatibility**: All existing tools compile and function unchanged

### ✅ Category Mapping Implemented

The default implementation maps tool name prefixes to CLI categories:

```rust
"memo" => Some("memo"),
"issue" => Some("issue"), 
"file" | "files" => Some("file"),
"search" => Some("search"),
"web" => Some("web"),
"shell" => Some("shell"),
"todo" => Some("todo"),
"outline" => Some("outline"),
"notify" => Some("notify"),
"abort" => Some("abort"),
_ => None  // Unknown categories return None
```

### ✅ Expected CLI Command Structure

With this implementation, tools will map to CLI commands as:
- `memo_create` → `sah memo create`
- `issue_list` → `sah issue list`
- `files_read` → `sah file read`
- `search_query` → `sah search query`
- etc.

Tools with unknown category prefixes return `None` for `cli_category()` and can be filtered out of CLI command generation.

## Ready for Integration

The trait extension is complete and ready for the next phase of CLI integration. The dynamic command generation system can now query these methods to automatically create CLI commands from MCP tool definitions without requiring manual enum updates.

All original acceptance criteria have been satisfied:
- [x] McpTool trait extended with CLI methods
- [x] All existing tools compile without changes  
- [x] Default implementations provide reasonable values
- [x] CLI category mapping follows consistent noun-verb pattern
- [x] Unit tests verify trait extension works correctly