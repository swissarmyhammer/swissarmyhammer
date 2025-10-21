# MCP Server

The `McpServer` is the central component of SwissArmyHammer Tools, implementing the Model Context Protocol specification to provide AI assistants with structured access to development tools and prompts.

## Responsibilities

The MCP server handles:

1. **Protocol Implementation**: Full MCP ServerHandler trait implementation
2. **Tool Management**: Registration, discovery, and execution of MCP tools
3. **Prompt Management**: Loading, rendering, and serving prompt templates
4. **File Watching**: Automatic reloading when prompts change
5. **Client Communication**: Both stdio and HTTP transport modes
6. **Error Handling**: Structured error responses with retry logic

## Architecture

```rust
pub struct McpServer {
    library: Arc<RwLock<PromptLibrary>>,
    file_watcher: Arc<Mutex<FileWatcher>>,
    tool_registry: Arc<ToolRegistry>,
    tool_context: Arc<ToolContext>,
}
```

### Components

#### Prompt Library
- Manages all prompt templates
- Supports Liquid templating with variables
- Hot-reloads when prompt files change
- Filters out partial templates from MCP exposure

#### File Watcher
- Monitors prompt directories for changes
- Triggers automatic reload with exponential backoff
- Sends `prompts/list_changed` notifications to clients
- Handles transient file system errors gracefully

#### Tool Registry
- Stores all registered MCP tools
- Provides O(1) tool lookup by name
- Supports CLI integration and validation
- Returns structured tool descriptions for MCP

#### Tool Context
- Provides dependency injection for tools
- Gives tools access to storage backends
- Manages git operations
- Holds agent configuration

## Initialization

```rust
let server = McpServer::new(library).await?;
server.initialize().await?;
```

### Initialization Steps

1. **Storage Setup**
   - Create issue storage in `.swissarmyhammer/issues`
   - Create memo storage (environment variable or default location)
   - Initialize git operations (optional, warns if unavailable)

2. **Configuration Loading**
   - Load agent configuration from `sah.yaml`
   - Fall back to defaults if configuration unavailable
   - Apply environment variable overrides

3. **Tool Registration**
   - Register all tool categories (files, issues, memos, etc.)
   - Validate tool schemas for CLI compatibility
   - Build tool context with storage backends

4. **Prompt Loading**
   - Scan bundled, global, and project-local prompt directories
   - Parse frontmatter and template content
   - Register prompts in the library

## MCP Protocol Implementation

### ServerHandler Methods

#### initialize()
Handles MCP client connection:
```rust
async fn initialize(
    &self,
    request: InitializeRequestParam,
    context: RequestContext<RoleServer>,
) -> Result<InitializeResult, McpError>
```

- Starts file watching for the connected client
- Returns server capabilities (prompts with list_changed, tools with list_changed)
- Provides server information (name, version, website)

#### list_prompts()
Returns all available prompts:
```rust
async fn list_prompts(
    &self,
    _request: Option<PaginatedRequestParam>,
    _context: RequestContext<RoleServer>,
) -> Result<ListPromptsResult, McpError>
```

- Filters out partial templates (not exposed via MCP)
- Returns prompt names, descriptions, and arguments
- Supports pagination (not currently used)

#### get_prompt()
Retrieves and renders a specific prompt:
```rust
async fn get_prompt(
    &self,
    request: GetPromptRequestParam,
    _context: RequestContext<RoleServer>,
) -> Result<GetPromptResult, McpError>
```

- Validates prompt exists and is not a partial template
- Renders template with provided arguments
- Returns rendered content as PromptMessage
- Provides helpful error messages for missing prompts

#### list_tools()
Returns all registered tools:
```rust
async fn list_tools(
    &self,
    _request: Option<PaginatedRequestParam>,
    _context: RequestContext<RoleServer>,
) -> Result<ListToolsResult, McpError>
```

- Queries tool registry for all registered tools
- Returns tool schemas for client validation
- Includes tool descriptions and parameter definitions

#### call_tool()
Executes a specific tool:
```rust
async fn call_tool(
    &self,
    request: CallToolRequestParam,
    _context: RequestContext<RoleServer>,
) -> Result<CallToolResult, McpError>
```

- Looks up tool in registry by name
- Validates arguments against tool schema
- Executes tool with tool context
- Returns structured result or error
- Logs execution for debugging

## Progress Notifications

### Architecture

Progress notifications use a channel-based architecture for non-blocking delivery:

```rust
pub struct ToolContext {
    progress_sender: Option<Arc<ProgressSender>>,
    // ... other fields
}

pub struct ProgressSender {
    sender: mpsc::Sender<ProgressNotification>,
}
```

**Design Principles**:
- **Non-blocking**: Tools send notifications without waiting for client acknowledgment
- **Optional**: Progress sender is optional in ToolContext to avoid overhead when not needed
- **ULID Tokens**: Each operation gets a unique ULID token for tracking concurrent operations
- **Metadata Support**: Tools can include custom JSON metadata for rich progress information

**Notification Flow**:
1. Tool generates progress update (e.g., file processed, command output line)
2. Tool sends notification via ToolContext.progress_sender channel
3. MCP server receives notification on channel
4. Server serializes and sends MCP progress notification to client
5. Client displays progress to user (non-blocking)

### Tools with Progress Notification Support

The following tools send progress notifications during execution:

