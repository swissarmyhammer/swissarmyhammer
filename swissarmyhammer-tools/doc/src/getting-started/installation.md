# Installation

This guide will help you install SwissArmyHammer Tools and configure it for use with Claude Code or other MCP clients.

## Requirements

- Rust 1.70 or later
- Modern operating system (macOS, Linux, Windows)
- Optional: mdBook for building documentation

## Installation Methods

### Using Cargo (Recommended)

The easiest way to install SwissArmyHammer Tools is through the main SwissArmyHammer CLI, which includes the tools package:

```bash
cargo install swissarmyhammer
```

This installs the `sah` command, which provides access to all SwissArmyHammer functionality including the MCP server.

### From Source

To build from source:

```bash
# Clone the repository
git clone https://github.com/swissarmyhammer/swissarmyhammer.git
cd swissarmyhammer

# Build the tools package
cd swissarmyhammer-tools
cargo build --release

# The binary will be in target/release/
```

### As a Library

To use SwissArmyHammer Tools as a library in your own Rust project, add this to your `Cargo.toml`:

```toml
[dependencies]
swissarmyhammer-tools = "0.1"
swissarmyhammer-prompts = "0.1"
tokio = { version = "1", features = ["full"] }
```

## Verification

Verify the installation:

```bash
# Check version
sah --version

# Run diagnostics
sah doctor
```

## Claude Code Integration

To use SwissArmyHammer Tools with Claude Code, you need to register it as an MCP server.

### Configuration

Add the MCP server configuration to Claude Code:

```bash
# Add for user scope (recommended)
claude mcp add --scope user sah sah serve

# Or add for a specific project
claude mcp add --scope project sah sah serve
```

### Manual Configuration

Alternatively, you can manually edit the Claude Code MCP configuration file:

**Location**: `~/.config/claude/mcp.json` (Linux/macOS) or `%APPDATA%\claude\mcp.json` (Windows)

```json
{
  "mcpServers": {
    "sah": {
      "command": "sah",
      "args": ["serve"]
    }
  }
}
```

### Custom Working Directory

If you want to specify a custom working directory:

```bash
claude mcp add --scope user sah sah --cwd /path/to/project serve
```

Or in the configuration file:

```json
{
  "mcpServers": {
    "sah": {
      "command": "sah",
      "args": ["--cwd", "/path/to/project", "serve"]
    }
  }
}
```

## HTTP Server Mode

SwissArmyHammer Tools can also run as an HTTP server for integration with other clients:

```bash
# Start HTTP server on default port (3000)
sah serve --http

# Start on custom port
sah serve --http --port 8080
```

## Directory Structure

After installation, SwissArmyHammer uses these directories:

```
~/.swissarmyhammer/          # User-level configuration
├── prompts/                 # Custom prompts
├── workflows/               # Custom workflows
└── config.toml             # Configuration file

./.swissarmyhammer/          # Project-level (in your working directory)
├── issues/                  # Issue tracking
├── memos/                   # Notes and documentation
├── todo/                    # Task tracking
├── workflows/               # Project workflows
└── search.db               # Semantic search index
```

## Shell Completions

Generate shell completions for better command-line experience:

```bash
# Bash
sah completions bash > ~/.local/share/bash-completion/completions/sah

# Zsh
sah completions zsh > ~/.zfunc/_sah

# Fish
sah completions fish > ~/.config/fish/completions/sah.fish

# PowerShell
sah completions powershell > $PROFILE/sah.ps1
```

## Troubleshooting

### Command Not Found

If `sah` is not found after installation:

1. Check that `~/.cargo/bin` is in your PATH
2. Restart your terminal
3. Verify installation: `cargo install --list | grep swissarmyhammer`

### Permission Denied

On Unix systems, ensure the binary is executable:

```bash
chmod +x ~/.cargo/bin/sah
```

### Claude Code Connection Issues

If Claude Code cannot connect to the MCP server:

1. Check the MCP configuration: `claude mcp list`
2. Verify the server starts: `sah serve`
3. Check logs in Claude Code for error messages
4. Ensure no firewall is blocking the connection

### Port Already in Use (HTTP Mode)

If the HTTP server fails to start:

```bash
# Check what's using the port
lsof -i :3000

# Use a different port
sah serve --http --port 8080
```

## Next Steps

Now that you have SwissArmyHammer Tools installed:

- [Quick Start](quick-start.md) - Learn the basics
- [Configuration](configuration.md) - Customize your setup
- [Claude Code Integration](../integration/claude-code.md) - Detailed integration guide
