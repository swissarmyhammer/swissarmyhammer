# Update MCP Tools with CLI Metadata Implementation

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective

Update all existing MCP tools to implement the new CLI metadata methods, providing proper categorization and CLI-specific help text for dynamic command generation.

## Implementation Tasks

### 1. Update Issues Category Tools

Update all issue-related MCP tools in `swissarmyhammer-tools/src/mcp/tools/issues/`:

#### issues/create/mod.rs
```rust
impl McpTool for IssueCreateTool {
    // ... existing methods ...
    
    fn cli_category(&self) -> Option<&'static str> { Some("issue") }
    fn cli_name(&self) -> &'static str { "create" }
    fn cli_about(&self) -> Option<&'static str> {
        Some("Create a new issue with automatic numbering and git branch integration")
    }
}
```

#### issues/list/mod.rs
```rust
impl McpTool for IssueListTool {
    fn cli_category(&self) -> Option<&'static str> { Some("issue") }
    fn cli_name(&self) -> &'static str { "list" }
    fn cli_about(&self) -> Option<&'static str> {
        Some("List issues with filtering options for active, completed, or all issues")
    }
}
```

#### issues/show/mod.rs
```rust
impl McpTool for IssueShowTool {
    fn cli_category(&self) -> Option<&'static str> { Some("issue") }
    fn cli_name(&self) -> &'static str { "show" }
    fn cli_about(&self) -> Option<&'static str> {
        Some("Display issue details with optional raw content output")
    }
}
```

#### Update remaining issue tools:
- `issues/update/mod.rs` → `cli_name: "update"`
- `issues/work/mod.rs` → `cli_name: "work"`  
- `issues/merge/mod.rs` → `cli_name: "merge"`
- `issues/mark_complete/mod.rs` → `cli_name: "complete"`
- `issues/all_complete/mod.rs` → `cli_name: "status"`

### 2. Update Memoranda Category Tools

Update all memo-related MCP tools in `swissarmyhammer-tools/src/mcp/tools/memoranda/`:

#### memoranda/create/mod.rs
```rust
impl McpTool for MemoCreateTool {
    fn cli_category(&self) -> Option<&'static str> { Some("memo") }
    fn cli_name(&self) -> &'static str { "create" }
    fn cli_about(&self) -> Option<&'static str> {
        Some("Create a new memo with title and markdown content")
    }
}
```

#### memoranda/list/mod.rs
```rust
impl McpTool for MemoListTool {
    fn cli_category(&self) -> Option<&'static str> { Some("memo") }
    fn cli_name(&self) -> &'static str { "list" }
    fn cli_about(&self) -> Option<&'static str> {
        Some("List all memos with metadata and content previews")
    }
}
```

#### Update remaining memo tools:
- `memoranda/get/mod.rs` → `cli_name: "get"`
- `memoranda/update/mod.rs` → `cli_name: "update"`
- `memoranda/delete/mod.rs` → `cli_name: "delete"`
- `memoranda/search/mod.rs` → `cli_name: "search"`
- `memoranda/get_all_context/mod.rs` → `cli_name: "context"`

### 3. Update Files Category Tools

Update all file-related MCP tools in `swissarmyhammer-tools/src/mcp/tools/files/`:

#### files/read/mod.rs
```rust
impl McpTool for FileReadTool {
    fn cli_category(&self) -> Option<&'static str> { Some("file") }
    fn cli_name(&self) -> &'static str { "read" }
    fn cli_about(&self) -> Option<&'static str> {
        Some("Read file contents with optional offset and limit for large files")
    }
}
```

#### files/write/mod.rs
```rust
impl McpTool for FileWriteTool {
    fn cli_category(&self) -> Option<&'static str> { Some("file") }
    fn cli_name(&self) -> &'static str { "write" }
    fn cli_about(&self) -> Option<&'static str> {
        Some("Write content to files with atomic operations and directory creation")
    }
}
```

#### Update remaining file tools:
- `files/edit/mod.rs` → `cli_name: "edit"`
- `files/glob/mod.rs` → `cli_name: "glob"`  
- `files/grep/mod.rs` → `cli_name: "grep"`

### 4. Update Search Category Tools

Update search-related MCP tools in `swissarmyhammer-tools/src/mcp/tools/search/`:

#### search/index/mod.rs
```rust
impl McpTool for SearchIndexTool {
    fn cli_category(&self) -> Option<&'static str> { Some("search") }
    fn cli_name(&self) -> &'static str { "index" }
    fn cli_about(&self) -> Option<&'static str> {
        Some("Index files for semantic search using vector embeddings")
    }
}
```

