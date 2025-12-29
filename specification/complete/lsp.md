# LSP MCP Tool Specification

## Problem Statement

AI coding assistants currently lack deep semantic understanding of code. While grep and AST-based tools find text patterns, they miss the rich information Language Server Protocol (LSP) provides: precise symbol definitions, all references across the codebase, and compiler-level diagnostics.

The goal is to expose LSP capabilities as MCP tools, starting with Rust (via rust-analyzer) and TypeScript (via typescript-language-server), enabling AI agents to:
- Jump to exact definitions (not just text matches)
- Find all true references to a symbol (not just string occurrences)
- Access compiler diagnostics (errors, warnings, lints)

## Background

### Language Server Protocol (LSP)

LSP is a JSON-RPC protocol between editors and language servers. Key capabilities:
- `textDocument/definition` - Go to definition
- `textDocument/references` - Find all references
- `textDocument/publishDiagnostics` - Errors and warnings
- `textDocument/hover` - Type information and docs
- `textDocument/completion` - Code completion

### rust-analyzer

The de-facto Rust language server. Provides:
- Fast incremental analysis
- Full semantic understanding of Rust code
- Project-aware (understands Cargo workspaces)
- Supports all major LSP features

### lsp-bridge Approach

Rather than embedding language servers directly, we bridge to them:
1. Start language server as subprocess (e.g., `rust-analyzer`)
2. Communicate via JSON-RPC over stdin/stdout
3. Translate between MCP tool calls and LSP requests/responses
4. Manage server lifecycle (start, restart, shutdown)

This allows:
- Using existing, well-maintained language servers
- Easy addition of new languages (just configure the server command)
- Server crash isolation from SAH process

## Proposed Approaches

### Approach 1: Direct LSP Integration

Embed an LSP client directly in SwissArmyHammer:

```rust
// In-process LSP client
struct LspBridge {
    servers: HashMap<String, LanguageServer>,  // lang -> server
}

impl LspBridge {
    async fn start_server(&mut self, lang: &str) -> Result<()> {
        let cmd = match lang {
            "rust" => "rust-analyzer",
            "typescript" => "typescript-language-server",
            _ => return Err(UnsupportedLanguage)
        };
        let server = LanguageServer::spawn(cmd).await?;
        self.servers.insert(lang.to_string(), server);
    }
}
```

**Pros:**
- Full control over server lifecycle
- Can optimize communication
- Single binary deployment

**Cons:**
- Complex subprocess management
- Need to handle server crashes/restarts
- Memory overhead per language server

---

### Approach 2: On-Demand Server Management

Start servers lazily when first needed, keep alive with timeout:

```rust
struct LazyLspBridge {
    servers: HashMap<String, ManagedServer>,
    config: LspConfig,
}

struct ManagedServer {
    process: Child,
    client: LspClient,
    last_used: Instant,
}

impl LazyLspBridge {
    async fn get_server(&mut self, lang: &str) -> Result<&mut LspClient> {
        if !self.servers.contains_key(lang) {
            self.start_server(lang).await?;
        }
        let server = self.servers.get_mut(lang).unwrap();
        server.last_used = Instant::now();
        Ok(&mut server.client)
    }

    async fn cleanup_idle(&mut self, timeout: Duration) {
        // Shut down servers idle for > timeout
    }
}
```

**Pros:**
- No upfront cost
- Memory efficient (only active servers)
- Automatic cleanup

**Cons:**
- First request per language is slow (server startup)
- Complexity in lifecycle management

---

### Approach 3: Project-Scoped Servers

One server instance per project root, shared across tools:

```rust
struct ProjectLspManager {
    // project_root -> servers_for_project
    projects: HashMap<PathBuf, ProjectServers>,
}

struct ProjectServers {
    root: PathBuf,
    servers: HashMap<String, LspClient>,  // lang -> client
}
```

**Pros:**
- Matches how language servers actually work (project-scoped)
- Efficient for multi-language projects
- Proper workspace understanding

**Cons:**
- Need to detect project roots
- More complex state management

---

## Recommendation

**Approach 2 (On-Demand Server Management)** combined with elements of Approach 3:

