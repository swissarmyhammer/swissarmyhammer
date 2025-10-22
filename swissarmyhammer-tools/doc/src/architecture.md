# System Overview

SwissArmyHammer Tools follows a clean, modular architecture designed for extensibility and maintainability.

## Architecture Principles

- **Modular Design**: Each feature area is a separate crate with clear boundaries
- **Tool Registry**: Pluggable tool system where each tool is independently registered
- **Shared Context**: Common context providing access to storage backends and operations
- **Protocol Agnostic**: Tools are independent of the MCP protocol layer
- **Async First**: Built on Tokio for efficient async operations

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        MCP Clients                          │
│              (Claude Desktop, Custom Clients)               │
└────────────────────────┬────────────────────────────────────┘
                         │
                         │ MCP Protocol (stdio/HTTP)
                         │
┌────────────────────────▼────────────────────────────────────┐
│                     MCP Server Layer                        │
│  ┌─────────────────────────────────────────────────────┐   │
│  │         Unified Server (stdio + HTTP)               │   │
│  └─────────────────────┬───────────────────────────────┘   │
│                        │                                     │
│  ┌─────────────────────▼───────────────────────────────┐   │
│  │              Tool Registry                           │   │
│  │  - Tool Discovery    - Schema Validation            │   │
│  │  - Tool Execution    - Error Handling               │   │
│  └─────────────────────┬───────────────────────────────┘   │
└────────────────────────┼─────────────────────────────────────┘
                         │
         ┌───────────────┼───────────────┐
         │               │               │
┌────────▼─────┐  ┌──────▼──────┐  ┌────▼─────┐
│ File Tools   │  │Search Tools │  │Git Tools │
│ - read       │  │ - index     │  │ - changes│
│ - write      │  │ - query     │  └──────────┘
│ - edit       │  └─────────────┘
│ - glob       │  ┌─────────────┐  ┌──────────┐
│ - grep       │  │Issue Tools  │  │Web Tools │
└──────────────┘  │ - create    │  │ - fetch  │
                  │ - list      │  │ - search │
┌──────────────┐  │ - show      │  └──────────┘
│ Shell Tools  │  │ - update    │
│ - execute    │  │ - complete  │  ┌──────────┐
└──────────────┘  └─────────────┘  │Flow Tool │
                                    │ - execute│
┌──────────────┐  ┌─────────────┐  └──────────┘
│Outline Tools │  │ Rules Tools │
│ - generate   │  │ - check     │  ┌──────────┐
└──────────────┘  └─────────────┘  │Memo Tools│
                                    │ - create │
┌──────────────┐                    │ - list   │
│ Todo Tools   │                    │ - get    │
│ - create     │                    └──────────┘
│ - show       │
│ - complete   │
└──────────────┘
         │
         │
