# Configuration

This guide covers configuration options for SwissArmyHammer Tools.

## Configuration Overview

SwissArmyHammer Tools can be configured through:

1. **Command-line arguments**: Override settings for a specific run
2. **Environment variables**: Set defaults for your environment
3. **Configuration files**: Persistent settings for projects
4. **Client configuration**: Integration with MCP clients like Claude Desktop

## Command-Line Arguments

### Server Command

```bash
sah serve [OPTIONS]
```

**Options:**

- `--http`: Start HTTP server instead of stdio
- `--port <PORT>`: HTTP server port (default: 3000)
- `--host <HOST>`: HTTP server host (default: 127.0.0.1)

**Global Options:**

- `--cwd <PATH>`: Change working directory before starting
- `-v, --verbose`: Enable verbose logging
- `-q, --quiet`: Suppress non-error output

### Examples

Stdio mode (default):
```bash
sah serve
```

HTTP mode on custom port:
```bash
sah serve --http --port 8080
```

Custom working directory:
```bash
sah --cwd /path/to/project serve
```

Verbose logging:
```bash
sah -v serve
```

## Environment Variables

### Core Settings

**RUST_LOG**: Control logging level
```bash
# Options: error, warn, info, debug, trace
export RUST_LOG=debug
sah serve
```

**SAH_CWD**: Default working directory
```bash
export SAH_CWD=/path/to/project
sah serve
```

### Tool-Specific Settings

**SAH_CONFIG**: Configuration file path
```bash
export SAH_CONFIG=/path/to/config.json
sah serve
```

## Client Configuration

### Claude Desktop Integration

Configure Claude Desktop to use SwissArmyHammer Tools as an MCP server.

**Configuration File Locations:**

- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Linux: `~/.config/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`

**Basic Configuration:**

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

**With Custom Working Directory:**

```json
{
  "mcpServers": {
    "swissarmyhammer": {
      "command": "sah",
      "args": ["--cwd", "/path/to/project", "serve"]
    }
  }
}
```

**With Debug Logging:**

```json
{
  "mcpServers": {
    "swissarmyhammer": {
      "command": "sah",
      "args": ["serve"],
      "env": {
        "RUST_LOG": "debug"
      }
    }
  }
}
```

**Multiple Projects:**

```json
{
  "mcpServers": {
    "project-a": {
      "command": "sah",
      "args": ["--cwd", "/path/to/project-a", "serve"]
    },
    "project-b": {
      "command": "sah",
      "args": ["--cwd", "/path/to/project-b", "serve"]
    }
  }
}
```

## Project Configuration

### Directory Structure

SwissArmyHammer stores project-specific data in `.swissarmyhammer/`:

```
.swissarmyhammer/
├── issues/           # Issue tracking
│   ├── active-issue.md
│   └── complete/
│       └── done-issue.md
├── memos/            # Note storage
│   └── memo.md
├── search.db         # Search index (gitignore)
└── todo.yaml         # Ephemeral todos
```

### Git Configuration

Add to `.gitignore`:

```gitignore
# SwissArmyHammer - ignore generated/cached files
.swissarmyhammer/search.db
.swissarmyhammer/todo.yaml

# Keep issues and memos
!.swissarmyhammer/issues/
!.swissarmyhammer/memos/
```

### Commit Issues and Memos

Issues and memos should be version controlled:

```bash
git add .swissarmyhammer/issues/
git add .swissarmyhammer/memos/
git commit -m "Update project issues and memos"
```

## HTTP Server Configuration

### Security Considerations

The HTTP server is intended for local development only. For production:

1. **Use HTTPS**: Set up reverse proxy with TLS
2. **Authentication**: Add authentication layer
3. **Network Isolation**: Bind to localhost only
4. **Firewall**: Configure firewall rules

### Reverse Proxy Example (nginx)

```nginx
server {
    listen 443 ssl;
    server_name mcp.example.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    location / {
        proxy_pass http://localhost:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_cache_bypass $http_upgrade;
    }
}
```

