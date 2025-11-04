# Tool Registry

The Tool Registry is a pluggable system for managing MCP tools, providing registration, validation, and execution routing.

## Overview

The registry maintains a catalog of all available tools and handles:
- Tool registration at startup
- Schema validation
- Tool discovery
- Execution routing
- Metadata management

## Tool Interface

All tools implement the `McpTool` trait:

```rust
pub trait McpTool: Send + Sync {
    /// Unique tool identifier
    fn name(&self) -> &str;

    /// Human-readable description
    fn description(&self) -> &str;

    /// JSON schema for parameters
    fn schema(&self) -> Value;

    /// Execute the tool
    async fn execute(&self, params: Value, context: &ToolContext) -> Result<Value>;
}
```

## Registration

Tools are registered during server initialization:

```rust
let mut registry = ToolRegistry::new();

// Register tool categories
register_file_tools(&mut registry);
register_search_tools(&mut registry);
register_issue_tools(&mut registry);
register_memo_tools(&mut registry);
register_todo_tools(&mut registry);
register_git_tools(&mut registry);
register_shell_tools(&mut registry);
register_outline_tools(&mut registry);
register_rules_tools(&mut registry);
register_web_fetch_tools(&mut registry);
register_web_search_tools(&mut registry);
register_flow_tools(&mut registry);
register_abort_tools(&mut registry);
```

## Schema Validation

Each tool defines a JSON schema describing its parameters:

```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to the file"
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
}
```

The registry validates all parameters before execution.

## Tool Discovery

Clients can query available tools:

```rust
// List all tools
let tools = registry.list_tools();

// Get specific tool
let tool = registry.get_tool("files_read")?;

// Get tool metadata
let name = tool.name();
let description = tool.description();
let schema = tool.schema();
```

## Execution Routing

When a tool is called:

1. **Lookup**: Find tool by name in registry
2. **Validate**: Check parameters against schema
3. **Execute**: Call tool's execute method with context
4. **Handle Errors**: Catch and format errors

```rust
async fn execute_tool(
    &self,
    name: &str,
    params: Value,
    context: &ToolContext
) -> Result<Value> {
    let tool = self.get_tool(name)?;
    tool.execute(params, context).await
}
```

## Tool Context

The `ToolContext` provides tools with access to:

```rust
pub struct ToolContext {
    /// Working directory
    pub work_dir: PathBuf,

    /// Issue storage
    pub issue_storage: Arc<RwLock<Box<dyn IssueStorage>>>,

    /// Memo storage
    pub memo_storage: Arc<RwLock<Box<dyn MemoStorage>>>,

    /// Git operations (optional)
    pub git_ops: Option<GitOperations>,

    /// Configuration
    pub config: Arc<RwLock<Config>>,
}
```

## Tool Categories

### File Tools
Handle filesystem operations with security validation.

### Search Tools
Provide semantic code search using vector embeddings.

### Issue Tools
Manage work items as markdown files.

### Memo Tools
Handle note-taking and knowledge management.

### Todo Tools
Manage ephemeral task tracking.

### Git Tools
Interact with git repositories.

### Shell Tools
Execute shell commands safely.

### Outline Tools
Generate code structure outlines.

### Rules Tools
Check code quality against standards.

### Web Tools
Fetch and search web content.

### Flow Tools
Execute workflows and manage state.

## Adding Custom Tools

To add a new tool:

1. **Implement the trait**:
```rust
pub struct MyTool;

impl McpTool for MyTool {
    fn name(&self) -> &str {
        "my_tool"
    }

    fn description(&self) -> &str {
        "Does something useful"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "param": { "type": "string" }
            },
            "required": ["param"]
        })
    }

    async fn execute(&self, params: Value, context: &ToolContext) -> Result<Value> {
        // Implementation
        Ok(json!({"result": "success"}))
    }
}
```

2. **Register the tool**:
```rust
registry.register(Arc::new(MyTool));
```

## Next Steps

- [Storage Backends](storage-backends.md) - Data persistence layer
- [MCP Tools Reference](../tools/overview.md) - All available tools
- [Custom Tool Development](../integration/custom-tools.md) - Building your own tools
