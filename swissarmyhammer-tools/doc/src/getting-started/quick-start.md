# Quick Start

This guide will get you up and running with SwissArmyHammer Tools in minutes.

## Prerequisites

Make sure you have:
- Installed SwissArmyHammer Tools (see [Installation](installation.md))
- Configured Claude Code integration (optional but recommended)

## Starting the MCP Server

### With Claude Code

If you've configured Claude Code, the server will start automatically when you use Claude. You can verify the integration:

```bash
# List configured MCP servers
claude mcp list

# You should see 'sah' in the list
```

### Standalone Mode

To run the server directly:

```bash
# Start in stdio mode (for MCP clients)
sah serve

# Start in HTTP mode (for HTTP clients)
sah serve --http --port 3000
```

## Your First MCP Tool Call

Once the server is running with Claude Code, you can ask Claude to use the tools. Here are some examples:

### Reading a File

```
Claude, please read the file src/main.rs
```

Claude will use the `files_read` tool to read the file and show you its contents.

### Searching Code

```
Claude, please index all Rust files and then search for "error handling"
```

Claude will:
1. Use `search_index` to index your codebase
2. Use `search_query` to search semantically

### Creating an Issue

```
Claude, please create an issue to add unit tests for the authentication module
```

Claude will use `issue_create` to create a markdown file in `.swissarmyhammer/issues/`.

## Common Operations

### File Operations

```bash
# List all Rust files
Claude, show me all .rs files in the src directory

# Edit a file
Claude, replace the function signature in src/lib.rs

# Search file contents
Claude, search for TODO comments in all files
```

### Semantic Search

```bash
# Index your codebase
Claude, index all source files

# Search by meaning
Claude, find code related to database connections

# Find similar code
Claude, find functions similar to the authenticate() function
```

### Issue Management

```bash
# Create an issue
Claude, create an issue for implementing OAuth support

# List all issues
Claude, show me all open issues

# Update an issue
Claude, update issue-123 with implementation details

# Complete an issue
Claude, mark issue-123 as complete
```

### Code Analysis

```bash
# Generate code outline
Claude, generate an outline of the src directory

# Check code quality
Claude, check the code against our quality rules
```

## Using as a Library

If you're building your own application, here's a complete example:

```rust
use swissarmyhammer_tools::{McpServer, ToolRegistry, ToolContext};
use swissarmyhammer_prompts::PromptLibrary;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the prompt library
    let library = PromptLibrary::new();

    // Create server with custom working directory
    let work_dir = PathBuf::from("/path/to/project");
    let server = McpServer::new_with_work_dir(library, work_dir).await?;

    // Initialize to register all tools
    server.initialize().await?;

    // List available tools
    let tools = server.list_tools();
    println!("Available tools:");
    for tool in tools {
        println!("  - {}: {}", tool.name(), tool.description());
    }

    // Execute a tool
    let params = serde_json::json!({
        "path": "src/main.rs"
    });

    let result = server.execute_tool("files_read", params).await?;
    println!("File contents: {:?}", result);

    Ok(())
}
```

## Project Setup

### Initialize Project Structure

Create the SwissArmyHammer directories in your project:

```bash
mkdir -p .swissarmyhammer/{issues,memos,todo,workflows}
```

### Add to Git

Add these directories to git but ignore generated files:

```bash
# .gitignore
.swissarmyhammer/search.db
.swissarmyhammer/todo.yaml
.swissarmyhammer/.abort
```

Commit the directory structure:

```bash
git add .swissarmyhammer/issues/ .swissarmyhammer/memos/
git commit -m "Add SwissArmyHammer project structure"
```

### Create Your First Memo

Memos are useful for documenting project-specific context:

```bash
# Ask Claude to create a memo
Claude, create a memo titled "Project Coding Standards" with our team's coding conventions
```

This creates a markdown file in `.swissarmyhammer/memos/` that Claude can reference later.

## Workflow Example

Here's a complete workflow for adding a new feature:

1. **Create an issue**
   ```
   Claude, create an issue for adding user authentication
   ```

2. **Index the codebase**
   ```
   Claude, index all source files
   ```

3. **Search for relevant code**
   ```
   Claude, search for existing authentication patterns
   ```

4. **Generate code outline**
   ```
   Claude, generate an outline of the auth module
   ```

5. **Implement the feature**
   ```
   Claude, implement JWT authentication in src/auth.rs
   ```

6. **Check code quality**
   ```
   Claude, check the auth module against our coding rules
   ```

7. **Complete the issue**
   ```
   Claude, mark the authentication issue as complete
   ```

## Configuration

### Working Directory

By default, SwissArmyHammer uses the current directory. To specify a different directory:

```bash
# CLI
sah --cwd /path/to/project serve

# Library
let server = McpServer::new_with_work_dir(library, PathBuf::from("/path/to/project")).await?;
```

### Custom Configuration

Create a configuration file at `.swissarmyhammer/config.toml`:

```toml
[search]
max_results = 50

[issues]
default_status = "pending"

[rules]
severity = "warning"
```

## Next Steps

Now that you're familiar with the basics:

- [Configuration](configuration.md) - Learn about configuration options
- [Architecture Overview](../concepts/architecture.md) - Understand how it works
- [MCP Tools Reference](../tools/overview.md) - Explore all available tools
- [Examples](../examples/basic.md) - See more practical examples
