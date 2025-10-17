# Configuration Reference

SwissArmyHammer Tools can be configured through YAML configuration files, environment variables, and command-line arguments. This document provides a complete reference for all configuration options.

## Configuration File Locations

Configuration files are loaded in priority order:

1. **Project-local configuration**: `./sah.yaml` (highest priority)
2. **Global configuration**: `~/.config/swissarmyhammer/sah.yaml`
3. **Default values** (lowest priority)

Environment variables override configuration file values.

## Complete Configuration Example

```yaml
# SwissArmyHammer Tools Configuration

# Agent configuration for AI execution
agent:
  name: "default"
  model: "claude-sonnet-4"
  max_tokens: 100000
  temperature: 0.7
  timeout: 300

# Issue storage configuration
issues:
  directory: ".swissarmyhammer/issues"
  complete_directory: ".swissarmyhammer/issues/complete"
  auto_commit: false

# Memo storage configuration
memos:
  directory: ".swissarmyhammer/memos"
  auto_commit: false

# Workflow storage configuration
workflows:
  directory: ".swissarmyhammer/workflows"

# Search configuration
search:
  database: ".swissarmyhammer/search.db"
  max_results: 1000
  chunk_size: 500

# File operation limits
files:
  max_size: 104857600  # 100 MB
  max_line_length: 2000

# Shell execution configuration
shell:
  default_timeout: 120000  # 2 minutes in milliseconds
  max_timeout: 600000      # 10 minutes in milliseconds

# Server configuration
server:
  mode: "stdio"  # or "http"
  host: "127.0.0.1"
  port: 8080
  cors_origins:
    - "http://localhost:3000"

# Logging configuration
logging:
  level: "info"  # error, warn, info, debug, trace
  format: "text"  # text or json
  file: null  # Optional log file path
```

## Configuration Sections

### Agent Configuration

Controls AI agent executor behavior:

```yaml
agent:
  name: "default"           # Agent name identifier
  model: "claude-sonnet-4"  # Model to use
  max_tokens: 100000        # Maximum tokens per request
  temperature: 0.7          # Response randomness (0.0-1.0)
  timeout: 300              # Request timeout in seconds
```

#### agent.name

- **Type**: String
- **Default**: `"default"`
- **Description**: Identifier for the agent configuration

#### agent.model

- **Type**: String
- **Default**: `"claude-sonnet-4"`
- **Description**: AI model to use for agent execution
- **Valid values**: Any supported Claude model name

#### agent.max_tokens

- **Type**: Integer
- **Default**: `100000`
- **Description**: Maximum number of tokens in a single request
- **Range**: `1` to model's maximum context size

#### agent.temperature

- **Type**: Float
- **Default**: `0.7`
- **Description**: Controls randomness in responses
- **Range**: `0.0` (deterministic) to `1.0` (creative)

#### agent.timeout

- **Type**: Integer
- **Default**: `300`
- **Description**: Request timeout in seconds
- **Range**: `1` to `3600`

### Issue Storage Configuration

Controls issue tracking behavior:

```yaml
issues:
  directory: ".swissarmyhammer/issues"
  complete_directory: ".swissarmyhammer/issues/complete"
  auto_commit: false
```

#### issues.directory

- **Type**: Path
- **Default**: `.swissarmyhammer/issues`
- **Description**: Directory for active issues
- **Environment**: `SWISSARMYHAMMER_ISSUES_DIR`

#### issues.complete_directory

- **Type**: Path
- **Default**: `.swissarmyhammer/issues/complete`
- **Description**: Directory for completed issues

#### issues.auto_commit

- **Type**: Boolean
- **Default**: `false`
- **Description**: Automatically commit issue changes to git

### Memo Storage Configuration

Controls memo system behavior:

```yaml
memos:
  directory: ".swissarmyhammer/memos"
  auto_commit: false
```

#### memos.directory

- **Type**: Path
- **Default**: `.swissarmyhammer/memos`
- **Description**: Directory for memo storage
- **Environment**: `SWISSARMYHAMMER_MEMOS_DIR`

#### memos.auto_commit

- **Type**: Boolean
- **Default**: `false`
- **Description**: Automatically commit memo changes to git

### Workflow Configuration

Controls workflow execution:

```yaml
workflows:
  directory: ".swissarmyhammer/workflows"
```

#### workflows.directory

- **Type**: Path
- **Default**: `.swissarmyhammer/workflows`
- **Description**: Directory for workflow definitions

### Search Configuration

Controls semantic search behavior:

```yaml
search:
  database: ".swissarmyhammer/search.db"
  max_results: 1000
  chunk_size: 500
```

#### search.database

- **Type**: Path
- **Default**: `.swissarmyhammer/search.db`
- **Description**: SQLite database for search index

