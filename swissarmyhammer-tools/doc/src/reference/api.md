# API Documentation

SwissArmyHammer Tools can be used as a Rust library in your own applications. This reference covers the main public APIs.

## Core Types

### McpServer

The main server type that implements the Model Context Protocol.

```rust
use swissarmyhammer_tools::McpServer;
use swissarmyhammer_prompts::PromptLibrary;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let library = PromptLibrary::new();
    let server = McpServer::new(library, None).await?;
    server.initialize().await?;
    server.run().await?;
    Ok(())
}
```

**Methods:**
- `new(library, config)` - Create new server
- `initialize()` - Register all tools
- `run()` - Start serving requests
- `list_tools()` - Get list of registered tools
- `execute_tool(name, params)` - Execute specific tool

### ToolRegistry

Manages tool registration and lookup.

```rust
use swissarmyhammer_tools::ToolRegistry;

let mut registry = ToolRegistry::new();

// Register tools
swissarmyhammer_tools::register_file_tools(&mut registry);
swissarmyhammer_tools::register_search_tools(&mut registry);

// List tools
let tools = registry.list_tools();
for tool in tools {
    println!("{}: {}", tool.name(), tool.description());
}

// Get specific tool
let tool = registry.get_tool("files_read")?;
```

**Methods:**
- `new()` - Create empty registry
- `register(tool)` - Register a tool
- `get_tool(name)` - Get tool by name
- `list_tools()` - Get all registered tools

### ToolContext

Shared context passed to all tools.

```rust
use swissarmyhammer_tools::ToolContext;
use swissarmyhammer_prompts::PromptLibrary;
use std::path::PathBuf;
use std::sync::Arc;

let library = PromptLibrary::new();
let working_dir = std::env::current_dir()?;

let context = Arc::new(ToolContext::new(
    library,
    working_dir,
)?);
```

**Fields:**
- `library: PromptLibrary` - Access to prompt templates
- `working_directory: PathBuf` - Working directory for file operations

### McpTool Trait

Trait that all tools implement.

```rust
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use anyhow::Result;

#[async_trait]
pub trait McpTool: Send + Sync {
    /// Tool's unique name
    fn name(&self) -> &str;

    /// Human-readable description
    fn description(&self) -> &str;

    /// JSON schema for parameters
    fn schema(&self) -> Value;

    /// Execute the tool
    async fn execute(
        &self,
        params: Value,
        context: Arc<ToolContext>,
    ) -> Result<Value>;
}
```

## Tool Registration Functions

Functions to register tools by category:

```rust
use swissarmyhammer_tools::*;

let mut registry = ToolRegistry::new();

register_file_tools(&mut registry);        // File operations
register_search_tools(&mut registry);      // Semantic search
register_issue_tools(&mut registry);       // Issue management
register_memo_tools(&mut registry);        // Memoranda
register_todo_tools(&mut registry);        // Todo tracking
register_git_tools(&mut registry);         // Git integration
register_shell_tools(&mut registry);       // Shell execution
register_rules_tools(&mut registry);       // Quality checks
register_web_fetch_tools(&mut registry);   // Web fetching
register_web_search_tools(&mut registry);  // Web search
```

## Custom Tool Implementation

### Basic Tool

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use anyhow::Result;

#[derive(Debug, Deserialize)]
struct MyParams {
    input: String,
}

#[derive(Debug, Serialize)]
struct MyResult {
    output: String,
}

pub struct MyTool;

#[async_trait]
impl McpTool for MyTool {
    fn name(&self) -> &str {
        "my_tool"
    }

    fn description(&self) -> &str {
        "My custom tool"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "input": {
                    "type": "string",
                    "description": "Input parameter"
                }
            },
            "required": ["input"]
        })
    }

    async fn execute(
        &self,
        params: Value,
        _context: Arc<ToolContext>,
    ) -> Result<Value> {
        let params: MyParams = serde_json::from_value(params)?;

        let result = MyResult {
            output: format!("Processed: {}", params.input),
        };

        Ok(serde_json::to_value(result)?)
    }
}
```

### Register Custom Tool

```rust
let mut registry = ToolRegistry::new();

