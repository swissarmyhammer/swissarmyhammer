# MCP Server Design

The MCP (Model Context Protocol) server is the core component of SwissArmyHammer Tools, implementing the full MCP specification to enable AI assistants to interact with development tools.

## Overview

The MCP server provides a standardized interface for AI clients to access development capabilities through a well-defined protocol. It handles request routing, tool execution, and response formatting while ensuring type safety and proper error handling.

## Server Modes

### Stdio Mode

Stdio mode is optimized for desktop application integration like Claude Code.

**Characteristics:**
- Communicates over standard input/output
- Single-threaded request processing
- No network overhead
- Process-based isolation
- Best for local, single-user scenarios

**Implementation:**
```rust
// src/mcp/server.rs
pub async fn run_stdio_mode(&self) -> Result<()> {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    // Process requests from stdin
    // Write responses to stdout
}
```

###HTTP Mode

HTTP mode provides a RESTful interface for web-based integrations.

**Characteristics:**
- HTTP/1.1 protocol
- Concurrent request handling
- Network accessible
- Stateless operation
- Best for web integrations or remote access

**Endpoints:**
- `POST /mcp` - MCP protocol endpoint
- `GET /health` - Health check endpoint
- `GET /tools` - List available tools

**Implementation:**
```rust
// src/mcp/unified_server.rs
pub async fn run_http_mode(&self, port: u16) -> Result<()> {
    let app = Router::new()
        .route("/mcp", post(handle_mcp_request))
        .route("/health", get(health_check))
        .route("/tools", get(list_tools));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
}
```

## Request Processing

### JSON-RPC 2.0

MCP uses JSON-RPC 2.0 for message format:

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "files_read",
    "arguments": {
      "path": "Cargo.toml"
    }
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": "...",
    "contentType": "text",
    "encoding": "utf-8"
  }
}
```

**Error Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32602,
    "message": "Invalid params",
    "data": {
      "details": "Path parameter is required"
    }
  }
}
```

### Request Flow

1. **Receive**: Accept JSON-RPC request via stdio or HTTP
2. **Parse**: Deserialize JSON to request structure
3. **Validate**: Check method and parameter validity
4. **Route**: Determine which tool to invoke
5. **Execute**: Run tool with provided parameters
6. **Format**: Serialize result to JSON response
7. **Send**: Return response to client

## Tool Discovery

### List Tools

Clients discover available tools via the `tools/list` method:

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/list"
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "tools": [
      {
        "name": "files_read",
        "description": "Read file contents...",
        "inputSchema": {
          "type": "object",
          "properties": {
            "path": {
              "type": "string",
              "description": "Path to file"
            }
          },
          "required": ["path"]
        }
      }
    ]
  }
}
```

## Type Safety

### JSON Schema Validation

Every tool defines a JSON schema for its parameters:

```rust
impl McpTool for FilesRead {
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                },
                "offset": {
                    "type": "number",
                    "description": "Line offset (optional)"
                },
                "limit": {
                    "type": "number",
                    "description": "Number of lines (optional)"
                }
            },
            "required": ["path"]
        })
    }
}
```

### Runtime Validation

Parameters are validated before tool execution:

```rust
pub fn validate_params(
    schema: &serde_json::Value,
    params: &serde_json::Value
) -> Result<()> {
    // Validate required fields
    // Check type constraints
    // Verify value ranges
}
```

## Error Handling

### Error Types

```rust
pub enum McpError {
    ParseError { message: String },
    InvalidRequest { message: String },
    MethodNotFound { method: String },
    InvalidParams { details: String },
    InternalError { message: String, source: Option<Box<dyn Error>> },
}
```

### Error Codes

Following JSON-RPC 2.0 standard:
- `-32700`: Parse error
- `-32600`: Invalid request
- `-32601`: Method not found
- `-32602`: Invalid params
- `-32603`: Internal error

### Error Context

Errors include detailed context for debugging:

```rust
McpError::InvalidParams {
    details: format!(
        "Parameter 'path' is required but was not provided. \
         Tool: files_read, Request ID: {}",
        request_id
    )
}
```

## State Management

### Server State

The server maintains minimal state:
- Tool registry (immutable after initialization)
- Tool context (shared resources)
- Request/response correlation (per-request)

### Stateless Operations

Tools are designed to be stateless:
- No shared mutable state between invocations
- Each tool execution is independent
- State is stored externally (files, databases)

## Concurrency

### Async Execution

All tool execution is async using Tokio:

```rust
pub async fn execute_tool(
    &self,
    name: &str,
    params: serde_json::Value
) -> Result<serde_json::Value> {
    let tool = self.registry.get_tool(name)?;
    let context = Arc::clone(&self.context);

    tool.execute(params, context).await
}
```

### Concurrent Requests

HTTP mode supports concurrent tool execution:
- Multiple requests processed simultaneously
- Configurable concurrency limits
- Per-tool resource management

## Security

### Request Validation

All requests are validated:
- JSON-RPC 2.0 compliance
- Method whitelist
- Parameter schema validation
- Size limits

### Resource Protection

Protection against resource exhaustion:
- Maximum request size
- Execution timeouts
- Concurrent request limits
- Rate limiting (HTTP mode)

### Tool Isolation

Tools operate in isolation:
- Separate contexts
- Independent error handling
- No cross-tool state sharing

## Performance Optimization

### Connection Pooling

HTTP mode uses connection pooling:
```rust
let pool = ConnectionPool::new()
    .max_connections(100)
    .idle_timeout(Duration::from_secs(60));
```

### Request Batching

Support for batched JSON-RPC requests:
```json
[
  {"jsonrpc": "2.0", "id": 1, "method": "tools/call", ...},
  {"jsonrpc": "2.0", "id": 2, "method": "tools/call", ...}
]
```

### Response Streaming

Large responses can be streamed:
```rust
pub async fn stream_response(
    &self,
    writer: impl AsyncWrite
) -> Result<()> {
    // Stream response chunks
}
```

## Monitoring and Observability

### Logging

Structured logging using `tracing`:
```rust
tracing::info!(
    tool = %tool_name,
    duration_ms = %duration.as_millis(),
    "Tool execution completed"
);
```

### Metrics

Key metrics tracked:
- Request count per tool
- Execution duration
- Error rates
- Concurrent requests

### Health Checks

Health endpoint provides server status:
```rust
pub async fn health_check() -> Json<HealthStatus> {
    Json(HealthStatus {
        status: "healthy",
        uptime: server.uptime(),
        tools_registered: registry.count(),
    })
}
```

## Lifecycle

### Initialization

1. Create prompt library
2. Initialize tool registry
3. Register all tools
4. Start server (stdio or HTTP)

### Shutdown

1. Stop accepting new requests
2. Complete in-flight requests
3. Clean up resources
4. Flush logs and metrics

## Next Steps

- **[Tool Registry](tool-registry.md)**: How tools are registered and managed
- **[Component Relationships](components.md)**: Detailed component interactions
- **[Features](../features.md)**: Explore individual tool capabilities
