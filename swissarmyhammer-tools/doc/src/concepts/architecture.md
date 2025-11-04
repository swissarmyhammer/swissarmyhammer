# Architecture Overview

SwissArmyHammer Tools follows a clean, modular architecture designed for extensibility, reliability, and ease of integration with AI assistants.

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    AI Assistant (Claude)                     │
│                                                               │
│  Sends MCP requests                    Receives responses    │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       │ JSON-RPC over stdio/HTTP
                       │
┌──────────────────────▼──────────────────────────────────────┐
│                      MCP Server                              │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              Request Handler                          │  │
│  │  - Protocol parsing                                   │  │
│  │  - Request validation                                 │  │
│  │  - Error handling                                     │  │
│  └────────────────────┬──────────────────────────────────┘  │
│                       │                                      │
│  ┌────────────────────▼──────────────────────────────────┐  │
│  │              Tool Registry                            │  │
│  │  - Tool registration                                  │  │
│  │  - Schema validation                                  │  │
│  │  - Tool routing                                       │  │
│  │  - Progress notifications                             │  │
│  └────────────────────┬──────────────────────────────────┘  │
│                       │                                      │
│  ┌────────────────────▼──────────────────────────────────┐  │
│  │              Tool Context                             │  │
│  │  - Storage backends                                   │  │
│  │  - Working directory                                  │  │
│  │  - Git operations                                     │  │
│  │  - Configuration                                      │  │
│  └────────────────────┬──────────────────────────────────┘  │
└───────────────────────┼──────────────────────────────────────┘
                        │
        ┌───────────────┴────────────────┐
        │                                │
┌───────▼────────┐              ┌────────▼────────┐
│  MCP Tools     │              │ Domain Crates   │
│  (28 tools)    │              │                 │
│  - files_*     │◄─────────────┤ - issues        │
│  - search_*    │              │ - search        │
│  - issue_*     │              │ - git           │
│  - memo_*      │              │ - workflow      │
│  - todo_*      │              │ - shell         │
│  - git_*       │              │ - outline       │
│  - shell_*     │              │ - rules         │
│  - outline_*   │              │ - memoranda     │
│  - rules_*     │              │                 │
│  - web_*       │              │                 │
│  - flow        │              │                 │
└────────────────┘              └─────────────────┘
        │                                │
        └────────────┬───────────────────┘
                     │
        ┌────────────▼────────────┐
        │   Storage Layer         │
        │                         │
        │  - File System          │
        │  - Git Repository       │
        │  - SQLite (search db)   │
        └─────────────────────────┘