## Logging Configuration

### Log Levels

Control verbosity with `RUST_LOG`:

**Production:**
```bash
export RUST_LOG=info
```

**Development:**
```bash
export RUST_LOG=debug
```

**Troubleshooting:**
```bash
export RUST_LOG=trace
```

**Module-Specific:**
```bash
export RUST_LOG=swissarmyhammer_tools=debug,mcp=info
```

### Log Output

Logs are written to stderr. Redirect as needed:

```bash
# Log to file
sah serve 2> sah.log

# Log to file and console
sah serve 2>&1 | tee sah.log
```

## Tool-Specific Configuration

### Search Configuration

Search index is stored in `.swissarmyhammer/search.db`.

**Index Settings:**
- Chunk size: 256 tokens
- Overlap: 50 tokens
- Embedding model: Built-in

**Re-index Strategy:**
- Automatic: Changed files only
- Manual: Force re-index when needed

### Issue Configuration

Issues are stored as markdown in `.swissarmyhammer/issues/`.

**Naming Convention:**
- Prefix with type: `FEATURE_`, `BUG_`, `REFACTOR_`
- Use descriptive names: `add-user-auth` not `issue1`
- Auto-generated: ULID if no name provided

**Lifecycle:**
- Active: `.swissarmyhammer/issues/`
- Complete: `.swissarmyhammer/issues/complete/`

## Performance Tuning

### Search Performance

**Large Codebases:**
```bash
# Index incrementally
# Only force re-index when necessary
```

**Query Performance:**
- Use specific queries
- Limit results appropriately
- Re-index after major changes

### File Operations

**Large Files:**
- Use `offset` and `limit` for reading
- Avoid loading entire file into memory

**Many Files:**
- Use specific glob patterns
- Filter by file type
- Use gitignore to skip irrelevant files

## Best Practices

### Development

1. **Use Debug Logging**: Set `RUST_LOG=debug` during development
2. **Test Locally**: Use HTTP mode for testing
3. **Version Control**: Commit issues and memos
4. **Clean Database**: Delete search.db when stale

### Production

1. **Use Info Logging**: Set `RUST_LOG=info` for production
2. **Secure HTTP**: Use reverse proxy with TLS
3. **Monitor Logs**: Watch for errors and warnings
4. **Regular Backups**: Backup `.swissarmyhammer/` directory

### Collaboration

1. **Share Configuration**: Document project-specific settings
2. **Commit Issues**: Keep team synchronized
3. **Consistent Paths**: Use relative paths in issues
4. **Review Changes**: Review issue and memo changes in PRs

## Troubleshooting Configuration

### Configuration Not Loading

1. Check file path and permissions
2. Verify JSON syntax
3. Check environment variables
4. Review logs for errors

### Claude Desktop Not Connecting

1. Verify configuration file location
2. Check JSON syntax (no trailing commas)
3. Verify `sah` is in PATH
4. Restart Claude Desktop after changes

### Working Directory Issues

1. Verify path exists: `ls -la /path/to/project`
2. Check permissions: `ls -la /path/to/project/.swissarmyhammer`
3. Use absolute paths
4. Verify git repository if using git tools

## Example Configurations

### Single Project Setup

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

Start Claude Desktop in project directory.

### Multi-Project Setup

```json
{
  "mcpServers": {
    "main-project": {
      "command": "sah",
      "args": ["--cwd", "/Users/dev/projects/main", "serve"]
    },
    "side-project": {
      "command": "sah",
      "args": ["--cwd", "/Users/dev/projects/side", "serve"]
    }
  }
}
```

### Development Setup

```json
{
  "mcpServers": {
    "swissarmyhammer-dev": {
      "command": "sah",
      "args": ["-v", "serve"],
      "env": {
        "RUST_LOG": "debug"
      }
    }
  }
}
```

## Next Steps

- [Features](./features.md): Explore available tools
- [Quick Start](./quick-start.md): Try your first tasks
- [Troubleshooting](./troubleshooting.md): Solve common problems
