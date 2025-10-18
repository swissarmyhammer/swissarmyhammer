# Getting Started

This guide will help you install, configure, and start using SwissArmyHammer Tools.

## Installation

SwissArmyHammer Tools is distributed as part of the SwissArmyHammer CLI. Install it using Cargo:

```bash
cargo install swissarmyhammer
```

This installs the `sah` command-line tool, which includes the MCP server.

### System Requirements

- **Rust**: Version 1.70 or later
- **Operating System**: macOS, Linux, or Windows
- **Memory**: 512 MB minimum, 2 GB recommended for semantic search
- **Disk Space**: 100 MB for installation, additional space for semantic search indices

### Optional Dependencies

- **mdBook**: For building documentation locally
  ```bash
  cargo install mdbook
  ```

## Configuration

SwissArmyHammer Tools looks for configuration in `sah.yaml` in your project directory or `~/.config/swissarmyhammer/sah.yaml` for global settings.

### Basic Configuration

Create a `sah.yaml` file:

```yaml
# Agent configuration
agent:
  name: "default"
  model: "claude-sonnet-4"
  max_tokens: 100000

# Issue storage location (default: ./.swissarmyhammer/issues)
issues:
  directory: ".swissarmyhammer/issues"

# Memo storage location (default: ./.swissarmyhammer/memos)
memos:
  directory: ".swissarmyhammer/memos"
```

### Environment Variables

You can override settings with environment variables:

- `SWISSARMYHAMMER_MEMOS_DIR`: Custom location for memos
- `SAH_CLI_MODE`: Set to "1" to enable CLI mode
- `RUST_LOG`: Set logging level (e.g., `debug`, `info`, `warn`)

## Starting the MCP Server

### Stdio Mode (Default)

For integration with Claude Desktop and similar tools:

```bash
sah serve
```

This starts the server in stdio mode, communicating over standard input/output.

### HTTP Mode

For HTTP-based integrations:

```bash
sah serve --http --port 8080
```

Access the server at `http://localhost:8080`.

### Working Directory

To set the working directory before starting the server:

```bash
# Change to a specific project directory
sah --cwd /path/to/project serve

# Use relative paths resolved from the specified directory
sah --cwd ~/projects/myapp serve
```

The `--cwd` flag changes the working directory before any initialization occurs, ensuring all relative paths and configurations are resolved from the specified directory. This is useful when:
- Starting the server from a different location than your project root
- Integrating with tools that launch from specific directories
- Running multiple instances in different project directories

## Integrating with Claude Desktop

To use SwissArmyHammer Tools with Claude Desktop:

### Step 1: Locate Configuration File

The Claude Desktop configuration file location varies by operating system:

- **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`
- **Linux**: `~/.config/Claude/claude_desktop_config.json`

### Step 2: Add MCP Server Configuration

Edit the configuration file and add SwissArmyHammer to the `mcpServers` section:

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

If the file already has other MCP servers, add SwissArmyHammer alongside them:

```json
{
  "mcpServers": {
    "existing-server": {
      "command": "existing",
      "args": []
    },
    "swissarmyhammer": {
      "command": "sah",
      "args": ["serve"]
    }
  }
}
```

### Step 3: Set Working Directory (Optional)

If you want SwissArmyHammer to start in a specific project directory:

```json
{
  "mcpServers": {
    "swissarmyhammer": {
      "command": "sah",
      "args": ["--cwd", "/path/to/your/project", "serve"]
    }
  }
}
```

### Step 4: Restart Claude Desktop

Completely quit and restart Claude Desktop for the changes to take effect.

### Step 5: Verify Integration

After restart, you should see SwissArmyHammer tools available. Test by asking Claude:

```
"Index all Rust files for semantic search"
```

If successful, Claude will use the `search_index` tool to index your codebase.

## Basic Usage Examples

### Example 1: Semantic Code Search

```bash
# First, index your codebase
# In Claude: "Index all Rust files for semantic search"

