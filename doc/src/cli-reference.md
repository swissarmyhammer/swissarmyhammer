# Command Line Interface

SwissArmyHammer provides a comprehensive command-line interface for managing prompts, running the MCP server, and integrating with your development workflow.

## Installation

```bash
# Install from Git repository (requires Rust)
cargo install --git https://github.com/swissarmyhammer/swissarmyhammer.git swissarmyhammer-cli

# Ensure ~/.cargo/bin is in your PATH
export PATH="$HOME/.cargo/bin:$PATH"
```

## Basic Usage

```bash
swissarmyhammer [COMMAND] [OPTIONS]
```

## Global Options

- `--help, -h` - Display help information
- `--version, -V` - Display version information

## Commands Overview

| Command | Description |
|---------|-------------|
| [`serve`](./cli-serve.md) | Run as MCP server for Claude Code integration |
| [`doctor`](./cli-doctor.md) | Diagnose configuration and setup issues |
| [`prompt`](./cli-prompt.md) | Manage and test prompts |
| [`flow`](./cli-flow.md) | Execute and manage workflows |
| [`issue`](./cli-issue.md) | Issue management commands |
| [`memo`](./cli-memoranda.md) | Memoranda (memo) management commands |
| [`search`](./cli-search.md) | Semantic search commands |
| [`config`](./cli-config.md) | Configuration management commands |
| [`validate`](./cli-validate.md) | Validate prompt files and workflows |
| [`completion`](./cli-completion.md) | Generate shell completion scripts |

## Quick Examples

### Start MCP Server
```bash
# Run as MCP server (for Claude Code)
swissarmyhammer serve
```

### Prompt Management
```bash
# List all available prompts
swissarmyhammer prompt list

# Test a prompt interactively
swissarmyhammer prompt test code-review

# Search for prompts
swissarmyhammer prompt search "code review"
```

### Issue Management
```bash
# List all issues
swissarmyhammer issue list

# Create a new issue
swissarmyhammer issue create --name "feature-auth" --content "# Authentication Feature\n\nImplement user authentication"

# Start working on an issue
swissarmyhammer issue work feature-auth

# Mark issue as complete
swissarmyhammer issue mark-complete feature-auth
```

### Memoranda (Notes)
```bash
# List all memos
swissarmyhammer memo list

# Create a new memo
swissarmyhammer memo create --title "Meeting Notes" --content "# Team Meeting\n\n- Discussed roadmap"

# Search memos
swissarmyhammer memo search "roadmap"
```

### Semantic Search
```bash
# Index files for semantic search
swissarmyhammer search index "**/*.rs"

# Query the index
swissarmyhammer search query "error handling"
```

### Workflow Execution
```bash
# List available workflows
swissarmyhammer flow list

# Run a workflow
swissarmyhammer flow run deploy-staging
```

### Configuration and Validation
```bash
# Check system configuration
swissarmyhammer doctor

# Validate all prompts and workflows
swissarmyhammer validate

# Generate shell completions
swissarmyhammer completion bash > ~/.bash_completion.d/swissarmyhammer
```

## Exit Codes

- `0` - Success
- `1` - General error
- `2` - Command line usage error
- `3` - Configuration error
- `4` - Prompt not found
- `5` - Template rendering error

## Configuration

SwissArmyHammer looks for content in these directories (in order):

### Prompts and Workflows
1. Built-in prompts (embedded in the binary)
2. User prompts: `~/.swissarmyhammer/prompts/`
3. Local prompts: `./.swissarmyhammer/prompts/` (current directory)
4. User workflows: `~/.swissarmyhammer/workflows/`
5. Local workflows: `./.swissarmyhammer/workflows/` (current directory)

### Issues and Memoranda
- Issues: `~/.swissarmyhammer/issues/` and `./.swissarmyhammer/issues/`
- Memoranda: `~/.swissarmyhammer/memoranda/` and `./.swissarmyhammer/memoranda/`
- Search index: `~/.swissarmyhammer/search.db` and `./.swissarmyhammer/search.db`

### Global Options
- `--verbose, -v` - Enable verbose logging
- `--debug, -d` - Enable debug logging  
- `--quiet, -q` - Suppress all output except errors

For detailed command documentation, see the individual command pages linked in the table above.