#### search.max_results

- **Type**: Integer
- **Default**: `1000`
- **Description**: Maximum number of search results to return
- **Range**: `1` to `10000`

#### search.chunk_size

- **Type**: Integer
- **Default**: `500`
- **Description**: Size of code chunks for semantic search (in tokens)
- **Range**: `100` to `2000`

### File Operation Configuration

Controls file operation limits:

```yaml
files:
  max_size: 104857600
  max_line_length: 2000
```

#### files.max_size

- **Type**: Integer
- **Default**: `104857600` (100 MB)
- **Description**: Maximum file size for read operations in bytes
- **Range**: `1` to `1073741824` (1 GB)

#### files.max_line_length

- **Type**: Integer
- **Default**: `2000`
- **Description**: Maximum line length in characters (lines are truncated)
- **Range**: `80` to `10000`

### Shell Configuration

Controls shell command execution:

```yaml
shell:
  default_timeout: 120000
  max_timeout: 600000
```

#### shell.default_timeout

- **Type**: Integer
- **Default**: `120000` (2 minutes)
- **Description**: Default timeout for shell commands in milliseconds
- **Range**: `1000` to `max_timeout`

#### shell.max_timeout

- **Type**: Integer
- **Default**: `600000` (10 minutes)
- **Description**: Maximum allowed timeout in milliseconds
- **Range**: `1000` to `3600000` (1 hour)

### Server Configuration

Controls MCP server behavior:

```yaml
server:
  mode: "stdio"
  host: "127.0.0.1"
  port: 8080
  cors_origins:
    - "http://localhost:3000"
```

#### server.mode

- **Type**: String
- **Default**: `"stdio"`
- **Description**: Server transport mode
- **Valid values**: `"stdio"`, `"http"`

#### server.host

- **Type**: String
- **Default**: `"127.0.0.1"`
- **Description**: Host address for HTTP mode
- **Valid values**: Any valid IP address or hostname

#### server.port

- **Type**: Integer
- **Default**: `8080`
- **Description**: Port for HTTP mode
- **Range**: `1024` to `65535`

#### server.cors_origins

- **Type**: Array of Strings
- **Default**: `[]`
- **Description**: Allowed CORS origins for HTTP mode
- **Example**: `["http://localhost:3000", "https://example.com"]`

### Logging Configuration

Controls logging behavior:

```yaml
logging:
  level: "info"
  format: "text"
  file: null
```

#### logging.level

- **Type**: String
- **Default**: `"info"`
- **Description**: Minimum log level to display
- **Valid values**: `"error"`, `"warn"`, `"info"`, `"debug"`, `"trace"`
- **Environment**: `RUST_LOG`

#### logging.format

- **Type**: String
- **Default**: `"text"`
- **Description**: Log output format
- **Valid values**: `"text"`, `"json"`

#### logging.file

- **Type**: Path or `null`
- **Default**: `null`
- **Description**: Path to log file (null = stdout/stderr only)

## Environment Variables

Environment variables take precedence over configuration file values.

### Core Environment Variables

#### SWISSARMYHAMMER_MEMOS_DIR

- **Type**: Path
- **Description**: Override memo storage directory
- **Example**: `/custom/path/to/memos`

#### SWISSARMYHAMMER_ISSUES_DIR

- **Type**: Path
- **Description**: Override issue storage directory
- **Example**: `/custom/path/to/issues`

#### SAH_CLI_MODE

- **Type**: Boolean (`0` or `1`)
- **Description**: Enable CLI mode features
- **Default**: `0`

### Logging Environment Variables

#### RUST_LOG

- **Type**: String
- **Description**: Control Rust logging (overrides `logging.level`)
- **Valid values**: `error`, `warn`, `info`, `debug`, `trace`
- **Example**: `RUST_LOG=debug sah serve`

#### RUST_LOG_STYLE

- **Type**: String
- **Description**: Control log output styling
- **Valid values**: `auto`, `always`, `never`

### Agent Environment Variables

#### ANTHROPIC_API_KEY

- **Type**: String
- **Description**: API key for Claude models
- **Required**: For agent execution features

## Command-Line Arguments

Command-line arguments override both configuration files and environment variables.

### Global Arguments

```bash
sah [OPTIONS] <COMMAND>
```

#### --config, -c

- **Type**: Path
- **Description**: Path to configuration file
- **Example**: `sah --config ./custom-config.yaml serve`

#### --verbose, -v

- **Type**: Flag (can be repeated)
- **Description**: Increase logging verbosity
- **Example**: `sah -vv serve` (debug level)

#### --quiet, -q

- **Type**: Flag
- **Description**: Suppress non-error output

#### --version, -V

