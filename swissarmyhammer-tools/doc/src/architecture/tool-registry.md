# Tool Registry

The tool registry is the central system for managing MCP tools in SwissArmyHammer. It provides dynamic tool registration, discovery, and invocation.

## Overview

The tool registry enables a plugin-style architecture where tools can be added or removed without modifying core server code. Each tool is self-contained and implements a standard interface.

## McpTool Trait

All tools implement the `McpTool` trait:

```rust
#[async_trait]
pub trait McpTool: Send + Sync {
    /// Unique tool identifier
    fn name(&self) -> &str;

    /// Human-readable description
    fn description(&self) -> &str;

    /// JSON schema for parameters
    fn schema(&self) -> serde_json::Value;

    /// Execute the tool
    async fn execute(
        &self,
        params: serde_json::Value,
        context: Arc<ToolContext>,
    ) -> Result<serde_json::Value>;
}
```text

## Registration

### Tool Registration Pattern

Tools are registered by category:

```rust
pub fn register_file_tools(registry: &mut ToolRegistry) {
    registry.register(Box::new(FilesRead));
    registry.register(Box::new(FilesWrite));
    registry.register(Box::new(FilesEdit));
    registry.register(Box::new(FilesGlob));
    registry.register(Box::new(FilesGrep));
}
```text

### Automatic Registration

The server automatically registers all tool categories on initialization:

```rust
impl McpServer {
    pub async fn initialize(&mut self) -> Result<()> {
        register_file_tools(&mut self.registry);
        register_search_tools(&mut self.registry);
        register_issue_tools(&mut self.registry);
        register_memo_tools(&mut self.registry);
        register_todo_tools(&mut self.registry);
        register_git_tools(&mut self.registry);
        register_shell_tools(&mut self.registry);
        register_outline_tools(&mut self.registry);
        register_rules_tools(&mut self.registry);
        register_web_fetch_tools(&mut self.registry);
        register_web_search_tools(&mut self.registry);
        // ... additional categories
    }
}
```text

## Tool Lookup

### By Name

Tools are retrieved by their unique name:

```rust
let tool = registry.get_tool("files_read")?;
```text

### List All Tools

Get all registered tools:

```rust
let tools = registry.list_tools();
for tool in tools {
    println!("{}: {}", tool.name(), tool.description());
}
```text

## Tool Execution

### Invocation Flow

1. Client requests tool execution
2. Registry retrieves tool by name
3. Parameters are validated against schema
4. Tool executes with provided context
5. Result is returned to client

### Context Passing

Each tool receives a `ToolContext` with shared resources:

```rust
pub struct ToolContext {
    pub library: PromptLibrary,
    pub working_directory: PathBuf,
    // ... other shared resources
}
```text

## Tool Organization

### By Category

Tools are organized into logical categories:

- `files_*`: File operations
- `search_*`: Semantic search
- `issue_*`: Issue management
- `memo_*`: Note-taking
- `todo_*`: Task tracking
- `git_*`: Git integration
- `shell_*`: Command execution
- `outline_*`: Code analysis
- `rules_*`: Quality checks
- `web_*`: Web tools
- `flow`: Workflows
- `abort_*`: Control flow

### Naming Convention

Tool names follow a consistent pattern:
- Category prefix (e.g., `files_`, `search_`)
- Action verb (e.g., `read`, `create`, `list`)
- Lowercase with underscores

Examples:
- `files_read`
- `issue_create`
- `search_query`

## Example Tool Implementation

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct FilesReadParams {
    path: String,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct FilesReadResult {
    content: String,
    content_type: String,
    encoding: String,
    lines_read: usize,
    total_lines: usize,
}

pub struct FilesRead;

#[async_trait]
impl McpTool for FilesRead {
    fn name(&self) -> &str {
        "files_read"
    }

    fn description(&self) -> &str {
        "Read file contents with optional offset and limit"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                },
                "offset": {
                    "type": "number",
                    "description": "Starting line number (optional)"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum lines to read (optional)"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        context: Arc<ToolContext>,
    ) -> Result<serde_json::Value> {
        let params: FilesReadParams = serde_json::from_value(params)?;

        // Resolve path relative to working directory
        let path = context.working_directory.join(&params.path);

        // Read file with offset/limit
        let content = read_file_partial(&path, params.offset, params.limit).await?;

        let result = FilesReadResult {
            content: content.text,
            content_type: "text",
            encoding: "utf-8",
            lines_read: content.lines_read,
            total_lines: content.total_lines,
        };

        Ok(serde_json::to_value(result)?)
    }
}
```text

## Tool Categories

### File Tools

Located in `src/mcp/tools/files/`:
- `read/mod.rs` - Read file contents
- `write/mod.rs` - Write files atomically
- `edit/mod.rs` - Edit with string replacement
- `glob/mod.rs` - Pattern matching
- `grep/mod.rs` - Content search

### Search Tools

Located in `src/mcp/tools/search/`:
- `index/mod.rs` - Index files for semantic search
- `query/mod.rs` - Query semantic index

### Issue Tools

Located in `src/mcp/tools/issues/`:
- `create/mod.rs` - Create issues
- `list/mod.rs` - List issues
- `show/mod.rs` - Show issue details
- `update/mod.rs` - Update issues
- `mark_complete/mod.rs` - Complete issues
- `all_complete/mod.rs` - Check completion status

## Next Steps

- **[Component Relationships](components.md)**: How components interact
- **[Features](../features.md)**: Detailed tool documentation
