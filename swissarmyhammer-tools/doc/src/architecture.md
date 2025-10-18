# Architecture Overview

SwissArmyHammer Tools follows a modular, extensible architecture built on the Model Context Protocol (MCP) specification. This document provides a high-level overview of the system design, component relationships, and key architectural decisions.

## Quick Overview

**Three-Layer Architecture**:

1. **MCP Server Layer**: Handles protocol communication with AI assistants (Claude Desktop, etc.)
2. **Tool Layer**: Implements individual capabilities (file operations, search, issues, etc.)
3. **Storage Layer**: Manages persistent data (issues, memos, workflows) as markdown/YAML files

**Key Components**:

- **Tool Registry**: Pluggable system for managing 40+ tools with O(1) lookup
- **Tool Context**: Dependency injection providing tools access to storage and services
- **Prompt Library**: Reusable templates with hot-reloading support
- **Storage Backends**: File-based persistence using markdown for human-readable, git-friendly data

**Communication Flow**: AI Client (via MCP) → MCP Server → Tool Registry → Specific Tool → Storage Backends

## System Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        MCP Client                            │
│                    (Claude Desktop, etc.)                    │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           │ MCP Protocol (stdio/HTTP)
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                      McpServer                               │
│  ┌────────────────┐  ┌────────────────┐  ┌───────────────┐ │
│  │ Prompt Library │  │ Tool Registry  │  │ File Watcher  │ │
│  └────────────────┘  └────────────────┘  └───────────────┘ │
└──────────────────────────┬──────────────────────────────────┘
                           │
         ┌─────────────────┴─────────────────┐
         │                                   │
┌────────▼────────┐                  ┌──────▼──────┐
│  Tool Context   │                  │   Prompts   │
│  ┌───────────┐  │                  │             │
│  │ Storage   │  │                  │ Liquid      │
│  │ Backends  │  │                  │ Templates   │
│  └───────────┘  │                  │             │
└─────────────────┘                  └─────────────┘
         │
         │
    ┌────┴───────────────────────────────────┐
    │                                         │
┌───▼────────┐  ┌──────────────┐  ┌─────────▼──────┐
│  Issues    │  │    Memos     │  │   Workflows    │
│  Storage   │  │   Storage    │  │    Storage     │
└────────────┘  └──────────────┘  └────────────────┘
```

## Core Components

### MCP Server

The `McpServer` serves as the central orchestrator for all SwissArmyHammer functionality. It:

- Implements the MCP `ServerHandler` trait for protocol compliance
- Manages the prompt library and tool registry
- Handles file watching for automatic prompt reloading
- Provides both stdio and HTTP server modes
- Coordinates tool execution through the tool context

Key responsibilities:
- Protocol implementation and client communication
- Lifecycle management (initialization, shutdown)
- Request routing (prompts, tools, resources)
- Error handling and response formatting

See [MCP Server](./architecture/mcp-server.md) for detailed information.

### Tool Registry

The `ToolRegistry` provides a pluggable system for managing MCP tools. Instead of hardcoded tool dispatch, it enables:

- Dynamic tool registration at startup
- O(1) tool lookup by name
- Tool discovery and enumeration
- Schema validation and CLI integration
- Graceful error handling for invalid tools

Each tool implements the `McpTool` trait:
```rust
pub trait McpTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn schema(&self) -> serde_json::Value;
    async fn execute(&self, arguments: Map, context: &ToolContext) -> Result;
}
```

See [Tool Registry](./architecture/tool-registry.md) for implementation details.

### Tool Context

The `ToolContext` provides dependency injection for tools, giving them access to:

- **Issue Storage**: File-based issue tracking system with lifecycle management
- **Memo Storage**: Markdown note-taking and knowledge management with ULID identifiers
- **Git Operations**: Repository operations, change tracking, and branch detection
- **Agent Configuration**: AI agent executor settings and workflow coordination
- **Tool Handlers**: Shared utilities for common operations across tools

This shared context ensures tools can access necessary services without tight coupling, enabling modular tool development and independent testing.

### Storage Backends

SwissArmyHammer uses file-based storage with trait abstractions:

- **IssueStorage**: Manages work items as markdown files in `.swissarmyhammer/issues`
- **MemoStorage**: Stores notes as markdown files in `.swissarmyhammer/memos`
- **WorkflowStorage**: Defines development workflows as YAML files

All storage backends:
- Use markdown/YAML for human-readable, version-controllable data
- Support default location discovery via git repository detection
- Allow custom paths via environment variables or configuration
- Implement atomic operations for data consistency

See [Storage Backends](./architecture/storage-backends.md) for details.

### Prompt Library

The `PromptLibrary` manages reusable prompt templates:

- Loads prompts from multiple directories (bundled, global, project-local)
- Supports Liquid templating with variables
- Handles partial templates for composition
- Provides hot-reloading via file watching

Prompts are discovered from:
1. Bundled prompts in the swissarmyhammer-prompts crate
2. Global prompts in `~/.swissarmyhammer/prompts`
3. Project-local prompts in `.swissarmyhammer/prompts`

## Data Flow

### Tool Execution Flow

```
1. MCP Client sends call_tool request
   ↓
