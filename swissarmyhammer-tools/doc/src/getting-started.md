# Getting Started

This guide will help you install and configure SwissArmyHammer Tools to start using the MCP server with AI assistants.

## Prerequisites

- **Rust**: Version 1.70 or later
- **Operating System**: macOS, Linux, or Windows
- **Optional**: mdBook for building documentation locally

## Installation

### From Crates.io

The easiest way to install SwissArmyHammer Tools is through the SwissArmyHammer CLI package:

```bash
cargo install swissarmyhammer
```text

This installs the `sah` command which includes the MCP server functionality.

### From Source

To build from source with the latest changes:

```bash
# Clone the repository
git clone https://github.com/swissarmyhammer/swissarmyhammer
cd swissarmyhammer

# Build and install
cargo install --path swissarmyhammer-cli
```text

### Verify Installation

Check that the installation was successful:

```bash
sah --version
```text

You should see output showing the version number.

## Basic Configuration

SwissArmyHammer Tools works out of the box with sensible defaults, but you can customize behavior through environment variables or configuration files.

### Environment Variables

- `SAH_LOG_LEVEL`: Set logging level (`error`, `warn`, `info`, `debug`, `trace`)
- `RUST_LOG`: Alternative logging configuration using Rust's env_logger format
- `SAH_CLAUDE_SYSTEM_PROMPT_DEBUG`: Enable debug logging for system prompt operations

Example:

```bash
export SAH_LOG_LEVEL=info
```text

### Configuration File

Create a configuration file at `~/.swissarmyhammer/sah.toml`:

```toml
[server]
# Port for HTTP server mode (default: 3000)
port = 3000

# Enable stdio mode (default: true)
stdio = true

[logging]
# Log level (default: "info")
level = "info"

# Log file path (optional)
# file = "/var/log/sah-server.log"
```text

## Running the MCP Server

### Stdio Mode (Recommended for Claude Code)

Start the server in stdio mode, which communicates over standard input/output:

```bash
sah serve
```text

This is the recommended mode for integration with Claude Code and other desktop applications.

### HTTP Server Mode

Start an HTTP server for web-based integrations:

```bash
sah serve --http
```text

By default, this starts the server on `http://localhost:3000`. Use `--port` to customize:

```bash
sah serve --http --port 8080
```text

### Change Working Directory

Set the working directory before starting the server:

```bash
sah --cwd /path/to/project serve
```text

## Integration with Claude Code

To use SwissArmyHammer Tools with Claude Code, add it as an MCP server in your Claude Code configuration.

### Using Claude CLI

The easiest way is to use the Claude CLI:

```bash
claude mcp add --scope user sah sah serve
```text

This adds the SwissArmyHammer MCP server to your user configuration.

### Manual Configuration

Alternatively, edit your Claude Code MCP configuration file manually:

**Location**: `~/.config/claude/mcp_settings.json` (Linux/macOS) or `%APPDATA%\Claude\mcp_settings.json` (Windows)

```json
{
  "mcpServers": {
    "sah": {
      "command": "sah",
      "args": ["serve"],
      "env": {
        "SAH_LOG_LEVEL": "info"
      }
    }
  }
}
```text

### Verify Integration

After adding the server, restart Claude Code and verify the tools are available:

1. Open Claude Code
2. Check that SwissArmyHammer tools appear in the available tools list
3. Try a simple command like asking Claude to list available MCP tools

## Using as a Library

You can also use SwissArmyHammer Tools as a Rust library in your own applications.

### Add Dependency

Add to your `Cargo.toml`:

```toml
[dependencies]
swissarmyhammer-tools = "0.1"
swissarmyhammer-prompts = "0.1"
tokio = { version = "1", features = ["full"] }
```text

### Basic Server Setup

```rust
use swissarmyhammer_tools::McpServer;
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

    // Start serving (stdio mode)
    server.run().await?;

    Ok(())
}
```text

### Custom Tool Registration

```rust
use swissarmyhammer_tools::{ToolRegistry, ToolContext};
use std::sync::Arc;

// Create a custom tool registry
let mut registry = ToolRegistry::new();

// Register specific tool categories
swissarmyhammer_tools::register_file_tools(&mut registry);
swissarmyhammer_tools::register_search_tools(&mut registry);
swissarmyhammer_tools::register_issue_tools(&mut registry);

// Create tool context
let context = Arc::new(ToolContext::new(
    library,
    std::env::current_dir()?,
)?);

// Use the registry
let tool_names: Vec<_> = registry.list_tools()
    .iter()
    .map(|t| t.name())
    .collect();

println!("Registered {} tools: {:?}", tool_names.len(), tool_names);
```text

## Directory Structure

SwissArmyHammer Tools uses a standard directory structure for storing data:

```text
~/.swissarmyhammer/          # User directory
├── memoranda/               # Personal notes
├── issues/                  # Issue tracking
│   ├── active/             # Active issues
│   └── complete/           # Completed issues
├── search.db               # Semantic search index
└── sah.toml                # Configuration

./.swissarmyhammer/          # Project directory (git repository)
├── memoranda/               # Project notes
├── issues/                  # Project issues
└── .abort                  # Workflow abort signal (temporary)
```text

The project directory (`./.swissarmyhammer/`) should be committed to version control, while the user directory contains personal data.

## Verifying Your Setup

Test your installation with these commands:

### List Available Tools

```bash
# Using sah directly (if running HTTP mode)
curl http://localhost:3000/tools

# Or check via Claude Code
# Ask Claude: "What SwissArmyHammer tools are available?"
```text

### Test File Operations

Create a test file and verify tools work:

```bash
# Create a test directory
mkdir -p /tmp/sah-test
cd /tmp/sah-test

# Start the server with this working directory
sah serve
```text

Then in Claude Code:
- "Create a memo with title 'test' and content 'Hello from SwissArmyHammer'"
- "List all memos"
- "Search for 'Hello' in memos"

### Test Semantic Search

Index and search a codebase:

```bash
# Navigate to a code directory
cd ~/projects/my-rust-project

# Start server
sah serve
```text

Then in Claude Code:
- "Index all Rust files for semantic search"
- "Search for error handling code"
- "Show me the outline of src/main.rs"

## Next Steps

- **[Quick Start](quick-start.md)**: Walk through your first tasks with the MCP server
- **[Configuration](configuration.md)**: Advanced configuration options
- **[Architecture](architecture.md)**: Understand how SwissArmyHammer Tools is designed
- **[Features](features.md)**: Explore all available tools and capabilities

## Troubleshooting

### Server Won't Start

If the server fails to start:

1. Check that the port isn't already in use (HTTP mode)
2. Verify Rust and dependencies are installed correctly
3. Check file permissions for `~/.swissarmyhammer/`
4. Review logs with `SAH_LOG_LEVEL=debug sah serve`

### Claude Code Can't Connect

If Claude Code can't connect to the MCP server:

1. Verify the server configuration in `mcp_settings.json`
2. Restart Claude Code after configuration changes
3. Check that the `sah` command is in your PATH
4. Review Claude Code logs for connection errors

### Tools Not Available

If tools aren't showing up:

1. Verify the server initialized successfully
2. Check that tool registration completed without errors
3. Review server logs for registration failures
4. Try restarting both the server and Claude Code

For more detailed troubleshooting, see the [Troubleshooting Guide](troubleshooting.md).