- Start servers lazily on first tool call
- Scope servers to detected project roots (Cargo.toml, package.json, etc.)
- Idle timeout shutdown (e.g., 5 minutes)
- Automatic restart on crash

This balances resource efficiency with proper project understanding.

## MCP Tool Design

### Tool: `lsp`

A single MCP tool with an `operation` parameter, following the same pattern as the `flow` tool.

**Language Detection**: The language server is auto-detected from file extension (e.g., `.rs` → rust-analyzer). No explicit language parameter needed.

**Schema:**
```json
{
  "type": "object",
  "properties": {
    "operation": {
      "type": "string",
      "enum": ["definition", "references", "diagnostics", "hover"],
      "description": "The LSP operation to perform"
    },
    "file_path": {
      "type": "string",
      "description": "Absolute path to the file"
    },
    "line": {
      "type": "integer",
      "description": "Line number (1-based). Required for definition, references, hover.",
      "minimum": 1
    },
    "column": {
      "type": "integer",
      "description": "Column number (1-based). Required for definition, references, hover.",
      "minimum": 1
    },
    "include_declaration": {
      "type": "boolean",
      "description": "Include the declaration in results (references only)",
      "default": true
    },
    "severity_filter": {
      "type": "string",
      "enum": ["error", "warning", "info", "hint", "all"],
      "description": "Filter by minimum severity (diagnostics only)",
      "default": "all"
    },
    "limit": {
      "type": "integer",
      "description": "Maximum number of results to return (diagnostics only)",
      "default": 100
    }
  },
  "required": ["operation", "file_path"]
}
```

---

### Operation: `definition`

Find the definition location of a symbol.

**Required parameters:** `file_path`, `line`, `column`

**Example:**
```json
{
  "operation": "definition",
  "file_path": "/src/main.rs",
  "line": 15,
  "column": 10
}
```

**Response:**
```json
{
  "definitions": [
    {
      "file_path": "/path/to/definition.rs",
      "line": 42,
      "column": 5,
      "preview": "pub fn create_agent(config: &AgentConfig) -> Result<Agent> {"
    }
  ]
}
```

---

### Operation: `references`

Find all references to a symbol.

**Required parameters:** `file_path`, `line`, `column`
**Optional parameters:** `include_declaration`

**Example:**
```json
{
  "operation": "references",
  "file_path": "/src/agent/mod.rs",
  "line": 42,
  "column": 8,
  "include_declaration": true
}
```

**Response:**
```json
{
  "references": [
    {
      "file_path": "/src/main.rs",
      "line": 15,
      "column": 10,
      "preview": "    let agent = create_agent(&config)?;",
      "is_declaration": false
    },
    {
      "file_path": "/src/tests/agent_test.rs",
      "line": 8,
      "column": 18,
      "preview": "        let agent = create_agent(&test_config)?;",
      "is_declaration": false
    }
  ],
  "total_count": 2
}
```

---

### Operation: `diagnostics`

Get compiler diagnostics for a file or project.

**Required parameters:** `file_path`
**Optional parameters:** `severity_filter`, `limit`

**Example:**
```json
{
  "operation": "diagnostics",
  "file_path": "/src/main.rs",
  "severity_filter": "error"
}
```

**Response:**
```json
{
  "diagnostics": [
    {
      "file_path": "/src/main.rs",
      "line": 23,
      "column": 5,
      "end_line": 23,
      "end_column": 15,
      "severity": "error",
      "code": "E0425",
      "message": "cannot find value `undefined_var` in this scope",
      "source": "rust-analyzer",
      "related": [
        {
          "file_path": "/src/main.rs",
          "line": 10,
          "message": "did you mean `defined_var`?"
        }
      ]
    }
  ],
  "summary": {
    "errors": 1,
    "warnings": 3,
    "info": 0,
    "hints": 2
  }
}
```

---

### Operation: `hover`

Get hover information (type, docs) for a symbol.

**Required parameters:** `file_path`, `line`, `column`

**Example:**
```json
{
  "operation": "hover",
  "file_path": "/src/main.rs",
  "line": 15,
  "column": 10
}
```

**Response:**
```json
{
  "contents": {
    "type_info": "fn create_agent(config: &AgentConfig) -> Result<Agent, Error>",
    "documentation": "Creates a new agent from the given configuration.\n\n# Arguments\n* `config` - The agent configuration\n\n# Returns\nA configured Agent instance or an error.",
    "source_module": "swissarmyhammer_agent::factory"
  }
}
```