2. McpServer receives request via ServerHandler::call_tool
   ↓
3. ToolRegistry looks up tool by name
   ↓
4. Tool validates arguments against schema
   ↓
5. Tool executes with ToolContext for storage access
   ↓
6. Tool returns CallToolResult
   ↓
7. McpServer sends response to client
```

### Prompt Rendering Flow

```
1. MCP Client sends get_prompt request
   ↓
2. McpServer receives request via ServerHandler::get_prompt
   ↓
3. PromptLibrary looks up prompt by name
   ↓
4. Template engine renders prompt with arguments
   ↓
5. McpServer returns rendered content
   ↓
6. Client uses prompt for AI interaction
```

### File Watching Flow

```
1. FileWatcher starts monitoring prompt directories
   ↓
2. File system change detected
   ↓
3. McpFileWatcherCallback receives event
   ↓
4. McpServer reloads prompts with retry logic
   ↓
5. Notification sent to MCP client via prompts/list_changed
   ↓
6. Client refreshes prompt list
```

## Design Principles

### Modularity

- Each domain (issues, memos, search, git, shell, etc.) lives in its own crate
- Tools are self-contained and independently testable
- Clean separation between MCP protocol and business logic
- Domain crates have no dependencies on the MCP server
- Each tool category is independently registered at startup

### Extensibility

- New tools can be added without modifying existing code
- Storage backends use trait abstractions for alternative implementations
- Registry pattern supports dynamic tool discovery

### Performance

- HashMap-based tool lookup provides O(1) access
- File watching avoids polling for prompt changes
- Semantic search uses SQLite with vector embeddings for efficient queries

### Safety

- Rust's type system prevents common errors
- Atomic file operations ensure data consistency
- Schema validation catches errors before execution

### User Experience

- Markdown-based storage is human-readable and git-friendly
- Comprehensive error messages with context
- Graceful degradation when optional features unavailable

## Integration Points

### MCP Protocol

SwissArmyHammer implements the full MCP specification:
- **Capabilities**: Prompts (with list_changed), Tools (with list_changed)
- **Transports**: stdio (for Claude Desktop) and HTTP (for web clients)
- **Protocol Version**: Latest stable MCP specification

### Claude Code Integration

Designed for seamless integration with Claude Code:
- stdio mode for native integration
- File watching for live prompt updates
- Comprehensive tool suite for development workflows
- Context preservation across long-running tasks

### Git Integration

Deep integration with git workflows:
- Automatic repository detection
- Branch-based issue tracking
- Change detection for focused work
- Commit and PR automation support

## Error Handling

SwissArmyHammer uses structured error handling:

1. **Domain Errors**: Specific to each subsystem (issues, memos, etc.)
2. **SwissArmyHammerError**: Unified error type for the ecosystem
3. **McpError**: MCP protocol error codes for client communication
4. **Retry Logic**: Exponential backoff for transient failures

Error conversion chain:
```
Domain Error → SwissArmyHammerError → McpError → JSON-RPC Error
```

## Configuration

Configuration is loaded in priority order:

1. Environment variables (highest priority)
   - `SWISSARMYHAMMER_MEMOS_DIR`
   - `SAH_CLI_MODE`
   - `RUST_LOG`

2. Project-local `sah.yaml`

3. Global `~/.config/swissarmyhammer/sah.yaml`

4. Default values (lowest priority)

See [Configuration Reference](../reference/configuration.md) for details.

## Performance Characteristics

- **Tool Lookup**: O(1) via HashMap
- **Prompt Loading**: O(n) where n is number of prompt files
- **File Watching**: Event-driven, no polling overhead
- **Semantic Search**: O(log n) via SQLite vector search
- **Storage Operations**: O(1) for single file operations

## Security Model

See [Security Model](./architecture/security.md) for comprehensive security documentation.

## Next Steps

- [MCP Server Details](./architecture/mcp-server.md)
- [Tool Registry Implementation](./architecture/tool-registry.md)
- [Storage Backends](./architecture/storage-backends.md)
- [Security Model](./architecture/security.md)
