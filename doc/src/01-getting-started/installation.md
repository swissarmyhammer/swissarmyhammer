# Installation

Install SwissArmyHammer and configure it for use with Claude Code.

## Prerequisites

- **Rust 1.70+** - Required for building from source
- **Claude Code** - For MCP integration (recommended)
- **Git** - For issue management features

## Install from HomeBrew

```bash
brew install swissarmyhammer/tap/swissarmyhammer-cli
```

## Verify Installation

Check that everything is working:
```bash
sah --version
sah doctor
```

The `doctor` command checks your installation and configuration.

## Claude Code Integration

Configure SwissArmyHammer as an MCP server for Claude Code:

```bash
# Add SwissArmyHammer as an MCP server
claude mcp add --scope user sah sah serve

# Verify the connection
claude mcp list
```

Once configured, SwissArmyHammer tools will be available in Claude Code automatically.

## Directory Setup

SwissArmyHammer creates directories as needed, but you can set them up manually:

### User Directory (Optional)
```bash
# Personal prompts and workflows
mkdir -p ~/.swissarmyhammer/prompts
mkdir -p ~/.swissarmyhammer/workflows
```

### Project Directory (Optional)
```bash
# Project-specific prompts and workflows
mkdir -p .swissarmyhammer/prompts
mkdir -p .swissarmyhammer/workflows
```

Built-in prompts and workflows are embedded in the binary and available immediately.

## Shell Completions (Optional)

Add shell completions for better CLI experience:

```bash
# Bash
sah completions bash > ~/.bash_completion.d/sah

# Zsh
sah completions zsh > ~/.zfunc/_sah

# Fish
sah completions fish > ~/.config/fish/completions/sah.fish
```

## Configuration (Optional)

SwissArmyHammer works with sensible defaults. Optionally create `~/.swissarmyhammer/sah.toml`:

```toml
[general]
auto_reload = true

[logging]
level = "info"

[mcp]
timeout_ms = 30000
```

## Quick Test

Test your installation:

```bash
# List built-in prompts
sah prompt list

# Test a simple workflow
sah flow run hello-world

# Check everything is working
sah doctor
```

## Common Issues

### Command not found
If `sah: command not found`, ensure Cargo's bin directory is in your PATH:
```bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

### Build failures
Update Rust and install dependencies:
```bash
rustup update stable
# On Ubuntu/Debian:
sudo apt-get install build-essential pkg-config libssl-dev
```

### MCP connection issues
Verify Claude Code can find the binary:
```bash
which sah
claude mcp restart sah
```

## Next Steps

- [Quick Start](quick-start.md) - Start with full auto coding
- [Zed Editor Integration](../02-editor-integration/zed.md) - Use SwissArmyHammer directly in Zed
