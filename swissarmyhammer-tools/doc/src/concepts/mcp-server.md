# MCP Server

The MCP Server is the core component that implements the Model Context Protocol specification, enabling standardized communication between AI assistants and development tools.

## Overview

The server handles all MCP protocol communication, including:
- Tool execution requests
- Resource queries
- Prompt template serving
- Progress notifications
- Error responses

## Transport Modes

### Stdio Mode (Default)

Used by Claude Code and other desktop AI assistants:

```bash
sah serve
# or explicitly
sah serve --stdio
```

**Characteristics**:
- JSON-RPC over standard input/output
- Single client connection
- Automatic startup by Claude Code
- Lower latency

### HTTP Mode

Used for web-based integrations:

```bash
sah serve --http --port 3000
```

**Characteristics**:
- JSON-RPC over HTTP POST
- Multiple client support
- Network accessible
- RESTful endpoints

## Server Lifecycle

### Initialization

```rust
// Create server
let library = PromptLibrary::new();
let server = McpServer::new(library).await?;

// Initialize and register tools
server.initialize().await?;
```

### Tool Registration

During initialization, the server registers all available tools:

1. File tools (read, write, edit, glob, grep)
2. Search tools (index, query)
3. Issue tools (create, list, show, update, complete)
4. Memo tools (create, get, list)
5. Todo tools (create, show, complete)
6. Git tools (changes)
7. Shell tools (execute)
8. Outline tools (generate)
9. Rules tools (check)
10. Web tools (fetch, search)
11. Workflow tools (flow, abort)

### Request Handling

The server processes requests in this order:

1. **Parse**: Decode JSON-RPC message
2. **Validate**: Check protocol conformance
3. **Route**: Find requested tool
4. **Execute**: Run tool with parameters
5. **Respond**: Return results or errors

## Progress Notifications

For long-running operations, the server sends progress notifications:

```json
{
  "method": "notifications/progress",
  "params": {
    "progressToken": "workflow-123",
    "progress": 50,
    "message": "Processing step 2 of 4"
  }
}
```

Tools that support progress:
- `search_index` - Indexing files
- `flow` - Workflow execution
- `web_search` - Fetching search results

## Error Handling

The server provides detailed error responses:

```json
{
  "error": {
    "code": -32602,
    "message": "Invalid params",
    "data": {
      "details": "Missing required parameter 'path'"
    }
  }
}
```

Error codes follow JSON-RPC specification:
- `-32700`: Parse error
- `-32600`: Invalid request
- `-32601`: Method not found
- `-32602`: Invalid params
- `-32603`: Internal error

## File Watching

The server watches for changes to prompts and workflows:

```
~/.swissarmyhammer/prompts/
~/.swissarmyhammer/workflows/
./.swissarmyhammer/prompts/
./.swissarmyhammer/workflows/
```

When changes are detected:
- Reload prompt library
- Revalidate workflows
- Send notification to clients

## Configuration

Server behavior can be customized:

```toml
[server]
host = "127.0.0.1"
port = 3000
max_connections = 100
timeout = 300
```

Environment variables:
```bash
SAH_SERVER_HOST=0.0.0.0
SAH_SERVER_PORT=8080
```

## Next Steps

- [Tool Registry](tool-registry.md) - How tools are registered and managed
- [Storage Backends](storage-backends.md) - Data persistence layer
- [MCP Tools Reference](../tools/overview.md) - Available tools