## Architecture

### Component Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    SwissArmyHammer                          │
│  ┌─────────────────────────────────────────────────────────┤
│  │                   MCP Tools Layer                        │
│  │  ┌─────────────────────────────────────────────────────┐│
│  │  │                    lsp tool                         ││
│  │  │  operation: definition|references|diagnostics|hover ││
│  │  └────────────────────────┬────────────────────────────┘│
│  │                           │                              │
│  │  ┌────────────────────────▼─────────────────────────┐   │
│  │  │              LspBridge (shared state)             │   │
│  │  │  - Server lifecycle management                    │   │
│  │  │  - Request/response correlation                   │   │
│  │  │  - Project root detection                         │   │
│  │  │  - File extension -> language server mapping      │   │
│  │  └────────────────────────┬─────────────────────────┘   │
│  │                           │                              │
└──┴──────────────────────────┼───────────────────────────────┘
                              │ JSON-RPC over stdin/stdout
                              ▼
              ┌───────────────────────────────┐
              │      Language Servers          │
              │  ┌─────────────────────────┐  │
              │  │    rust-analyzer        │  │
              │  └─────────────────────────┘  │
              │  ┌─────────────────────────┐  │
              │  │ typescript-language-    │  │
              │  │ server (future)         │  │
              │  └─────────────────────────┘  │
              └───────────────────────────────┘
```

### LspBridge Core

```rust
/// Manages LSP server connections and provides the bridge API
pub struct LspBridge {
    /// Active server connections, keyed by (project_root, language)
    servers: HashMap<(PathBuf, String), ManagedServer>,

    /// Language to server command mapping
    config: LspConfig,

    /// Idle timeout for server cleanup
    idle_timeout: Duration,
}

pub struct LspConfig {
    /// Language name -> server configuration
    languages: HashMap<String, LanguageServerConfig>,
}

pub struct LanguageServerConfig {
    /// Command to start the server
    command: String,

    /// Command arguments
    args: Vec<String>,

    /// File extensions this server handles
    extensions: Vec<String>,

    /// Project root markers (e.g., Cargo.toml, package.json)
    root_markers: Vec<String>,
}

pub struct ManagedServer {
    process: tokio::process::Child,
    client: LspClient,
    last_used: Instant,
    project_root: PathBuf,
}
```

### Default Configuration

```yaml
# Built-in LSP configuration
lsp:
  idle_timeout_seconds: 300  # 5 minutes

  languages:
    # MVP: Rust
    rust:
      command: "rust-analyzer"
      args: []
      extensions: [".rs"]
      root_markers: ["Cargo.toml", "Cargo.lock"]
      install_hint: "rustup component add rust-analyzer"

    # MVP: TypeScript/JavaScript
    # https://github.com/typescript-language-server/typescript-language-server
    typescript:
      command: "typescript-language-server"
      args: ["--stdio"]
      extensions: [".ts", ".tsx", ".js", ".jsx"]
      root_markers: ["package.json", "tsconfig.json", "jsconfig.json"]
      install_hint: "npm install -g typescript-language-server typescript"
      requires: ["node", "npm"]  # Prerequisites to check

    # Future additions
    python:
      command: "pyright-langserver"
      args: ["--stdio"]
      extensions: [".py"]
      root_markers: ["pyproject.toml", "setup.py", "requirements.txt"]
      install_hint: "npm install -g pyright"
