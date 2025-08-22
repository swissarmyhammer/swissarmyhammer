# Update Existing MCP Tools with CLI Metadata

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective  
Update all existing MCP tools to implement the new CLI metadata methods, enabling them to participate in dynamic CLI generation.

## Technical Details

### Tool Categories and Naming
Update tools to provide CLI metadata based on their function:

**Memo Tools** (`swissarmyhammer-tools/src/mcp/tools/memoranda/`):
- `memo_create` → category: "memo", name: "create"
- `memo_get` → category: "memo", name: "get"  
- `memo_list` → category: "memo", name: "list"
- `memo_search` → category: "memo", name: "search"
- `memo_update` → category: "memo", name: "update"
- `memo_delete` → category: "memo", name: "delete"
- `memo_get_all_context` → category: "memo", name: "context"

**Issue Tools** (`swissarmyhammer-tools/src/mcp/tools/issues/`):
- `issue_create` → category: "issue", name: "create"
- `issue_show` → category: "issue", name: "show"
- `issue_list` → category: "issue", name: "list"
- `issue_work` → category: "issue", name: "work"
- `issue_merge` → category: "issue", name: "merge"
- `issue_mark_complete` → category: "issue", name: "complete"
- `issue_update` → category: "issue", name: "update"
- `issue_all_complete` → category: "issue", name: "status"

**File Tools** (`swissarmyhammer-tools/src/mcp/tools/files/`):
- `files_read` → category: "file", name: "read"
- `files_write` → category: "file", name: "write"  
- `files_edit` → category: "file", name: "edit"
- `files_glob` → category: "file", name: "glob"
- `files_grep` → category: "file", name: "grep"

**Search Tools**:
- `search_index` → category: "search", name: "index"
- `search_query` → category: "search", name: "query"

**Other Tools**:
- `web_search` → category: "web-search", name: "search"
- `web_fetch` → category: "web-search", name: "fetch" 
- `shell_execute` → category: "shell", name: "execute"
- `abort_create` → hidden from CLI (`hidden_from_cli() { true }`)
- `notify_create` → hidden from CLI (`hidden_from_cli() { true }`)

### Implementation Pattern
For each tool, add CLI metadata methods:

```rust
impl McpTool for CreateMemoTool {
    // Existing methods...
    
    fn cli_category(&self) -> Option<&'static str> {
        Some("memo")
    }
    
    fn cli_name(&self) -> &'static str {
        "create"
    }
    
    fn cli_about(&self) -> Option<&'static str> {
        Some("Create a new memo with title and content")
    }
}
```

### CLI About Text Guidelines
- Keep descriptions concise (one line)
- Focus on what the command does, not implementation details
- Use active voice
- Match existing CLI help text style
- Examples:
  - "Create a new memo with title and content"
  - "List all available issues"
  - "Search file contents using patterns"

## Acceptance Criteria
- [ ] All memo tools updated with CLI metadata
- [ ] All issue tools updated with CLI metadata  
- [ ] All file tools updated with CLI metadata
- [ ] All search tools updated with CLI metadata
- [ ] Other tools categorized appropriately
- [ ] Internal tools marked as hidden from CLI
- [ ] Consistent category and naming scheme
- [ ] Brief, helpful CLI about text for each tool
- [ ] All tools compile and pass existing tests

## Implementation Notes
- Update tools in small batches to avoid conflicts
- Ensure category names match existing CLI structure
- Consider how command names will look in help output
- Internal/system tools should be hidden from CLI
- Focus on user-facing functionality