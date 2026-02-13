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

Initialize SwissArmyHammer for your project:

```bash
sah init
```

This does two things:
1. Registers `sah` as an MCP server in `.mcp.json`
2. Creates the `.swissarmyhammer/` project directory with `prompts/` and `workflows/`

Verify the setup:
```bash
sah doctor
```

### Scope Options

By default, `sah init` writes to project settings. You can choose a different scope:

| Target  | File                             | Use case                           |
|---------|----------------------------------|------------------------------------|
| project | `.mcp.json`                      | Shared with team (default)         |
| local   | `~/.claude.json` (per-project)   | Personal, not committed            |
| user    | `~/.claude.json` (global)        | Applies to all projects            |

```bash
sah init user      # Install globally
sah init local     # Install locally (not committed)
```

### Removing Configuration

```bash
sah deinit                     # Remove from project settings
sah deinit --remove-directory  # Also remove .swissarmyhammer/
sah deinit user                # Remove from user settings
```

### Manual Configuration (Alternative)

If you prefer manual setup:
```bash
claude mcp add --scope user sah sah serve
```

## Directory Setup

`sah init` creates the project directory automatically. For user-level directories, set them up manually:

```bash
# Personal prompts and workflows (optional)
mkdir -p ~/.swissarmyhammer/prompts
mkdir -p ~/.swissarmyhammer/workflows
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