```

### TypeScript Language Server Setup

The TypeScript language server runs as a Node.js subprocess. Requirements:
- Node.js (v14+) installed and in PATH
- npm available for global package installation
- `typescript-language-server` and `typescript` packages installed globally

```bash
# Install prerequisites
npm install -g typescript-language-server typescript
```

The server communicates via stdio and requires a project with `package.json` or `tsconfig.json` to function properly.

## Implementation Plan

### Phase 1: Core Infrastructure

1. **Create LspBridge struct** in new crate `swissarmyhammer-lsp`
   - Server lifecycle management (start, stop, restart)
   - JSON-RPC client implementation
   - Request/response correlation with IDs

2. **Implement project root detection**
   - Walk up from file path looking for root markers
   - Cache detected roots

3. **Add LSP client protocol types**
   - Request/response structures for LSP methods
   - Use existing crate (tower-lsp, lsp-types) or define minimal subset

### Phase 2: MCP Tools

4. **Implement `lsp_definition` tool**
   - Map file position to LSP Position (0-based)
   - Call textDocument/definition
   - Convert LocationLink response to our format
   - Include line preview from file

5. **Implement `lsp_references` tool**
   - Call textDocument/references
   - Group results by file
   - Include context/preview lines

6. **Implement `lsp_diagnostics` tool**
   - Subscribe to textDocument/publishDiagnostics notifications
   - Cache diagnostics per file
   - Support severity filtering

7. **Implement `lsp_hover` tool**
   - Call textDocument/hover
   - Parse markdown content
   - Extract type signature and docs

### Phase 3: Language Server Integration

8. **Test with rust-analyzer**
   - Verify initialization sequence
   - Test on real Rust projects (including SwissArmyHammer itself)
   - Handle workspace/didChangeWatchedFiles notifications

9. **Handle Cargo workspace specifics**
   - Multiple crate project roots
   - Cross-crate references

10. **Test with typescript-language-server**
    - Verify Node.js/npm prerequisite checking
    - Test on real TypeScript projects
    - Handle tsconfig.json and jsconfig.json detection
    - Test with monorepo structures (multiple package.json)

11. **Prerequisite validation**
    - Check for node/npm before starting TypeScript server
    - Provide actionable install instructions on failure
    - Log clear errors when servers fail to start

### Phase 4: Robustness

12. **Server crash recovery**
    - Detect server exit
    - Auto-restart with backoff
    - Re-send open documents

13. **Resource management**
    - Idle timeout cleanup
    - Memory monitoring
    - Maximum server limit

14. **Caching and performance**
    - Cache diagnostic results
    - Debounce rapid requests
    - Batch reference requests

## Error Handling

All errors are logged with helpful messages and actionable suggestions.

### Prerequisite Not Found

For language servers that require runtime dependencies (e.g., TypeScript requires Node.js):

```json
{
  "error": {
    "code": "LSP_PREREQUISITE_MISSING",
    "message": "Node.js not found. TypeScript language server requires Node.js to run.",
    "details": {
      "language": "typescript",
      "missing": "node",
      "suggestion": "Install Node.js from https://nodejs.org/ or via your package manager",
      "checked_paths": ["/usr/local/bin", "/usr/bin", "~/.nvm/versions/node/*/bin"]
    }
  }
}
```

```json
{
  "error": {
    "code": "LSP_PREREQUISITE_MISSING",
    "message": "npm not found. Required to verify TypeScript language server installation.",
    "details": {
      "language": "typescript",
      "missing": "npm",
      "suggestion": "npm is typically installed with Node.js. Reinstall Node.js or install npm separately."
    }
  }
}
```

### Server Not Installed

```json
{
  "error": {
    "code": "LSP_SERVER_UNAVAILABLE",
    "message": "rust-analyzer not found.",
    "details": {
      "language": "rust",
      "command": "rust-analyzer",
      "install_hint": "rustup component add rust-analyzer",
      "search_paths": ["/usr/local/bin", "~/.cargo/bin"]
    }
  }
}
```

```json
{
  "error": {
    "code": "LSP_SERVER_UNAVAILABLE",
    "message": "typescript-language-server not found.",
    "details": {
      "language": "typescript",
      "command": "typescript-language-server",
      "install_hint": "npm install -g typescript-language-server typescript",
      "search_paths": ["/usr/local/bin", "~/.npm-global/bin"]
    }
  }
}
```

### Server Failed to Start

```json
{
  "error": {
    "code": "LSP_SERVER_START_FAILED",
    "message": "typescript-language-server failed to start",
    "details": {
      "language": "typescript",
      "command": "typescript-language-server --stdio",
      "exit_code": 1,
      "stderr": "Error: Cannot find module 'typescript'",
      "suggestion": "The TypeScript package may be missing. Run: npm install -g typescript"
    }
  }
}
```

### Server Crash

```json
{
  "error": {
    "code": "LSP_SERVER_CRASHED",
    "message": "Language server crashed and is restarting",
    "details": {
      "language": "rust",
      "exit_code": 1,
      "stderr": "thread 'main' panicked at...",
      "action": "retry_in_seconds",
      "retry_after": 2
    }
  }
}
```

### No Project Root Found

```json
{
  "error": {
    "code": "LSP_NO_PROJECT_ROOT",
    "message": "No TypeScript project found",
    "details": {
      "file_path": "/tmp/scratch.ts",
      "looked_for": ["package.json", "tsconfig.json", "jsconfig.json"],
      "suggestion": "Create a package.json or tsconfig.json in the project directory"
    }
  }
}
```

### File Not Indexed

```json
{
  "error": {
    "code": "LSP_FILE_NOT_INDEXED",
    "message": "File not yet indexed by language server",
    "details": {
      "file_path": "/src/new_file.rs",
      "suggestion": "Wait for indexing to complete or save the file"
    }
  }
}
```

### Unsupported File Type

```json
{
  "error": {
    "code": "LSP_UNSUPPORTED_FILE",
    "message": "No language server configured for .xyz files",
    "details": {
      "file_path": "/src/data.xyz",
      "extension": ".xyz",
      "supported_extensions": [".rs", ".ts", ".tsx", ".js", ".jsx"]
    }
  }
}
```

## Configuration

### User Configuration

```yaml
# .swissarmyhammer/config.yaml
lsp:
  # Override idle timeout
  idle_timeout_seconds: 600

  # Disable specific languages
  disabled_languages: []

  # Custom language server paths
  server_paths:
    rust-analyzer: "/custom/path/rust-analyzer"

  # Additional language configurations
  languages:
    go:
      command: "gopls"
      args: []
      extensions: [".go"]
      root_markers: ["go.mod", "go.sum"]
