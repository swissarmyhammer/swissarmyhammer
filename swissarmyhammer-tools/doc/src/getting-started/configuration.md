# Configuration

SwissArmyHammer Tools can be configured through configuration files, environment variables, and command-line options.

## Configuration Files

### Locations

Configuration files are loaded in this order (later files override earlier ones):

1. **Built-in defaults** - Embedded in the binary
2. **User config** - `~/.swissarmyhammer/config.toml`
3. **Project config** - `./.swissarmyhammer/config.toml`

### Configuration Format

Configuration files use TOML format:

```toml
# .swissarmyhammer/config.toml

[server]
# Server configuration
host = "127.0.0.1"
port = 3000
max_connections = 100

[search]
# Semantic search settings
max_results = 50
similarity_threshold = 0.7
chunk_size = 512
chunk_overlap = 128

[issues]
# Issue management settings
default_status = "pending"
auto_branch = true
branch_prefix = "issue/"

[files]
# File operation settings
max_file_size = 10485760  # 10 MB
allowed_extensions = [".rs", ".toml", ".md", ".json", ".yaml"]
respect_gitignore = true

[shell]
# Shell execution settings
timeout = 120  # seconds
max_output_size = 1048576  # 1 MB

[rules]
# Code quality check settings
default_severity = "warning"
max_errors = 100

[memos]
# Memo settings
auto_timestamp = true

[todo]
# Todo settings
auto_cleanup = true
```

## Environment Variables

Environment variables can override configuration settings:

```bash
# Server settings
export SAH_SERVER_HOST=0.0.0.0
export SAH_SERVER_PORT=8080

# Search settings
export SAH_SEARCH_MAX_RESULTS=100

# Working directory
export SAH_WORK_DIR=/path/to/project

# Log level
export RUST_LOG=debug
```

## Command-Line Options

### Server Command

```bash
# Basic server start
sah serve

# HTTP mode
sah serve --http --port 8080

# Custom working directory
sah --cwd /path/to/project serve

# Stdio mode (default)
sah serve --stdio
```

### Global Options

```bash
# Set working directory
sah --cwd /path/to/project <command>

# Set log level
RUST_LOG=debug sah serve

# Show version
sah --version

# Show help
sah --help
```

## MCP Server Configuration

### Claude Code Integration

Configure SwissArmyHammer as an MCP server in Claude Code:

```json
{
  "mcpServers": {
    "sah": {
      "command": "sah",
      "args": ["serve"],
      "env": {
        "RUST_LOG": "info",
        "SAH_SEARCH_MAX_RESULTS": "50"
      }
    }
  }
}
```

### With Custom Working Directory

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

### Multiple Instances

You can run multiple instances for different projects:

```json
{
  "mcpServers": {
    "sah-project-a": {
      "command": "sah",
      "args": ["--cwd", "/path/to/project-a", "serve"]
    },
    "sah-project-b": {
      "command": "sah",
      "args": ["--cwd", "/path/to/project-b", "serve"]
    }
  }
}
```

## Feature-Specific Configuration

### Semantic Search

Configure how semantic search indexes and queries your code:

```toml
[search]
# Maximum number of results to return
max_results = 50

# Minimum similarity score (0.0 to 1.0)
similarity_threshold = 0.7

# Size of code chunks for embedding
chunk_size = 512

# Overlap between chunks
chunk_overlap = 128

# Languages to index
languages = ["rust", "python", "typescript", "javascript"]

# Directories to exclude
exclude_dirs = ["target", "node_modules", ".git"]
```

### Issue Management

Configure issue tracking behavior:

```toml
[issues]
# Default status for new issues
default_status = "pending"

# Automatically create git branches for issues
auto_branch = true

# Prefix for issue branches
branch_prefix = "issue/"

# Issue ID format
id_format = "ISSUE_{category}_{number}"

# Categories
categories = ["bug", "feature", "refactor", "docs"]
```

### File Operations

Configure file operation behavior:

```toml
[files]
# Maximum file size to read (bytes)
max_file_size = 10485760  # 10 MB

# Allowed file extensions
allowed_extensions = [".rs", ".toml", ".md", ".json", ".yaml"]

# Respect .gitignore files
respect_gitignore = true

# Follow symbolic links
follow_symlinks = false

# Maximum search depth
max_depth = 10
```

### Shell Execution

Configure shell command execution:

```toml
[shell]
# Default timeout for commands (seconds)
timeout = 120

# Maximum output size (bytes)
max_output_size = 1048576  # 1 MB

# Allow background processes
allow_background = true

# Shell to use
shell = "/bin/bash"
```

### Code Quality Rules

Configure code quality checking:

```toml
[rules]
# Default severity level to report
default_severity = "warning"

# Maximum number of errors to return
max_errors = 100

# Specific rules to enable
enabled_rules = [
    "no-unwrap",
    "no-panic",
    "proper-error-handling"
]

# Directories to exclude
exclude_dirs = ["tests", "examples"]
```

## Logging Configuration

Control logging output with the `RUST_LOG` environment variable:

```bash
# Error level only
RUST_LOG=error sah serve

# Warning and above
RUST_LOG=warn sah serve

# Info level (default)
RUST_LOG=info sah serve

# Debug level
RUST_LOG=debug sah serve

# Trace level (very verbose)
RUST_LOG=trace sah serve

# Per-module logging
RUST_LOG=swissarmyhammer_tools=debug,swissarmyhammer_search=trace sah serve
```

## Performance Tuning

### Search Performance

```toml
[search]
# Increase for better recall, decrease for faster queries
max_results = 100

# Increase for more context, decrease for faster indexing
chunk_size = 1024

# Adjust based on available memory
cache_size = 1000
```

### Server Performance

```toml
[server]
# Maximum concurrent connections (HTTP mode)
max_connections = 100

# Request timeout (seconds)
timeout = 300

# Enable request compression
compress = true
```

## Security Configuration

### File Access

```toml
[files]
# Restrict file access to specific directories
allowed_paths = ["/path/to/project"]

# Deny access to specific patterns
denied_patterns = ["**/*.key", "**/*.pem", "**/secrets/**"]

# Maximum file size
max_file_size = 10485760
```

### Shell Execution

```toml
[shell]
# Disable shell execution entirely
enabled = false

# Restrict to specific commands
allowed_commands = ["git", "cargo", "npm"]

# Set environment variables
env = { PATH = "/usr/local/bin:/usr/bin" }
```

## Validation

Validate your configuration:

```bash
# Check configuration
sah config validate

# Show effective configuration
sah config show

# Show configuration file locations
sah config list
```

## Next Steps

- [Architecture Overview](../concepts/architecture.md) - Understand the system
- [MCP Tools Reference](../tools/overview.md) - Learn about available tools
- [Troubleshooting](../reference/troubleshooting.md) - Solve common issues