- **Type**: Flag
- **Description**: Print version information

#### --help, -h

- **Type**: Flag
- **Description**: Print help information

#### --cwd

- **Type**: Path
- **Description**: Change working directory before any initialization
- **Example**: `sah --cwd /path/to/project serve`
- **Usage**: This flag is processed before loading configuration or initializing contexts, ensuring all relative paths and configurations are resolved from the specified directory. Useful for starting the server from a different location than your project root.

### Serve Command Arguments

```bash
sah serve [OPTIONS]
```

#### --http

- **Type**: Flag
- **Description**: Use HTTP transport instead of stdio

#### --port, -p

- **Type**: Integer
- **Description**: Port for HTTP server (requires --http)
- **Default**: `8080`
- **Example**: `sah serve --http --port 3000`

#### --host

- **Type**: String
- **Description**: Host address for HTTP server (requires --http)
- **Default**: `127.0.0.1`
- **Example**: `sah serve --http --host 0.0.0.0`

## Configuration Precedence

Settings are applied in this order (later overrides earlier):

1. Default values (hardcoded)
2. Global configuration file (`~/.config/swissarmyhammer/sah.yaml`)
3. Project-local configuration file (`./sah.yaml`)
4. Environment variables
5. Command-line arguments (highest priority)

### Example Precedence

Given:
- Global config: `port: 8080`
- Local config: `port: 3000`
- Environment: `SAH_PORT=4000`
- Command line: `--port 5000`

Result: Port `5000` is used.

## Validation

Configuration values are validated at startup:

- Required fields must be present
- Values must be within acceptable ranges
- Paths must be valid
- Enum values must match allowed values

Invalid configuration causes startup to fail with descriptive error messages.

## Best Practices

### Project-Specific Configuration

Create `sah.yaml` in project root for project-specific settings:

```yaml
# Project-specific settings
issues:
  directory: ".swissarmyhammer/issues"

memos:
  directory: ".swissarmyhammer/memos"

# Team-specific agent settings
agent:
  model: "claude-sonnet-4"
  max_tokens: 100000
```

Commit this file to version control so all team members use consistent settings.

### Personal Configuration

Use global configuration for personal preferences:

```yaml
# ~/.config/swissarmyhammer/sah.yaml

logging:
  level: "debug"  # Personal preference for more logging

server:
  port: 8080  # Personal default port
```

Don't commit personal configuration to version control.

### Environment-Specific Configuration

Use environment variables for environment-specific values:

```bash
# Development
export RUST_LOG=debug
export SWISSARMYHAMMER_MEMOS_DIR=/tmp/dev-memos

# Production
export RUST_LOG=warn
export SWISSARMYHAMMER_MEMOS_DIR=/var/lib/swissarmyhammer/memos
```

### Security Considerations

- Never commit API keys to version control
- Use environment variables for sensitive values
- Restrict configuration file permissions: `chmod 600 sah.yaml`
- Review CORS origins carefully for HTTP mode

## Configuration Examples

### Minimal Configuration

```yaml
# Simplest possible configuration
agent:
  model: "claude-sonnet-4"
```

### Development Configuration

```yaml
# Good for development
logging:
  level: "debug"

files:
  max_size: 52428800  # 50 MB for faster testing

shell:
  default_timeout: 60000  # 1 minute for faster feedback
```

### Production Configuration

```yaml
# Production-ready configuration
agent:
  name: "production"
  model: "claude-sonnet-4"
  max_tokens: 100000
  timeout: 600

logging:
  level: "warn"
  format: "json"
  file: "/var/log/swissarmyhammer/server.log"

server:
  mode: "http"
  host: "127.0.0.1"
  port: 8080
  cors_origins:
    - "https://example.com"

files:
  max_size: 104857600

shell:
  max_timeout: 300000  # 5 minutes max
```

### Team Configuration

```yaml
# Committed to repository for team consistency
agent:
  model: "claude-sonnet-4"
  max_tokens: 100000

issues:
  directory: ".swissarmyhammer/issues"
  auto_commit: true  # Team uses git integration

memos:
  directory: ".swissarmyhammer/memos"
  auto_commit: true

search:
  max_results: 500  # Reasonable default for team
```

## Migration Guide

### From Previous Versions

If upgrading from an earlier version:

1. **Backup existing configuration**:
   ```bash
   cp sah.yaml sah.yaml.backup
   ```

2. **Check for deprecated options** in server logs

3. **Update configuration format** if needed

4. **Test with new configuration**:
   ```bash
   sah serve --config sah.yaml
   ```

## Related Documentation

- [Environment Variables Reference](./environment.md)
- [Getting Started](../getting-started.md)
- [Troubleshooting](../troubleshooting.md)
