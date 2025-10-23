# Configuration

SwissArmyHammer Tools provides flexible configuration options through environment variables, configuration files, and command-line arguments.

## Configuration Files

### Location

Configuration files are loaded from these locations in order of precedence:

1. **Project config**: `./.swissarmyhammer/sah.toml`
2. **User config**: `~/.swissarmyhammer/sah.toml`
3. **System config**: `/etc/swissarmyhammer/sah.toml` (Linux/macOS)

Settings in higher-precedence files override those in lower-precedence files.

### Format

Configuration files use TOML format:

```toml
[server]
# Server configuration
port = 3000
stdio = true
http = false

[logging]
# Logging configuration
level = "info"
# file = "/var/log/sah-server.log"

[security]
# Security settings
max_file_size = 10485760  # 10MB
max_search_results = 1000

[performance]
# Performance tuning
max_concurrent_tools = 10
tool_timeout_seconds = 300
```

## Environment Variables

Environment variables override configuration file settings.

### Server Configuration

- `SAH_PORT`: HTTP server port (default: 3000)
- `SAH_HTTP_ENABLED`: Enable HTTP server mode (`true`/`false`)
- `SAH_STDIO_ENABLED`: Enable stdio mode (`true`/`false`, default: `true`)

### Logging

- `SAH_LOG_LEVEL`: Log level (`error`, `warn`, `info`, `debug`, `trace`)
- `SAH_LOG_FILE`: Path to log file (optional)
- `SAH_CLAUDE_SYSTEM_PROMPT_DEBUG`: Enable system prompt debug logging
- `RUST_LOG`: Alternative logging using Rust env_logger format

### Working Directory

- `SAH_CWD`: Working directory for file operations

### Security

- `SAH_MAX_FILE_SIZE`: Maximum file size in bytes (default: 10MB)
- `SAH_MAX_SEARCH_RESULTS`: Maximum search results (default: 1000)

### Performance

- `SAH_MAX_CONCURRENT_TOOLS`: Maximum concurrent tool executions (default: 10)
- `SAH_TOOL_TIMEOUT`: Tool execution timeout in seconds (default: 300)

### Example

```bash
export SAH_LOG_LEVEL=debug
export SAH_PORT=8080
export SAH_MAX_FILE_SIZE=5242880  # 5MB
sah serve --http
```

## Command-Line Arguments

Command-line arguments have the highest precedence.

### Serve Command

```bash
sah serve [OPTIONS]
```

**Options:**

- `--http`: Enable HTTP server mode
- `--port <PORT>`: Set HTTP server port (default: 3000)
- `--stdio`: Enable stdio mode (default: enabled)
- `--cwd <PATH>`: Set working directory

**Examples:**

```bash
# Start in stdio mode (default)
sah serve

# Start HTTP server on port 8080
sah serve --http --port 8080

# Change working directory
sah serve --cwd /path/to/project

# HTTP server with custom working directory
sah serve --http --port 3000 --cwd /home/user/projects/myapp
```

### Global Options

```bash
sah [GLOBAL OPTIONS] <COMMAND>
```

**Global Options:**

- `--cwd <PATH>`: Set working directory (before command)
- `--help`: Show help information
- `--version`: Show version information

## Server Configuration

### Stdio Mode

Stdio mode is designed for integration with desktop applications like Claude Code.

```toml
[server]
stdio = true
```

**Characteristics:**
- Communicates over stdin/stdout
- One request at a time
- No network overhead
- Best for single-user, local use

### HTTP Mode

HTTP mode provides a REST API for the MCP server.

```toml
[server]
http = true
port = 3000
```

**Characteristics:**
- RESTful HTTP API
- Supports concurrent requests
- Network accessible
- Best for web integrations or remote access

**Endpoints:**
- `POST /mcp`: MCP protocol endpoint
- `GET /health`: Health check
- `GET /tools`: List available tools

## Logging Configuration

### Log Levels

Configure logging verbosity:

- **error**: Only error messages
- **warn**: Warnings and errors
- **info**: General information, warnings, and errors (default)
- **debug**: Detailed debugging information
- **trace**: Very detailed trace information

### Log Destinations

**Console** (default):
```toml
[logging]
level = "info"
```

**File**:
```toml
[logging]
level = "info"
file = "/var/log/sah-server.log"
```

**Both console and file**:
```toml
[logging]
level = "info"
file = "/var/log/sah-server.log"
console = true
```

### Structured Logging

SwissArmyHammer uses Rust's `tracing` framework for structured logging.

**Environment variable format**:
```bash
# Enable debug for specific modules
export RUST_LOG="swissarmyhammer_tools=debug,swissarmyhammer_search=trace"

# Filter by target
export RUST_LOG="swissarmyhammer_tools::mcp::tools=debug"
```