```

## Core Components

### MCP Server

The central server component that handles Model Context Protocol communication.

**Responsibilities**:
- Accept JSON-RPC requests over stdio or HTTP
- Parse and validate MCP protocol messages
- Route requests to appropriate tool handlers
- Manage tool lifecycle and registration
- Send progress notifications for long-running operations
- Handle errors gracefully with detailed error messages

**Key Features**:
- **Dual Transport**: Supports both stdio (for Claude Code) and HTTP modes
- **Progress Notifications**: Real-time updates using MCP notification protocol
- **File Watching**: Automatic detection of prompt/workflow changes
- **Error Resilience**: Graceful degradation when components are unavailable

**Implementation**: `src/mcp/server.rs`

### Tool Registry

A pluggable system for registering and managing MCP tools.

**Responsibilities**:
- Maintain registry of available tools
- Validate tool schemas against MCP specification
- Route tool execution requests
- Provide tool metadata and descriptions
- Manage tool dependencies

**Key Features**:
- **Dynamic Registration**: Tools register themselves at startup
- **Schema Validation**: JSON schema validation for all tool parameters
- **Extensibility**: Easy to add new tools
- **Modularity**: Each tool is self-contained

**Tool Interface**:
Each tool implements the `McpTool` trait:
```rust
pub trait McpTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> Value;
    async fn execute(&self, params: Value, context: &ToolContext) -> Result<Value>;
}
```

**Implementation**: `src/mcp/tool_registry.rs`

### Tool Context

Shared context providing access to storage backends and system resources.

**Responsibilities**:
- Manage working directory
- Provide access to storage backends (issues, memos, todos)
- Supply Git operations interface
- Share configuration settings
- Coordinate logging

**Key Features**:
- **Thread-Safe**: Uses Arc and RwLock for concurrent access
- **Lazy Initialization**: Components created on demand
- **Flexible Storage**: Abstracted storage interfaces
- **Configuration**: Centralized settings management

**Implementation**: `src/mcp/tool_registry.rs` (ToolContext struct)

### Storage Backends

Abstracted storage interfaces for different data types.

#### Issue Storage
- **Interface**: `IssueStorage` trait
- **Implementation**: `FileSystemIssueStorage`
- **Location**: `.swissarmyhammer/issues/`
- **Format**: Markdown files with YAML frontmatter
- **Features**: Git-friendly, human-readable, version-controlled

#### Memo Storage
- **Interface**: `MemoStorage` trait
- **Implementation**: `MarkdownMemoStorage`
- **Location**: `.swissarmyhammer/memos/`
- **Format**: Markdown files with metadata
- **Features**: Full-text search, ULID-based IDs, timestamps

#### Todo Storage
- **Format**: YAML file
- **Location**: `.swissarmyhammer/todo/todo.yaml`
- **Features**: Ephemeral, session-based, auto-cleanup

#### Search Index
- **Implementation**: SQLite database
- **Location**: `.swissarmyhammer/search.db`
- **Features**: Vector embeddings, tree-sitter parsing, similarity search

## Domain Crates

Each major feature area is implemented as a separate crate:

### swissarmyhammer-issues
Issue tracking and lifecycle management
- Create, list, update, complete issues
- Integration with Git branches
- Markdown-based storage

### swissarmyhammer-search
Semantic code search using vector embeddings
- Tree-sitter parsing for code structure
- Vector embeddings for similarity
- SQLite storage for fast queries

### swissarmyhammer-git
Git operations and repository management
- Change tracking
- Branch detection
- Parent branch identification

### swissarmyhammer-workflow
Workflow execution engine
- State-based workflow definitions
- Action execution
- Progress tracking

### swissarmyhammer-shell
Safe shell command execution
- Command execution with timeouts
- Output capture and streaming
- Environment variable management

### swissarmyhammer-outline
Code structure analysis
- Tree-sitter based parsing
- Symbol extraction
- Multi-language support

### swissarmyhammer-rules
Code quality checking
- Rule-based code analysis
- Severity levels
- Configurable rules

### swissarmyhammer-memoranda
Note-taking and knowledge management
- Markdown-based storage
- ULID identifiers
- Full-text search

## Tool Categories

### File Tools (`files_*`)
- **files_read**: Read file contents with partial reading
- **files_write**: Write files atomically
- **files_edit**: Precise string replacements
- **files_glob**: Pattern-based file discovery
- **files_grep**: Content-based search

### Search Tools (`search_*`)
- **search_index**: Index codebase for semantic search
- **search_query**: Query indexed code by meaning

### Issue Tools (`issue_*`)
- **issue_create**: Create new issues
- **issue_list**: List all issues
- **issue_show**: Show issue details
- **issue_update**: Update issue content
- **issue_mark_complete**: Complete an issue
- **issue_all_complete**: Check if all issues complete

### Memo Tools (`memo_*`)
- **memo_create**: Create new memo
- **memo_get**: Retrieve memo by title
- **memo_list**: List all memos
- **memo_get_all_context**: Get all memo content for AI context

### Todo Tools (`todo_*`)
- **todo_create**: Create todo item
- **todo_show**: Show todo item
- **todo_mark_complete**: Complete todo item

### Git Tools (`git_*`)
- **git_changes**: List changed files on a branch

### Shell Tools (`shell_*`)
- **shell_execute**: Execute shell commands

### Outline Tools (`outline_*`)
- **outline_generate**: Generate code structure outlines

### Rules Tools (`rules_*`)
- **rules_check**: Check code against quality rules

### Web Tools (`web_*`)
- **web_fetch**: Fetch and convert web content
- **web_search**: Search web using DuckDuckGo

### Workflow Tools
- **flow**: Execute workflows
- **abort_create**: Signal workflow termination

## Data Flow

### Typical Request Flow

1. **Client sends MCP request**
   - AI assistant formulates tool call
   - Sends JSON-RPC request with tool name and parameters

2. **Server receives and validates**
   - Parse JSON-RPC message
   - Validate against MCP protocol
   - Check tool exists in registry

3. **Tool executes**
   - Retrieve tool from registry
   - Validate parameters against JSON schema
   - Execute tool with context
   - Access storage backends as needed

4. **Response returned**
   - Tool returns result as JSON
   - Server wraps in MCP response
   - Sends back to client

5. **Progress notifications (if applicable)**
   - Long-running tools send progress updates
   - Client receives notifications during execution

## Design Principles

### Modularity
Each tool and domain crate is self-contained and independently testable.

### Extensibility
New tools can be added without modifying existing code.

### Safety
All operations include validation, error handling, and security checks.

### Performance
Efficient data structures, caching, and lazy initialization where appropriate.

### Simplicity
Clear interfaces, minimal dependencies, straightforward implementations.

### Testability
Comprehensive test coverage with unit, integration, and property-based tests.

## Next Steps

- [MCP Server](mcp-server.md) - Detailed MCP server documentation
- [Tool Registry](tool-registry.md) - Tool registration and management
- [Storage Backends](storage-backends.md) - Storage implementation details
- [MCP Tools Reference](../tools/overview.md) - Complete tool documentation
