# Configuration

SwissArmyHammer provides flexible configuration options to customize behavior, directory locations, and integration settings.

## Quick Start Configuration

For most users, SwissArmyHammer works out of the box with minimal configuration. Here are the most common settings you might want to customize:

### 1. Essential 5-Minute Setup

Create a basic configuration file at `~/.swissarmyhammer/sah.toml`:

```toml
[general]
# Enable automatic reloading when files change (recommended for development)
auto_reload = true

[logging]
# Set to "debug" for troubleshooting, "info" for normal use
level = "info"

[mcp]
# Enable the tools you want to use with Claude Code
enable_tools = ["issues", "memoranda", "search"]

[search]
# Use the code-optimized embedding model
embedding_model = "nomic-embed-code"
```

### 2. Common Use Cases

#### For Individual Developers

```toml
[general]
auto_reload = true

[logging]
level = "info"

[mcp]
enable_tools = ["issues", "memoranda", "search", "outline"]

[issues]
auto_create_branches = true
branch_pattern = "issue/{{name}}"

[search]
# Index common development file types
languages = ["rust", "python", "javascript", "typescript"]
```

#### For Teams

```toml
[directories]
# Add shared team prompts directory
prompt_paths = ["/shared/team-prompts"]
workflow_paths = ["/shared/team-workflows"]

[git]
# Consistent commit messages
commit_template = "{{action}}: {{issue_name}}\n\nCo-authored-by: {{author}}"

[workflow]
# Higher parallel execution for team workflows
max_parallel_actions = 8
```

#### For CI/CD Integration

```toml
[logging]
level = "info"
format = "json"  # Better for log aggregation

[mcp]
# Minimal tools for CI environment
enable_tools = ["search", "outline"]

[security]
# Restrict commands in CI
allowed_commands = ["git", "npm", "cargo"]
allow_network = false

[workflow]
max_workflow_time_ms = 600000  # 10 minutes max
```

### 3. Apply Your Configuration

After creating your config file:

```bash
# Validate the configuration
sah config validate

# Test that everything works
sah doctor

# Apply changes (restart Claude Code if using MCP)
claude mcp restart sah
```

### 4. Configuration Priorities

Settings are applied in this order (later overrides earlier):
1. **Built-in defaults** (always safe)
2. **User config** (`~/.swissarmyhammer/sah.toml`) 
3. **Project config** (`./.swissarmyhammer/sah.toml`)
4. **Environment variables** (`SAH_LOG_LEVEL=debug`)
5. **Command flags** (`sah --debug`)

### 5. Quick Customizations

**Change log level temporarily**:
```bash
SAH_LOG_LEVEL=debug sah doctor
```

**Override MCP timeout**:
```bash
SAH_MCP_TIMEOUT=60000 sah serve
```

**Use custom directory**:
```bash
SAH_HOME="/custom/path" sah doctor
```

For advanced configuration options, see the sections below.

---

## Complete Configuration Reference

## Configuration File

The main configuration file is `sah.toml`, located in:

1. **User config**: `~/.swissarmyhammer/sah.toml` (applies to all projects)
2. **Project config**: `./.swissarmyhammer/sah.toml` (project-specific overrides)

### Example Configuration

