# MCP Server

The MCP server is the core component that implements the Model Context Protocol specification and coordinates all tool execution.

## Architecture

```
┌────────────────────────────────────────┐
│         MCP Clients                    │
│  (Claude Desktop, Custom Clients)      │
└─────────────────┬──────────────────────┘
                  │
                  │ MCP Protocol
                  │
┌─────────────────▼──────────────────────┐
│         Unified Server                 │
│  ┌──────────────┐  ┌───────────────┐  │
│  │ Stdio Server │  │  HTTP Server  │  │
│  └──────┬───────┘  └───────┬───────┘  │
│         │                  │           │
│         └──────────┬───────┘           │
│                    │                   │
│         ┌──────────▼───────────┐       │
│         │    MCP Server Core   │       │
│         │  - Request handling  │       │
│         │  - Tool dispatch     │       │
│         │  - Error handling    │       │
│         └──────────┬───────────┘       │
└────────────────────┼───────────────────┘
                     │
           ┌─────────▼──────────┐
           │   Tool Registry    │
           └────────────────────┘
```

## Components

### Unified Server

The unified server provides both stdio and HTTP modes:

**Stdio Mode:**
- Communicates via standard input/output
- Used by Claude Desktop and similar clients
- Single-client, single-session
- Process-based communication

**HTTP Mode:**
- Exposes HTTP endpoints
- Multiple clients supported
- RESTful API
- Network-based communication

### MCP Server Core

The core server implements the MCP protocol:

**Initialization:**
- Registers all tools with registry
- Loads prompt library
- Sets up storage backends
- Prepares tool context

**Request Handling:**
- Parses MCP requests
- Validates tool parameters
- Dispatches to appropriate tools
- Formats responses

**Error Handling:**
- Catches tool errors
- Formats error responses
- Logs errors appropriately
- Maintains server stability

### Protocol Implementation

The server implements MCP methods:

**Tools:**
- `tools/list`: List available tools
- `tools/call`: Execute a tool

**Prompts:**
- `prompts/list`: List available prompts
- `prompts/get`: Get prompt template

**Resources:**
- Resource listing and retrieval
- File system resources
- Issue resources

## Request Flow

### Tool Execution Flow

1. **Client Request**
   - Client sends MCP tool call request
   - Includes tool name and parameters

2. **Validation**
   - Server validates request format
   - Checks tool exists in registry
   - Validates parameters against schema

3. **Context Creation**
   - Creates tool context with storage backends
   - Sets working directory
   - Provides access to shared resources

4. **Tool Execution**
   - Looks up tool in registry
   - Calls tool execute method
   - Passes validated parameters and context

5. **Response Formatting**
   - Tool returns structured result
   - Server formats as MCP response
   - Includes success/error status

6. **Client Response**
   - Server sends response to client
   - Client processes result

### Error Flow

1. **Error Occurs**
   - Tool execution fails
   - Validation fails
   - Protocol error

2. **Error Capture**
   - Server catches error
   - Logs error details
   - Determines error type

3. **Error Response**
   - Formats error as MCP error response
   - Includes error message and code
   - Provides helpful context

4. **Client Handling**
   - Client receives error
   - Displays to user
   - May retry or adjust

## Server Lifecycle

### Startup

1. Load configuration
2. Initialize prompt library
3. Create storage backends
4. Register all tools
5. Start server (stdio or HTTP)
6. Wait for connections

### Runtime

1. Accept client connections
2. Process requests
3. Execute tools
4. Return responses
5. Handle errors
6. Maintain state

### Shutdown

1. Finish pending requests
2. Close connections
3. Cleanup resources
4. Exit gracefully

## Configuration

### Server Configuration

**Working Directory:**
```bash
sah --cwd /path/to/project serve
```

**Logging:**
```bash
RUST_LOG=debug sah serve
```

**HTTP Mode:**
```bash
sah serve --http --port 3000
```

### Client Configuration

**Claude Desktop:**
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

## Performance

### Request Handling

- **Async Execution**: All operations are async using Tokio
- **Concurrent Requests**: Multiple requests can execute concurrently
- **Non-Blocking**: Server remains responsive during tool execution

### Resource Management

- **Memory**: Moderate memory usage, scales with concurrent requests
- **CPU**: CPU usage depends on tool operations (parsing, searching, etc.)
- **I/O**: Heavy I/O for file operations and search indexing

### Optimization

- **Tool Registry**: O(1) tool lookup
- **Parameter Validation**: Fast JSON schema validation
- **Error Handling**: Minimal overhead
- **Response Formatting**: Efficient serialization

## Security

### Path Validation

All file operations validate paths:
- Must be within working directory
- No path traversal
- No symlink escape
- Absolute path resolution

### Command Execution

Shell commands are validated:
- No arbitrary code execution
- Environment variable control
- Working directory restriction
- Output size limits

### Network Operations

Web tools are restricted:
- HTTP/HTTPS only
- Timeout enforcement
- Content size limits
- URL validation

## Monitoring

### Logging

Server logs include:
- Request processing
- Tool execution
- Error details
- Performance metrics

Set log level with `RUST_LOG`:
```bash
RUST_LOG=debug sah serve
```

### Metrics

Track:
- Request count
- Tool execution time
- Error rate
- Resource usage

## Error Handling

### Error Types

**Protocol Errors:**
- Invalid request format
- Unsupported method
- Malformed JSON

**Tool Errors:**
- Tool not found
- Invalid parameters
- Execution failure

**System Errors:**
- I/O errors
- Permission errors
- Resource exhaustion

### Error Recovery

- **Graceful Degradation**: Server continues after tool errors
- **Error Reporting**: Clear error messages to client
- **Logging**: All errors logged for debugging
- **Stability**: Server remains stable after errors

## Testing

### Unit Tests

Test individual components:
- Request parsing
- Tool dispatch
- Error handling
- Response formatting

### Integration Tests

Test complete flows:
- Tool execution
- Error scenarios
- Concurrent requests
- Client integration

### Performance Tests

Measure:
- Request throughput
- Latency
- Resource usage
- Scalability

## Extending the Server

### Adding Tools

1. Implement `McpTool` trait
2. Register with tool registry
3. Server automatically exposes tool

### Custom Protocol Methods

1. Implement MCP method handler
2. Register handler with server
3. Document protocol extension

### Middleware

Add middleware for:
- Authentication
- Logging
- Metrics
- Caching

## Deployment

### Development

```bash
# Stdio mode for local development
sah serve

# HTTP mode for testing
sah serve --http
```

### Production

```bash
# With logging
RUST_LOG=info sah serve

# Custom working directory
sah --cwd /app serve

# HTTP with reverse proxy
sah serve --http --port 3000
```

### Docker

```dockerfile
FROM rust:latest
COPY . /app
WORKDIR /app
RUN cargo install swissarmyhammer
CMD ["sah", "serve", "--http"]
```

## Troubleshooting

### Server Won't Start

- Check working directory exists
- Verify permissions
- Review error logs
- Check port availability (HTTP)

### Slow Response

- Check tool execution time
- Monitor resource usage
- Review logs for bottlenecks
- Consider caching

### Connection Issues

- Verify client configuration
- Check firewall settings (HTTP)
- Review server logs
- Test with curl (HTTP)

## Next Steps

- [Tool Registry](./tool-registry.md): Understanding tool registration
- [Storage Backends](./storage-backends.md): Storage implementation
- [Domain Crates](./domain-crates.md): Domain logic structure
