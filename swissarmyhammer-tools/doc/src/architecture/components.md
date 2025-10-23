# Component Relationships

This document details how the major components of SwissArmyHammer Tools interact to provide a cohesive MCP server implementation.

## Component Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      MCP Server                             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌───────────────┐      ┌──────────────┐                   │
│  │ Request       │─────▶│ Tool         │                   │
│  │ Handler       │      │ Registry     │                   │
│  └───────────────┘      └──────┬───────┘                   │
│                                │                            │
│                                ▼                            │
│  ┌───────────────────────────────────────────┐             │
│  │            Tool Context                   │             │
│  │  ┌────────────┐  ┌──────────────┐        │             │
│  │  │  Prompt    │  │   Working    │        │             │
│  │  │  Library   │  │  Directory   │        │             │
│  │  └────────────┘  └──────────────┘        │             │
│  └───────────────────────────────────────────┘             │
│                                                             │
└─────────────────────────────────────────────────────────────┘
                            │
         ┌──────────────────┼──────────────────┐
         ▼                  ▼                  ▼
┌────────────────┐  ┌───────────────┐  ┌──────────────┐
│  Domain Crates │  │   Storage     │  │   External   │
│  (Issues, Git, │  │  (Files, DB)  │  │   Services   │
│   Search, etc.)│  └───────────────┘  │  (Git, Shell)│
└────────────────┘                     └──────────────┘
```

## Request Flow

### 1. Request Reception

**Stdio Mode:**
```
┌──────────┐    JSON-RPC    ┌───────────┐
│  Client  │───────────────▶│   Stdin   │
│ (Claude) │                │  Handler  │
└──────────┘                └─────┬─────┘
                                  │
                                  ▼
                            Parse Request
```

**HTTP Mode:**
```
┌──────────┐     HTTP       ┌───────────┐
│  Client  │───────────────▶│   Axum    │
│  (Web)   │                │  Handler  │
└──────────┘                └─────┬─────┘
                                  │
                                  ▼
                            Parse Request
```

### 2. Request Routing

```
Parse Request
      │
      ▼
┌─────────────┐
│  Method?    │
└──────┬──────┘
       │
       ├─ tools/list ──────▶ List Tools
       │
       └─ tools/call ───┐
                        │
                        ▼
                  ┌────────────┐
                  │ Tool Name? │
                  └──────┬─────┘
                         │
                         ▼
                   Registry Lookup
```

### 3. Tool Execution

```
Registry Lookup
      │
      ▼
┌──────────────┐
│ Validate     │
│ Parameters   │
└──────┬───────┘
       │
       ▼
┌──────────────┐
│ Create       │
│ Context      │
└──────┬───────┘
       │
       ▼
┌──────────────┐
│ Execute      │
│ Tool Logic   │
└──────┬───────┘
       │
       ▼
┌──────────────┐
│ Format       │
│ Response     │
└──────────────┘
```

## Component Interactions

### MCP Server ↔ Tool Registry

**Registration:**
```rust
// Server initializes registry
let mut registry = ToolRegistry::new();

// Register tools by category
register_file_tools(&mut registry);
register_search_tools(&mut registry);

// Server uses registry for lookups
let tool = registry.get_tool("files_read")?;
```

**Tool Invocation:**
```rust
// Server delegates to registry
let result = registry
    .execute_tool(tool_name, params, context)
    .await?;
```

### Tool ↔ Tool Context

**Context Usage:**
```rust
async fn execute(
    &self,
    params: serde_json::Value,
    context: Arc<ToolContext>,
) -> Result<serde_json::Value> {
    // Access working directory
    let path = context.working_directory.join(&params.path);

    // Use prompt library (for workflows)
    let template = context.library.get_prompt("workflow")?;

    // Perform tool operation
    // ...
}
```

### Tools ↔ Domain Crates

**Example: Issue Tools → swissarmyhammer-issues**
```rust
use swissarmyhammer_issues::{IssueManager, Issue};