```toml
# ~/.swissarmyhammer/sah.toml

[general]
# Default template engine (liquid is the only supported engine)
default_template_engine = "liquid"

# Automatically reload prompts when files change
auto_reload = true

# Default timeout for prompt operations (milliseconds)
default_timeout_ms = 30000

[directories]
# Custom user directory (default: ~/.swissarmyhammer)
user_dir = "~/.swissarmyhammer"

# Additional prompt search paths
prompt_paths = [
    "~/my-custom-prompts",
    "/shared/team-prompts"
]

# Additional workflow search paths  
workflow_paths = [
    "~/my-workflows"
]

[logging]
# Log level: trace, debug, info, warn, error
level = "info"

# Log format: json, compact, pretty
format = "compact"

# Log file location (optional, defaults to stderr)
file = "~/.swissarmyhammer/sah.log"

[mcp]
# Enable specific MCP tools
enable_tools = ["issues", "memoranda", "search", "abort", "outline"]

# MCP request timeout (milliseconds)
timeout_ms = 30000

# Maximum concurrent MCP requests
max_concurrent_requests = 10

[template]
# Custom liquid filters directory
custom_filters_dir = "~/.swissarmyhammer/filters"

# Template compilation cache size
cache_size = 1000

# Allow unsafe template features
allow_unsafe = false

[search]
# Embedding model for semantic search
embedding_model = "nomic-embed-code"

# Vector database location
index_path = "~/.swissarmyhammer/search.db"

# Maximum file size to index (bytes)
max_file_size = 1048576  # 1MB

# Languages to index
languages = ["rust", "python", "javascript", "typescript", "dart"]

[workflow]
# Maximum parallel actions in workflows
max_parallel_actions = 4

# Default workflow timeout (milliseconds)
default_timeout_ms = 300000  # 5 minutes



# Workflow cache directory
cache_dir = "~/.swissarmyhammer/workflow_cache"

[issues]
# Default issue template
default_template = "standard"

# Auto-create git branches for issues
auto_create_branches = true

# Branch name pattern (supports {{name}}, {{id}})
branch_pattern = "issue/{{name}}"

# Auto-commit issue changes
auto_commit = true

[memoranda]
# Full-text search engine: tantivy, simple
search_engine = "tantivy"

# Maximum memo size (bytes)
max_memo_size = 1048576  # 1MB

# Auto-backup interval (hours, 0 to disable)
backup_interval = 24

[git]
# Default commit message template for issues
commit_template = "{{action}}: {{issue_name}}\n\n{{description}}"

# GPG signing for commits
sign_commits = false

# Default branch name for new repositories
default_branch = "main"  # Note: Issue operations use git merge-base, not this setting

[security]
# Allowed shell commands for workflow actions
allowed_commands = [
    "git", "npm", "cargo", "python", "node", "make"
]

# Maximum shell command timeout (milliseconds)
shell_timeout_ms = 60000

# Allow network access in workflows
allow_network = true

# Resource limits
max_memory_mb = 512
max_disk_usage_mb = 1024
```

## Environment Variables

Override configuration with environment variables:

```bash
# General settings
export SAH_HOME="$HOME/.swissarmyhammer"
export SAH_LOG_LEVEL="debug"
export SAH_AUTO_RELOAD="true"

# MCP settings
export SAH_MCP_TIMEOUT="30000"
export SAH_MCP_ENABLE_TOOLS="issues,memoranda,search"

# Search settings
export SAH_SEARCH_MODEL="nomic-embed-code"
export SAH_SEARCH_INDEX="$HOME/.sah-search.db"

# Workflow settings  
export SAH_WORKFLOW_MAX_PARALLEL="4"
export SAH_WORKFLOW_TIMEOUT="300000"

# Security settings
export SAH_SHELL_TIMEOUT="60000"
export SAH_ALLOW_NETWORK="true"
```

## Directory Structure Configuration

### Built-in Directories

These are embedded in the binary and always available:

```
builtin/
├── prompts/          # Pre-installed prompts
└── workflows/        # Pre-installed workflows
```

### User Directories

Configurable via `directories.user_dir`:

```
~/.swissarmyhammer/   # Default user directory
├── prompts/          # Personal prompts
├── workflows/        # Personal workflows
├── memoranda/        # Personal notes
├── issues/           # Global issues
├── search.db         # Search index
├── sah.toml         # Configuration
└── logs/            # Log files
```

### Local Directories

Project-specific, searched in current directory and parents:

```
./.swissarmyhammer/   # Project directory
├── prompts/          # Project prompts
├── workflows/        # Project workflows  
├── memoranda/        # Project notes
├── issues/           # Project issues
└── sah.toml         # Project config
```

## Precedence Rules

Configuration values are resolved in this order (later values override earlier ones):

1. **Built-in defaults**
2. **User configuration** (`~/.swissarmyhammer/sah.toml`)
3. **Project configuration** (`./.swissarmyhammer/sah.toml`)
4. **Environment variables**
5. **Command-line arguments**

## Template Configuration

### Custom Liquid Filters

Create custom filters for templates:

```rust
// ~/.swissarmyhammer/filters/my_filters.rs
use swissarmyhammer::prelude::*;

pub struct ProjectNameFilter;

impl CustomLiquidFilter for ProjectNameFilter {
    fn name(&self) -> &str {
        "project_name"
    }
    
    fn filter(&self, input: &str) -> Result<String> {
        // Extract project name from path
        Ok(std::path::Path::new(input)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string())
    }
}
```

Register in configuration:

