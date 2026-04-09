<div align="center">

<img src="code-context.png" alt="code-context" width="256" height="256">

# code-context

**Code intelligence for AI agents -- symbols, search, and call graphs.**

</div>

---

code-context gives AI coding agents structural understanding of a codebase. It maintains a `.code-context/` index built from tree-sitter parsing and LSP integration, then exposes symbol lookup, call graph traversal, blast radius analysis, semantic search, and more as MCP tools. Agents navigate code by meaning instead of grepping raw text.

## Why

Agents that rely on text search and file reads waste tokens and miss structure. code-context replaces that with indexed, structural code intelligence:

- **Symbol lookup** -- jump to definitions with fuzzy matching, no file reads needed
- **Call graphs** -- trace who calls what, inbound and outbound, across the whole project
- **Blast radius** -- before changing code, see everything that could break
- **Semantic search** -- find code by meaning, not exact text ("authentication handler" finds `verify_token`)
- **AST queries** -- run tree-sitter S-expression patterns for structural code search
- **Duplicate detection** -- find copy-pasted code across the codebase
- **LSP integration** -- hover, go-to-definition, references, diagnostics, and rename preview
- **Auto-population** -- indexes on startup with no manual steps

## Install

### macOS (Homebrew)

```bash
brew install swissarmyhammer/tap/code-context
```

### Linux

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/swissarmyhammer/swissarmyhammer/releases/latest/download/code-context-cli-installer.sh | sh
```

### From source

```bash
cargo install --git https://github.com/swissarmyhammer/swissarmyhammer code-context-cli
```

Then set up the tool:

```bash
code-context init
```

This registers the MCP server, deploys the code-context skill, and creates the `.code-context/` index in your project.

## Commands

| Command | Description |
|---------|-------------|
| `code-context serve` | Run MCP server over stdio |
| `code-context init [project\|local\|user]` | Install for your agent |
| `code-context deinit [project\|local\|user]` | Remove from agent config |
| `code-context doctor` | Diagnose setup issues |
| `code-context skill` | Deploy code-context skill to .skills/ |

## Operations

All operations follow a verb-noun pattern and can be run directly from the CLI or through the MCP server.

### get -- retrieve resources

| Operation | Description |
|-----------|-------------|
| `get symbol` | Look up symbol locations and source text with fuzzy matching |
| `get callgraph` | Traverse call graph from a starting symbol |
| `get blastradius` | Analyze blast radius of changes to a file or symbol |
| `get status` | Health report with file counts, indexing progress, chunk/edge counts |
| `get definition` | Go to definition with layered resolution (live LSP, LSP index, tree-sitter) |
| `get type-definition` | Go to type definition (live LSP only) |
| `get hover` | Get hover information (type signature, docs) |
| `get references` | Find all references to a symbol |
| `get implementations` | Find implementations of a trait/interface |
| `get code-actions` | Get code actions (quickfixes, refactors) for a range |
| `get inbound-calls` | Find all callers of a function at a given position |
| `get rename-edits` | Preview rename edits without applying them |
| `get diagnostics` | Get errors and warnings for a file |

### search -- find symbols and code

| Operation | Description |
|-----------|-------------|
| `search symbol` | Fuzzy search across all indexed symbols |
| `search code` | Semantic similarity search across code chunks using embeddings |
| `search workspace-symbol` | Live workspace symbol search with layered resolution |

### list -- enumerate resources

| Operation | Description |
|-----------|-------------|
| `list symbols` | List all symbols in a specific file, sorted by start line |

### grep -- pattern matching

| Operation | Description |
|-----------|-------------|
| `grep code` | Regex search across stored code chunks |

### query -- structural search

| Operation | Description |
|-----------|-------------|
| `query ast` | Execute tree-sitter S-expression queries against parsed ASTs |

### find -- detection

| Operation | Description |
|-----------|-------------|
| `find duplicates` | Find code in a file that is duplicated elsewhere in the codebase |

### build -- index management

| Operation | Description |
|-----------|-------------|
| `build status` | Mark files for re-indexing by resetting indexed flags |

### clear -- cleanup

| Operation | Description |
|-----------|-------------|
| `clear status` | Wipe all index data and return stats about what was cleared |

### lsp -- language server management

| Operation | Description |
|-----------|-------------|
| `lsp status` | Show detected languages, their LSP servers, and install status |

### detect -- project detection

| Operation | Description |
|-----------|-------------|
| `detect projects` | Detect project types in the workspace and return language-specific guidelines |

Use `--json` for machine-readable output.

## How It Works

When you run `code-context init` or start the MCP server, code-context creates a `.code-context/` directory at your project root containing a SQLite index. On startup, it automatically discovers source files, parses them with tree-sitter to extract symbols and code chunks, and stores everything in the index. If an LSP server is available for the language, it layers in live type information, hover docs, and reference resolution on top of the tree-sitter baseline.

The index is incremental -- changed files are re-parsed automatically. The agent queries the index through MCP tool calls (or CLI commands), getting structural answers without reading entire files.

## Works With

Claude Code, Cursor, Windsurf, or any MCP-compatible agent.