async fn execute(&self, params: ..., context: ...) -> Result<...> {
    // Create issue manager from domain crate
    let manager = IssueManager::new(&context.working_directory)?;

    // Use domain logic
    let issue = Issue::new(name, content);
    manager.create(issue).await?;

    // Return result
}
```

**Example: Search Tools → swissarmyhammer-search**
```rust
use swissarmyhammer_search::{SearchEngine, IndexRequest};

async fn execute(&self, params: ..., context: ...) -> Result<...> {
    let engine = SearchEngine::new(&context.working_directory)?;

    let request = IndexRequest {
        patterns: params.patterns,
        force: params.force.unwrap_or(false),
    };

    let result = engine.index(request).await?;

    // Format and return
}
```

### Tools ↔ Storage

**File System:**
```rust
// Files stored at known locations
let issues_dir = working_dir.join(".swissarmyhammer/issues");
let memos_dir = working_dir.join(".swissarmyhammer/memoranda");
```

**Database:**
```rust
// Search index in DuckDB
let search_db = working_dir.join(".swissarmyhammer/search.db");
let conn = Connection::open(&search_db)?;
```

**Git Integration:**
```rust
// Git operations via swissarmyhammer-git
use swissarmyhammer_git::Repository;

let repo = Repository::open(&working_dir)?;
let changes = repo.get_changes("main")?;
```

## Data Flow Examples

### File Read Operation

```
Client Request
      │
      ▼
files_read tool
      │
      ├─ Validate path
      │
      ├─ Check permissions
      │
      ├─ Resolve relative to working_dir
      │
      ├─ Read file (with offset/limit)
      │
      ├─ Detect encoding
      │
      ├─ Format result
      │
      ▼
Return to client
```

### Semantic Search

```
Client Request
      │
      ▼
search_query tool
      │
      ├─ Open search database
      │
      ├─ Load embedding model
      │
      ├─ Encode query
      │
      ├─ Vector similarity search
      │
      ├─ Rank results
      │
      ├─ Format with context
      │
      ▼
Return to client
```

### Issue Creation

```
Client Request
      │
      ▼
issue_create tool
      │
      ├─ Validate issue name
      │
      ├─ Check for duplicates
      │
      ├─ Create markdown file
      │
      ├─ Git: create branch
      │
      ├─ Git: commit issue
      │
      ├─ Return issue ID
      │
      ▼
Return to client
```

## Error Propagation

```
Tool Execution Error
      │
      ▼
Domain Crate Error
      │
      ▼
Tool converts to McpError
      │
      ▼
Server formats as JSON-RPC error
      │
      ▼
Client receives structured error
```

Example:
```rust
// In domain crate
return Err(IssueError::NotFound { name });

// Tool catches and converts
.map_err(|e| McpError::InvalidParams {
    details: format!("Issue not found: {}", e)
})

// Server formats
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32602,
    "message": "Invalid params",
    "data": { "details": "Issue not found: ..." }
  }
}
```

## Concurrency Model

### Async Throughout

All I/O operations are async:

```
Client Request (async)
      │
      ▼
Server Handler (async)
      │
      ▼
Tool Execution (async)
      │
      ├─ File I/O (async)
      ├─ Database queries (async)
      └─ Shell commands (async)
      │
      ▼
Response (async)
```

### Shared State

Minimal shared mutable state:

```rust
// Immutable after initialization
Arc<ToolRegistry>

// Read-only shared context
Arc<ToolContext>

// Per-request state (not shared)
Request { id, method, params }
```

## Lifecycle

### Startup

```
1. Create PromptLibrary
2. Create ToolContext
3. Create ToolRegistry
4. Register all tools
5. Start server (stdio or HTTP)
6. Begin accepting requests
```

### Request Handling

```
1. Receive request
2. Parse JSON-RPC
3. Validate method/params
4. Look up tool
5. Execute async
6. Format response
7. Send to client
```

### Shutdown

```
1. Stop accepting requests
2. Wait for in-flight requests
3. Close database connections
4. Flush logs
5. Exit cleanly
```

## Next Steps

- **[MCP Server Design](mcp-server.md)**: Server implementation details
- **[Tool Registry](tool-registry.md)**: Tool management system
- **[Features](../features.md)**: Individual tool documentation
