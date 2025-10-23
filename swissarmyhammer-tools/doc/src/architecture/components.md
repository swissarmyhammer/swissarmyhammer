# Component Relationships

This document details how the major components of SwissArmyHammer Tools interact to provide a cohesive MCP server implementation.

## Component Overview

```text
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
```text

## Request Flow

### 1. Request Reception

**Stdio Mode:**
```text
┌──────────┐    JSON-RPC    ┌───────────┐
│  Client  │───────────────▶│   Stdin   │
│ (Claude) │                │  Handler  │
└──────────┘                └─────┬─────┘
                                  │
                                  ▼
                            Parse Request
```text

**HTTP Mode:**
```text
┌──────────┐     HTTP       ┌───────────┐
│  Client  │───────────────▶│   Axum    │
│  (Web)   │                │  Handler  │
└──────────┘                └─────┬─────┘
                                  │
                                  ▼
                            Parse Request
```text

### 2. Request Routing

```text
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
```text

### 3. Tool Execution

```text
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
```text

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
```text

**Tool Invocation:**
```rust
// Server delegates to registry
let result = registry
    .execute_tool(tool_name, params, context)
    .await?;
```text

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
```text

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
```text

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
```text

### Tools ↔ Storage

**File System:**
```rust
// Files stored at known locations
let issues_dir = working_dir.join(".swissarmyhammer/issues");
let memos_dir = working_dir.join(".swissarmyhammer/memoranda");
```text

**Database:**
```rust
// Search index in DuckDB
let search_db = working_dir.join(".swissarmyhammer/search.db");
let conn = Connection::open(&search_db)?;
```text

**Git Integration:**
```rust
// Git operations via swissarmyhammer-git
use swissarmyhammer_git::Repository;

let repo = Repository::open(&working_dir)?;
let changes = repo.get_changes("main")?;
```text

## Data Flow Examples

### File Read Operation

```text
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
```text

### Semantic Search

```text
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
```text

### Issue Creation

```text
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
```text

## Error Propagation

```text
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
```text

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
```text

## Concurrency Model

### Async Throughout

All I/O operations are async:

```text
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
```text

### Shared State

Minimal shared mutable state:

```rust
// Immutable after initialization
Arc<ToolRegistry>

// Read-only shared context
Arc<ToolContext>

// Per-request state (not shared)
Request { id, method, params }
```text

## Lifecycle

### Startup

```text
1. Create PromptLibrary
2. Create ToolContext
3. Create ToolRegistry
4. Register all tools
5. Start server (stdio or HTTP)
6. Begin accepting requests
```text

### Request Handling

```text
1. Receive request
2. Parse JSON-RPC
3. Validate method/params
4. Look up tool
5. Execute async
6. Format response
7. Send to client
```text

### Shutdown

```text
1. Stop accepting requests
2. Wait for in-flight requests
3. Close database connections
4. Flush logs
5. Exit cleanly
```text

## Next Steps

- **[MCP Server Design](mcp-server.md)**: Server implementation details
- **[Tool Registry](tool-registry.md)**: Tool management system
- **[Features](../features.md)**: Individual tool documentation