// Register standard tools
register_file_tools(&mut registry);

// Register custom tool
registry.register(Box::new(MyTool));

// Use with server
let context = Arc::new(ToolContext::new(library, working_dir)?);
let server = McpServer::with_registry(registry, context).await?;
```

## Server Configuration

### Custom Configuration

```rust
use swissarmyhammer_tools::McpServer;

let config = ServerConfig {
    max_concurrent_tools: 10,
    tool_timeout: Duration::from_secs(300),
    // ... other options
};

let server = McpServer::new(library, Some(config)).await?;
```

### HTTP Server Mode

```rust
let server = McpServer::new(library, None).await?;
server.initialize().await?;

// Start HTTP server on custom port
server.run_http_mode(8080).await?;
```

### Stdio Server Mode

```rust
let server = McpServer::new(library, None).await?;
server.initialize().await?;

// Start stdio mode (default)
server.run_stdio_mode().await?;
```

## Error Types

### McpError

Main error type for MCP operations:

```rust
pub enum McpError {
    ParseError { message: String },
    InvalidRequest { message: String },
    MethodNotFound { method: String },
    InvalidParams { details: String },
    InternalError { message: String, source: Option<Box<dyn Error>> },
}
```

### Tool Errors

Tools return `anyhow::Result`:

```rust
use anyhow::{Context, Result};

async fn execute(&self, ...) -> Result<Value> {
    let data = read_file(path)
        .context("Failed to read file")?;

    Ok(json!({ "data": data }))
}
```

## Testing Utilities

### Test Context

```rust
#[cfg(test)]
use swissarmyhammer_tools::test_utils::*;

#[tokio::test]
async fn test_my_tool() {
    let context = create_test_context().await;

    let tool = MyTool;
    let params = json!({ "input": "test" });

    let result = tool.execute(params, context).await;
    assert!(result.is_ok());
}
```

### Mock File System

```rust
#[tokio::test]
async fn test_with_temp_dir() {
    let temp_dir = create_temp_dir();
    let context = create_test_context_with_dir(&temp_dir).await;

    // Test with temporary directory
}
```

## Integration Examples

### Embedded Server

```rust
use swissarmyhammer_tools::McpServer;
use swissarmyhammer_prompts::PromptLibrary;

pub struct MyApplication {
    mcp_server: McpServer,
}

impl MyApplication {
    pub async fn new() -> Result<Self> {
        let library = PromptLibrary::new();
        let server = McpServer::new(library, None).await?;
        server.initialize().await?;

        Ok(Self { mcp_server: server })
    }

    pub async fn execute_tool(
        &self,
        name: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.mcp_server.execute_tool(name, params).await
    }
}
```

### Custom Tool Set

```rust
let mut registry = ToolRegistry::new();

// Only register specific tools
register_file_tools(&mut registry);
register_search_tools(&mut registry);

// Don't register shell or web tools for security

let context = Arc::new(ToolContext::new(library, working_dir)?);
let server = McpServer::with_registry(registry, context).await?;
```

### Programmatic Execution

```rust
let server = McpServer::new(library, None).await?;
server.initialize().await?;

// Execute tool programmatically
let params = json!({
    "path": "Cargo.toml"
});

let result = server.execute_tool("files_read", params).await?;
println!("Result: {}", result);
```

## API Stability

The public API is considered stable for the current major version. Breaking changes will only occur in major version updates.

**Stable APIs:**
- `McpServer`
- `ToolRegistry`
- `ToolContext`
- `McpTool` trait
- Tool registration functions

**Experimental:**
- Internal implementation details
- Non-public modules

## Next Steps

- **[Tool Catalog](tools.md)** - Complete list of available tools
- **[Contributing](contributing.md)** - Add new tools or features
- **[Rust Documentation](https://docs.rs/swissarmyhammer-tools)** - Full API docs