# Then search for relevant code
# In Claude: "Search for authentication logic"
```

### Example 2: Issue Management

```bash
# Create an issue
# In Claude: "Create an issue for implementing user authentication"

# List all issues
# In Claude: "Show me all active issues"

# Mark an issue complete
# In Claude: "Mark the authentication issue as complete"
```

### Example 3: File Operations

```bash
# Read a file
# In Claude: "Show me the contents of src/main.rs"

# Edit a file
# In Claude: "Replace the old_function with new_function in src/main.rs"

# Find files by pattern
# In Claude: "Find all TypeScript test files"
```

## Using as a Rust Library

If you want to embed SwissArmyHammer Tools in your own Rust application:

```rust,ignore
use swissarmyhammer_tools::McpServer;
use swissarmyhammer_prompts::PromptLibrary;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a prompt library
    let library = PromptLibrary::new();

    // Create the MCP server
    let server = McpServer::new(library).await?;

    // Initialize the server (loads prompts)
    server.initialize().await?;

    // List available tools
    let tools = server.list_tools();
    println!("Available tools: {:?}", tools.len());

    // Execute a tool
    let result = server.execute_tool(
        "files_read",
        serde_json::json!({
            "path": "./README.md"
        })
    ).await?;

    println!("Result: {:?}", result);

    Ok(())
}
```

Add this to your `Cargo.toml`:

```toml
[dependencies]
swissarmyhammer-tools = "0.1"
swissarmyhammer-prompts = "0.1"
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

## Verifying Installation

Check that SwissArmyHammer is installed correctly:

```bash
# Check version
sah --version

# List available commands
sah --help

# Test the server
sah serve --help
```

## Common Setup Issues

### Issue: Command not found

If `sah` is not found after installation:

```bash
# Add Cargo bin directory to PATH
export PATH="$HOME/.cargo/bin:$PATH"

# Add to your shell profile for persistence
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc  # or ~/.zshrc

# Or reinstall with verbose output
cargo install swissarmyhammer --verbose

# Verify installation
which sah
sah --version
```

### Issue: Permission denied

On Unix systems, ensure the binary is executable:

```bash
chmod +x ~/.cargo/bin/sah

# Check file permissions
ls -la ~/.cargo/bin/sah
```

### Issue: Port already in use

If the HTTP port is already in use:

```bash
# Use a different port
sah serve --http --port 8081

# Find what is using the port
lsof -i :8080  # macOS/Linux
netstat -ano | findstr :8080  # Windows
```

### Issue: Configuration file not found

If the server cannot find configuration:

```bash
# Check default locations
ls -la ~/.config/swissarmyhammer/sah.yaml
ls -la .swissarmyhammer/sah.yaml

# Create directory if missing
mkdir -p ~/.config/swissarmyhammer

# Copy example configuration
sah config init
```

### Issue: Semantic search index errors

If indexing fails or searches return no results:

```bash
# Remove old index
rm -rf .swissarmyhammer/search.db

# Re-index with force flag
sah search index '**/*.rs' --force

# Check index size
ls -lh .swissarmyhammer/search.db

# Verify indexing worked
sah search query "test query"
```

### Issue: MCP connection failures

If Claude Desktop or other clients cannot connect:

1. Check server logs for errors
2. Verify sah binary is in PATH
3. Restart the client application
4. Test server manually: `sah serve --help`
5. Check client configuration file syntax

### Issue: Slow performance

If operations are slow:

```bash
# Check system resources
top  # or htop on Linux

# Reduce search index size
sah search index 'src/**/*.rs'  # Index only src directory

# Check for large files in .swissarmyhammer
du -sh .swissarmyhammer/*

# Enable debug logging to identify bottlenecks
RUST_LOG=debug sah serve
```

## Next Steps

Now that you have SwissArmyHammer Tools installed and running:

- [Explore Features](./features.md): Learn about available tools and capabilities
- [Architecture Overview](./architecture.md): Understand how the system works
- [Troubleshooting](./troubleshooting.md): Solutions to common problems