## Security Configuration

### File Access Restrictions

```toml
[security]
# Maximum file size that can be read/written (bytes)
max_file_size = 10485760  # 10MB

# Maximum number of search results
max_search_results = 1000

# Allowed file extensions for editing
allowed_extensions = ["rs", "toml", "md", "txt", "json", "yaml"]

# Denied path patterns
denied_patterns = ["**/target/**", "**/.git/**", "**/node_modules/**"]
```

### Shell Command Restrictions

```toml
[security.shell]
# Maximum command execution time (seconds)
max_execution_time = 300

# Allowed commands (whitelist)
allowed_commands = ["cargo", "git", "npm", "rustc"]

# Denied commands (blacklist)
denied_commands = ["rm", "sudo", "chmod"]
```

## Performance Configuration

### Concurrency

```toml
[performance]
# Maximum concurrent tool executions
max_concurrent_tools = 10

# Tool execution timeout (seconds)
tool_timeout_seconds = 300
```

### Caching

```toml
[performance.cache]
# Enable template caching
enable_template_cache = true

# Cache size (number of entries)
template_cache_size = 1000

# Cache TTL (seconds)
template_cache_ttl = 3600
```

### Search Configuration

```toml
[search]
# Search index location
index_path = ".swissarmyhammer/search.db"

# Embedding model
model = "nomic-embed-code"

# Maximum chunks per file
max_chunks_per_file = 100

# Chunk size (characters)
chunk_size = 1000

# Chunk overlap (characters)
chunk_overlap = 200
```

## Integration Configuration

### Claude Code Integration

Configure SwissArmyHammer in Claude Code's MCP settings:

```json
{
  "mcpServers": {
    "sah": {
      "command": "sah",
      "args": ["serve"],
      "env": {
        "SAH_LOG_LEVEL": "info",
        "SAH_MAX_FILE_SIZE": "10485760",
        "SAH_TOOL_TIMEOUT": "300"
      }
    }
  }
}
```

### Custom Client Integration

When integrating with a custom MCP client:

```rust
use swissarmyhammer_tools::McpServer;
use swissarmyhammer_prompts::PromptLibrary;

let library = PromptLibrary::new();

let server = McpServer::new(library, Some(config)).await?;
server.initialize().await?;
```

## Advanced Configuration

### Custom Tool Registry

Register only specific tools:

```rust
use swissarmyhammer_tools::ToolRegistry;

let mut registry = ToolRegistry::new();

// Register only file and search tools
swissarmyhammer_tools::register_file_tools(&mut registry);
swissarmyhammer_tools::register_search_tools(&mut registry);

// Don't register shell or web tools for security
```

### Custom Working Directory

Set a different working directory for different contexts:

```toml
[directories]
# Default working directory
default = "."

# Project-specific directories
[directories.projects]
web_app = "/home/user/projects/web-app"
api_service = "/home/user/projects/api-service"
```

### Issue and Memo Storage

Configure where issues and memos are stored:

```toml
[storage]
# Issue storage directory
issues_dir = ".swissarmyhammer/issues"

# Memo storage directory
memos_dir = ".swissarmyhammer/memoranda"

# User-level storage
user_memos_dir = "~/.swissarmyhammer/memoranda"
```

## Configuration Examples

### Development Setup

```toml
[server]
http = true
port = 3000

[logging]
level = "debug"
console = true

[security]
max_file_size = 52428800  # 50MB for development
```

### Production Setup

```toml
[server]
http = true
port = 8080

[logging]
level = "info"
file = "/var/log/sah/server.log"
console = false

[security]
max_file_size = 10485760  # 10MB
max_search_results = 500

[performance]
max_concurrent_tools = 20
tool_timeout_seconds = 180
```

### CI/CD Integration

```toml
[server]
stdio = true

[logging]
level = "warn"
console = true

[security]
max_file_size = 5242880  # 5MB

[performance]
max_concurrent_tools = 5
tool_timeout_seconds = 60
```

## Environment-Specific Configuration

Use environment variables to switch between configurations:

```bash
# Development
export SAH_ENV=development
export SAH_LOG_LEVEL=debug
sah serve

# Production
export SAH_ENV=production
export SAH_LOG_LEVEL=info
export SAH_LOG_FILE=/var/log/sah-server.log
sah serve --http --port 8080
```

## Validation

Validate your configuration:

```bash
# Check configuration is valid
sah config validate

# Show effective configuration
sah config show
```

## Next Steps

- **[Architecture](architecture.md)**: Understand how configuration affects system behavior
- **[Features](features.md)**: Explore tools and their configuration options
- **[Troubleshooting](troubleshooting.md)**: Debug configuration issues