┌────────▼─────────────────────────────────────┐
│         Storage & Domain Crates              │
│  ┌──────────────┐  ┌──────────────┐         │
│  │swissarmyhammer│  │swissarmyhammer│        │
│  │   -issues    │  │  -memoranda  │         │
│  └──────────────┘  └──────────────┘         │
│  ┌──────────────┐  ┌──────────────┐         │
│  │swissarmyhammer│  │swissarmyhammer│        │
│  │   -search    │  │    -git      │         │
│  └──────────────┘  └──────────────┘         │
│  ┌──────────────┐  ┌──────────────┐         │
│  │swissarmyhammer│  │swissarmyhammer│        │
│  │   -workflow  │  │    -shell    │         │
│  └──────────────┘  └──────────────┘         │
└───────────────────────────────────────────────┘
```

## Component Relationships

### MCP Server

The MCP server is the entry point for all client interactions. It:

- Handles MCP protocol communication (stdio or HTTP)
- Routes requests to the appropriate tools
- Manages tool registration and discovery
- Provides error handling and response formatting
- Exposes prompt templates from the prompt library

See [MCP Server](./architecture/mcp-server.md) for details.

### Tool Registry

The tool registry is the core of the extensibility system. It:

- Maintains a collection of available tools
- Provides tool discovery by name
- Validates tool schemas
- Executes tools with proper context
- Handles tool errors gracefully

See [Tool Registry](./architecture/tool-registry.md) for details.

### Tool Context

The tool context provides shared access to:

- Storage backends (issues, memos, workflows)
- Working directory information
- Configuration settings
- Prompt library

Each tool receives a context instance during execution, enabling access to shared resources without tight coupling.

### Storage Backends

Storage backends provide persistent storage for:

- **Issues**: Markdown files in `.swissarmyhammer/issues/`
- **Memos**: Markdown files in `.swissarmyhammer/memos/`
- **Search Index**: SQLite database in `.swissarmyhammer/search.db`
- **Workflows**: YAML files defining workflow specifications
- **Todo Lists**: YAML file in `.swissarmyhammer/todo.yaml`

See [Storage Backends](./architecture/storage-backends.md) for details.

### Domain Crates

Each feature area is implemented as a separate crate:

- `swissarmyhammer-issues`: Issue tracking and lifecycle management
- `swissarmyhammer-memoranda`: Memo/note storage and retrieval
- `swissarmyhammer-search`: Semantic search with vector embeddings
- `swissarmyhammer-git`: Git operations and change tracking
- `swissarmyhammer-workflow`: Workflow execution engine
- `swissarmyhammer-shell`: Shell command execution
- `swissarmyhammer-outline`: Code outline generation with tree-sitter
- `swissarmyhammer-rules`: Code quality checking

See [Domain Crates](./architecture/domain-crates.md) for details.

## Data Flow

### Tool Execution Flow

1. Client sends MCP request with tool name and parameters
2. Server validates the request against tool schema
3. Server looks up tool in registry
4. Server creates tool context with storage backends
5. Tool executes with validated parameters and context
6. Tool returns structured result
7. Server formats result as MCP response
8. Client receives response

### File Operation Flow

1. Tool receives file path and operation parameters
2. Security validation checks path is within working directory
3. File encoding detection (UTF-8, UTF-16, etc.)
4. Operation executes atomically
5. File metadata updated (timestamps preserved)
6. Result returned with operation details

### Search Flow

1. Index tool parses source files with tree-sitter
2. Code chunks extracted based on language grammar
3. Embeddings generated for each chunk
4. Chunks stored in SQLite with vector index
5. Query tool generates embedding for search query
6. Vector similarity search finds matching chunks
7. Results returned ranked by similarity score

## Design Decisions

### Why Separate Domain Crates?

- **Clear Boundaries**: Each domain has well-defined responsibilities
- **Independent Testing**: Each crate can be tested in isolation
- **Reusability**: Domain logic can be used outside MCP context
- **Maintainability**: Changes in one domain don't affect others

### Why Tool Registry Pattern?

- **Extensibility**: New tools can be added without modifying core server
- **Discoverability**: Tools are self-describing with schemas
- **Composability**: Tools can be registered selectively
- **Testing**: Tools can be tested independently

### Why Markdown for Issues and Memos?

- **Human Readable**: Plain text files are easy to read and edit
- **Git Friendly**: Markdown diffs well in version control
- **Portable**: No proprietary formats or databases
- **Searchable**: Standard text search tools work out of the box

### Why SQLite for Search Index?

- **Performance**: Fast vector similarity search
- **Embedded**: No external database required
- **ACID**: Reliable storage with transactions
- **Portable**: Single file database

## Next Steps

- **[MCP Server](./architecture/mcp-server.md)**: Deep dive into server implementation
- **[Tool Registry](./architecture/tool-registry.md)**: Understanding tool registration
- **[Storage Backends](./architecture/storage-backends.md)**: Storage implementation details
- **[Domain Crates](./architecture/domain-crates.md)**: Exploring domain logic