```

## Testing Strategy

### Unit Tests

1. **LspBridge lifecycle tests**
   - Server start/stop
   - Idle timeout cleanup
   - Crash recovery

2. **Protocol tests**
   - JSON-RPC encoding/decoding
   - Request/response correlation
   - Notification handling

### Integration Tests

3. **rust-analyzer integration**
   - Test on sample Rust project
   - Verify definition/references accuracy
   - Test diagnostic updates

4. **Multi-project scenarios**
   - Multiple project roots
   - Cross-project references

### Performance Tests

5. **Latency benchmarks**
   - First request (cold start)
   - Subsequent requests (warm)
   - Large project diagnostics

## Success Criteria

1. **Accuracy**: Definition/references match what IDE shows
2. **Latency**: < 500ms for definition lookup after server warm
3. **Reliability**: Automatic recovery from server crashes
4. **Resource efficiency**: Idle servers shut down, < 200MB per server
5. **Developer experience**: Clear error messages, helpful suggestions

## Future Extensions

### Additional Languages (Post-MVP)

- Python (pyright, pylsp)
- Go (gopls)
- Java (jdtls)
- C/C++ (clangd)

### Additional LSP Features

- `lsp_rename` - Rename symbol across project
- `lsp_completion` - Code completion suggestions
- `lsp_signature` - Function signature help
- `lsp_symbols` - Document/workspace symbol search
- `lsp_format` - Code formatting
- `lsp_actions` - Code actions (quick fixes, refactors)

### Integration with Other Tools

- Use diagnostics in `rules_check` for compiler-aware rules
- Enhance `files_grep` with semantic search fallback
- Power intelligent code navigation in workflows

## Open Questions

1. **Server installation**: Should SAH auto-install language servers or require manual installation?
   - Recommendation: Require manual, provide clear install instructions

2. **Multi-language files**: How to handle files with embedded languages (e.g., HTML with JS)?
   - Recommendation: Support primary language only for MVP

3. **Remote development**: Support for LSP over network (remote containers, SSH)?
   - Recommendation: Out of scope for MVP, design for future addition

4. **Caching invalidation**: How long to cache diagnostics? When to refresh?
   - Recommendation: Cache until file change notification, max 30 seconds

5. **Concurrent requests**: Handle multiple simultaneous tool calls to same server?
   - Recommendation: Queue requests per server, LSP supports concurrent requests
