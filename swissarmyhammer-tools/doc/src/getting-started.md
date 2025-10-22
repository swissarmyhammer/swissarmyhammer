# Getting Started

This guide will help you install, configure, and start using SwissArmyHammer Tools.

## Installation

SwissArmyHammer Tools is available as part of the SwissArmyHammer CLI. Install it using Cargo:

```bash
cargo install swissarmyhammer
```

This will install the `sah` command-line tool, which includes the MCP server functionality.

### Requirements

- Rust 1.70 or later
- Modern operating system (macOS, Linux, Windows)
- Optional: mdBook for building documentation

## Quick Start

### Starting the MCP Server

The simplest way to start the MCP server is:

```bash
sah serve
```

This starts the server in stdio mode, which is compatible with MCP clients like Claude Desktop.

#### HTTP Server Mode

To start the server in HTTP mode:

```bash
sah serve --http --port 8080
```

This exposes the MCP server over HTTP on port 8080.

#### Custom Working Directory

To change the working directory before starting the server:

```bash
sah --cwd /path/to/project serve
```

## Configuration

### Claude Desktop Integration

To integrate SwissArmyHammer Tools with Claude Desktop, add the following to your Claude Desktop configuration:

**macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`

**Linux**: `~/.config/Claude/claude_desktop_config.json`

**Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "swissarmyhammer": {
      "command": "sah",
      "args": ["serve"]
    }
  }
}
```

### Project-Specific Configuration

SwissArmyHammer stores project-specific data in the `.swissarmyhammer` directory:

- `.swissarmyhammer/issues/` - Issue tracking files
- `.swissarmyhammer/memos/` - Memo storage
- `.swissarmyhammer/search.db` - Semantic search index
- `.swissarmyhammer/todo.yaml` - Ephemeral task tracking

Add `.swissarmyhammer/search.db` to your `.gitignore`, but commit the `issues/` and `memos/` directories as they contain important project metadata.

## Using as a Library

You can also use SwissArmyHammer Tools as a library in your Rust projects:

```toml
[dependencies]
swissarmyhammer-tools = "0.1"
swissarmyhammer-prompts = "0.1"
```

Example usage:

```rust
use swissarmyhammer_tools::{McpServer, ToolRegistry, ToolContext};
use swissarmyhammer_prompts::PromptLibrary;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the prompt library
    let library = PromptLibrary::new();
    
    // Create the MCP server
    let server = McpServer::new(library, None).await?;
    
    // Initialize to register all tools
    server.initialize().await?;
    
    // List available tools
    let tools = server.list_tools();
    println!("Available tools: {}", tools.len());
    
    Ok(())
}
```

## Verifying Installation

To verify that SwissArmyHammer Tools is installed correctly:

```bash
# Check version
sah --version

# List available commands
sah --help

# Test the server
sah serve --help
```

## Next Steps

- **[Installation Details](./installation.md)**: Learn about installation options and troubleshooting
- **[Quick Start Guide](./quick-start.md)**: Step-by-step tutorial for your first tasks
- **[Configuration](./configuration.md)**: Detailed configuration options
- **[Features](./features.md)**: Explore the available tools and capabilities