#### search/query/mod.rs  
```rust
impl McpTool for SearchQueryTool {
    fn cli_category(&self) -> Option<&'static str> { Some("search") }
    fn cli_name(&self) -> &'static str { "query" }
    fn cli_about(&self) -> Option<&'static str> {
        Some("Perform semantic search queries against indexed files")
    }
}
```

### 5. Update Web Search Tools

#### web_search/mod.rs
```rust
impl McpTool for WebSearchTool {
    fn cli_category(&self) -> Option<&'static str> { Some("web-search") }
    fn cli_name(&self) -> &'static str { "search" }
    fn cli_about(&self) -> Option<&'static str> {
        Some("Perform web searches using DuckDuckGo with privacy protection")
    }
}
```

### 6. Update Remaining Tool Categories

#### shell/execute/mod.rs
```rust
impl McpTool for ShellExecuteTool {
    fn cli_category(&self) -> Option<&'static str> { Some("shell") }
    fn cli_name(&self) -> &'static str { "exec" }
    fn cli_about(&self) -> Option<&'static str> {
        Some("Execute shell commands with timeout and output capture")
    }
}
```

#### todo/create/mod.rs, todo/show/mod.rs, todo/mark_complete/mod.rs
```rust
// Mark todo tools as hidden from CLI since they're for internal workflow use
fn hidden_from_cli(&self) -> bool { true }
```

#### outline/generate/mod.rs
```rust
impl McpTool for OutlineGenerateTool {
    fn cli_category(&self) -> Option<&'static str> { Some("outline") }
    fn cli_name(&self) -> &'static str { "generate" }
}
```

#### notify/create/mod.rs, abort/create/mod.rs, web_fetch/mod.rs
```rust
// Mark internal tools as hidden from CLI
fn hidden_from_cli(&self) -> bool { true }
```

### 7. Create CLI Metadata Validation Test

Create `swissarmyhammer-tools/src/mcp/tools/cli_metadata_tests.rs`:

```rust
#[cfg(test)]
mod cli_metadata_tests {
    use crate::mcp::tool_registry::{create_tool_registry, McpTool};
    
    #[tokio::test]
    async fn test_all_visible_tools_have_categories() {
        let registry = create_tool_registry().await;
        let tools = registry.get_all_tools();
        
        for tool in tools {
            if !tool.hidden_from_cli() {
                assert!(
                    tool.cli_category().is_some(),
                    "Tool {} is visible but has no CLI category",
                    tool.name()
                );
            }
        }
    }
    
    #[test]
    fn test_cli_names_follow_conventions() {
        // Test that CLI names follow kebab-case conventions
        // Test that no two tools in same category have same CLI name
    }
    
    #[test]
    fn test_about_text_quality() {
        // Test that cli_about text is concise and helpful
        // Test that it's different from description when provided
    }
}
```

### 8. Update Tool Registration

Verify all tools are properly registered and discoverable:

```rust
// In swissarmyhammer-tools/src/mcp/tool_registry.rs
#[tokio::test]
async fn test_cli_categorization() {
    let registry = create_tool_registry().await;
    
    let categories = registry.get_cli_categories();
    assert!(categories.contains(&"issue".to_string()));
    assert!(categories.contains(&"memo".to_string()));
    assert!(categories.contains(&"file".to_string()));
    assert!(categories.contains(&"search".to_string()));
    
    // Test tool counts per category
    assert_eq!(registry.get_tools_for_category("issue").len(), 8);
    assert_eq!(registry.get_tools_for_category("memo").len(), 6);
    assert_eq!(registry.get_tools_for_category("file").len(), 5);
}
```

## Success Criteria

- [ ] All MCP tools implement CLI metadata methods
- [ ] Tools are properly categorized (issue, memo, file, search, etc.)
- [ ] CLI names follow consistent kebab-case conventions
- [ ] CLI about text is concise and user-friendly  
- [ ] Internal/workflow tools marked as hidden from CLI
- [ ] No naming conflicts within categories
- [ ] Metadata validation tests pass
- [ ] Registry methods return expected tool counts
- [ ] All visible tools have meaningful categories and names

## Architecture Notes

- Systematic update of all existing MCP tools
- Consistent categorization matching current CLI structure
- Clear distinction between user-facing and internal tools
- Foundation for removing redundant CLI command enums
- Maintains backward compatibility with existing MCP functionality