- **flow**: Tracks workflow state transitions and step completion with state metadata
- **shell_execute**: Streams command output in real-time as lines are produced
- **search_index**: Reports files indexed with counts and percentage complete
- **files_glob**: Reports pattern matching progress across large directory trees
- **files_grep**: Reports content search progress with file and match counts
- **outline_generate**: Reports parsing progress across multiple source files
- **rules_check**: Reports rule checking progress with file counts
- **web_fetch**: Tracks HTTP request and HTML-to-markdown conversion progress
- **web_search**: Reports search execution and content fetching from result URLs

Each tool sends notifications at appropriate milestones to balance responsiveness with performance.

## File Watching

The server monitors prompt directories for changes and automatically reloads prompts when files are modified, created, or deleted.

### File Watching Flow

```
1. Client connects → initialize() called
   ↓
2. start_file_watching() initializes watcher
   ↓
3. FileWatcher monitors prompt directories
   ↓
4. File change detected → callback triggered
   ↓
5. reload_prompts_with_retry() reloads all prompts
   ↓
6. Send prompts/list_changed notification to client
   ↓
7. Client refreshes prompt list
```

### Retry Logic

File operations use exponential backoff for transient errors:

```rust
const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 100;
```

Retryable errors:
- `TimedOut`
- `Interrupted`
- `WouldBlock`
- `UnexpectedEof`
- Messages containing "temporarily unavailable", "resource busy", "locked"

### File Watcher Callback

The `McpFileWatcherCallback` handles file change events:

```rust
impl FileWatcherCallback for McpFileWatcherCallback {
    async fn on_change(&self, _path: &Path) -> Result<()> {
        // Reload prompts
        self.server.reload_prompts().await?;
        
        // Notify client
        self.peer.send_notification(
            "prompts/list_changed",
            None
        ).await?;
        
        Ok(())
    }
}
```

## Transport Modes

### Stdio Mode

Default mode for Claude Desktop integration:

```bash
sah serve  # Uses stdio by default
```

- Communicates via standard input/output
- Used by Claude Desktop and similar tools
- Single client per server instance
- Automatic lifecycle management

### HTTP Mode

For web-based integrations:

```bash
sah serve --http --port 8080
```

- HTTP server with JSON-RPC endpoints
- Supports multiple concurrent clients
- CORS support for browser clients
- Explicit shutdown required

## Error Handling

### Error Conversion

Domain errors are converted to MCP errors:

```rust
Domain Error → SwissArmyHammerError → McpError → JSON-RPC Error
```

### Error Types

- `InvalidRequest`: Malformed requests or missing required parameters
- `InternalError`: Unexpected errors during execution
- Custom error codes for specific failure modes

### Error Context

All errors include:
- Human-readable error message
- Optional details for debugging
- Appropriate HTTP status code (for HTTP mode)

## Configuration

The server respects configuration from multiple sources:

### Environment Variables
- `SWISSARMYHAMMER_MEMOS_DIR`: Custom memo storage location
- `SAH_CLI_MODE`: Enable CLI mode features
- `RUST_LOG`: Logging level (debug, info, warn, error)

### Configuration File

`sah.yaml` or `~/.config/swissarmyhammer/sah.yaml`:

```yaml
agent:
  name: "default"
  model: "claude-sonnet-4"
  max_tokens: 100000

issues:
  directory: ".swissarmyhammer/issues"

memos:
  directory: ".swissarmyhammer/memos"
```

## Performance Considerations

### Caching
- Prompt library cached in memory
- Tool registry built once at startup
- No caching of tool execution results (tools responsible for own caching)

### Concurrency
- Read-write lock for prompt library (multiple concurrent readers)
- Mutex for file watcher (single writer)
- Arc for shared ownership across async tasks

### Resource Usage
- Memory: ~50MB base + prompt library size
- CPU: Event-driven, minimal overhead
- Disk I/O: Only on prompt reloads and tool execution

## Testing

### Unit Tests
- Tool registration and lookup
- Prompt loading and rendering
- Error handling and conversion
- Configuration loading

### Integration Tests
- MCP protocol compliance
- End-to-end tool execution
- File watching behavior
- Multiple client scenarios

## Best Practices

### For Tool Developers
- Use BaseToolImpl helpers for consistent responses
- Convert errors using McpErrorHandler
- Test tools independently of server
- Document tool schemas comprehensively

### For Users
- Keep prompt files small and focused
- Use project-local prompts for project-specific needs
- Monitor server logs for debugging
- Restart server after configuration changes

## Troubleshooting

### Common Issues

**Problem**: Prompts not appearing in client

**Solution**:
1. Check prompt files have valid frontmatter
2. Ensure prompt files are in scanned directories
3. Check server logs for loading errors
4. Verify prompts are not marked as partial templates

**Problem**: Tools not executing

**Solution**:
1. Verify tool is registered in tool registry
2. Check tool schema is valid
3. Review tool execution logs
4. Ensure tool context has necessary storage backends

**Problem**: File watching not working

**Solution**:
1. Check file system permissions
2. Verify prompt directories exist
3. Review file watcher logs
4. Try manual reload via API

## Related Documentation

- [Tool Registry](./tool-registry.md)
- [Storage Backends](./storage-backends.md)
- [Security Model](./security.md)
- [Configuration Reference](../reference/configuration.md)
