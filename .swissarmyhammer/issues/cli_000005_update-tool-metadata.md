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

## Proposed Solution

After analyzing the McpTool trait in `/Users/wballard/github/sah-cli/swissarmyhammer-tools/src/mcp/tool_registry.rs`, I found that the CLI integration methods already exist with smart default implementations:

1. **`cli_category()`** - Extracts category from tool name prefix (`memo_create` → `"memo"`)
2. **`cli_name()`** - Extracts action from tool name suffix (`memo_create` → `"create"`)  
3. **`cli_about()`** - Uses first line of tool description
4. **`hidden_from_cli()`** - Defaults to `false` (visible)

The default implementations handle most cases correctly. However, I need to:

### Step 1: Verify Current Tool Implementations
- Check if tools already have these methods or if they rely on defaults
- Identify tools that need custom implementations

### Step 2: Update Tools with Custom CLI Metadata (if needed)
- **Internal tools** (`abort_create`, `notify_create`) - Override `hidden_from_cli()` to return `true`
- **File tools** - Ensure `files_*` tools map to `"file"` category (default handles `files` → `file`)
- **Tools with poor naming** - Override methods if default extraction doesn't work

### Step 3: Test CLI Integration
- Verify tools appear correctly in CLI help
- Ensure internal tools are hidden
- Check that categories and names make sense

The good news is the trait design makes this mostly automatic! Most tools should just work with the defaults.

## Implementation Results

✅ **Successfully completed all CLI metadata updates!**

### Changes Made

**1. Memo Tools** - All working with defaults except:
- `memo_get_all_context`: Added custom `cli_name()` → "context" (instead of "get_all_context")

**2. Issue Tools** - All working with defaults except:
- `issue_mark_complete`: Added custom `cli_name()` → "complete" (instead of "mark_complete")
- `issue_all_complete`: Added custom `cli_name()` → "status" (instead of "all_complete")

**3. File Tools** - All working perfectly with defaults:
- The trait already handles `files_*` → `"file"` category mapping

**4. Search Tools** - All working perfectly with defaults:
- `search_index` → category: "search", name: "index"
- `search_query` → category: "search", name: "query"

**5. Web Tools** - Required category overrides:
- `web_search`: Added custom `cli_category()` → "web-search" (instead of "web")
- `web_fetch`: Added custom `cli_category()` → "web-search" (instead of "web")

**6. Shell Tools** - Working perfectly with defaults:
- `shell_execute` → category: "shell", name: "execute"

**7. Internal Tools** - Now hidden from CLI:
- `abort_create`: Added `hidden_from_cli()` → `true`
- `notify_create`: Added `hidden_from_cli()` → `true`

### Verification
- ✅ `cargo build` - Successful compilation
- ✅ `cargo test --package swissarmyhammer-tools` - All 373 tests passed
- ✅ No regressions detected

### CLI Command Structure
The CLI will now support commands like:
```bash
sah memo create --title "My Memo" --content "Content"
sah issue list
sah file read --absolute-path "/path/to/file"  
sah search query --query "search terms"
sah web-search search --query "search terms"
sah shell execute --command "ls -la"
```

Internal tools (`abort_create`, `notify_create`) are properly hidden from CLI exposure.