```toml
[template]
custom_filters_dir = "~/.swissarmyhammer/filters"
```

### Template Variables

Set global template variables:

```toml
[template.variables]
author = "Your Name"
organization = "Your Company"
default_license = "MIT"
```

Use in prompts:

```markdown
---
title: New Project
---

Creating project for {{author}} at {{organization}}.
License: {{default_license}}
```

## MCP Integration Configuration

### Tool Selection

Enable/disable specific MCP tools:

```toml
[mcp]
enable_tools = [
    "issues",      # Issue management
    "memoranda",   # Note-taking
    "search",      # Semantic search
    "abort",       # Workflow control
    "outline"      # Code outline generation
]
```

### Claude Code Integration

Configure Claude Code MCP settings:

```json
// ~/.config/claude-code/mcp.json
{
  "servers": {
    "sah": {
      "command": "sah",
      "args": ["serve"],
      "env": {
        "SAH_LOG_LEVEL": "info",
        "SAH_MCP_TIMEOUT": "30000",
        "SAH_HOME": "/path/to/custom/sah/dir"
      }
    }
  }
}
```

## Search Configuration

### Embedding Models

Configure the embedding model for semantic search:

```toml
[search]
# Available models:
# - nomic-embed-code (recommended for code)
# - all-MiniLM-L6-v2 (general purpose)
embedding_model = "nomic-embed-code"

# Model cache directory
model_cache_dir = "~/.swissarmyhammer/models"

# Download timeout (milliseconds)
model_download_timeout = 300000
```

### Indexing Options

Control what gets indexed:

```toml
[search]
# File patterns to index
include_patterns = [
    "**/*.rs", "**/*.py", "**/*.js", "**/*.ts",
    "**/*.md", "**/*.txt", "**/*.json"
]

# File patterns to exclude
exclude_patterns = [
    "**/target/**", "**/node_modules/**", 
    "**/.git/**", "**/build/**"
]

# Maximum file size (bytes)
max_file_size = 1048576

# Languages for TreeSitter parsing
languages = ["rust", "python", "javascript", "typescript", "dart"]
```

## Workflow Configuration

### Execution Limits

```toml
[workflow]
# Maximum parallel actions
max_parallel_actions = 4

# Default timeout per action (milliseconds)
action_timeout_ms = 60000

# Maximum workflow runtime (milliseconds)
max_workflow_time_ms = 1800000  # 30 minutes

# Enable detailed execution logging
debug_execution = false
```

### Action Configuration

```toml
[workflow.actions]
# Shell action settings
[workflow.actions.shell]
allowed_commands = ["git", "npm", "cargo", "python"]
timeout_ms = 60000
working_directory = "."

# Prompt action settings
[workflow.actions.prompt]
timeout_ms = 30000
max_retries = 3
```

## Validation

Validate your configuration:

```bash
# Validate configuration file
sah validate --config

# Check all configuration sources
sah doctor --verbose

# Test configuration with specific settings
SAH_LOG_LEVEL=debug sah doctor
```

## Security Considerations

### Safe Defaults

SwissArmyHammer uses secure defaults:

- Limited shell command execution
- Path traversal protection
- Resource usage limits
- Network access controls

### Hardening

For production use, consider:

```toml
[security]
# Restrict allowed commands
allowed_commands = ["git"]

# Disable network access
allow_network = false

# Lower resource limits
max_memory_mb = 256
max_disk_usage_mb = 512

# Enable additional validation
strict_validation = true
```

### File Permissions

Set appropriate permissions:

```bash
# Secure configuration directory
chmod 755 ~/.swissarmyhammer
chmod 600 ~/.swissarmyhammer/sah.toml

# Secure search database
chmod 600 ~/.swissarmyhammer/search.db
```

## Migration

### Upgrading Configuration

When upgrading SwissArmyHammer, check for configuration changes:

```bash
# Check for configuration issues
sah doctor --config

# Validate against new schema
sah validate --config --strict
```

### Backup Configuration

Regular backups of important configuration:

```bash
# Backup entire user directory
tar -czf sah-backup-$(date +%Y%m%d).tar.gz ~/.swissarmyhammer

# Or just configuration
cp ~/.swissarmyhammer/sah.toml ~/.swissarmyhammer/sah.toml.backup
```

This configuration system provides fine-grained control over SwissArmyHammer's behavior while maintaining sensible defaults for common use cases.