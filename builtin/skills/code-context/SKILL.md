---
name: code-context
description: >-
  Code context operations for symbol lookup, search, grep, call graph, and blast
  radius analysis. Use when the user says "blast radius", "who calls this",
  "find symbol", "find references", "go to definition", "symbol lookup",
  "callgraph", "find callers", "what calls this function", or "what's affected
  if I change this". Also use proactively before modifying code to understand
  structure, dependencies, and impact — list symbols, get callgraph (inbound),
  and get blastradius before touching any function, type, or file. Provides
  indexed, structural code intelligence that is faster and more precise than
  raw text search.
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool  for indexed symbol lookup, grep, callgraph, and blast-radius operations.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Code Context

Structural code intelligence for AI coding agents. Provides indexed symbol lookup,
call graph traversal, blast radius analysis, semantic search, and AST queries.
Backed by tree-sitter parsing and optional live LSP integration.

## When to Use

- **Before modifying code**: Use `get blastradius` to understand what depends on the
  file or symbol you are changing. Use `get callgraph` (inbound) to see who calls a
  function before renaming or changing its signature.
- **Navigating a codebase**: Use `get symbol` to jump to definitions, `list symbols`
  to get a file overview, `search symbol` for fuzzy name searches.
- **Finding code by pattern**: Use `grep code` for regex searches across indexed
  chunks with language and file filters.
- **Finding code by meaning**: Use `search code` for semantic similarity search when
  you do not know the exact text.
- **Checking project health**: Use `get status` to verify indexing progress, `lsp
  status` to check language server availability, `detect projects` to discover
  project types and build commands.

## Operations

### get symbol

Look up symbol locations and source text with multi-tier fuzzy matching.

```json
{"op": "get symbol", "query": "MyStruct::new", "max_results": 5}
```

Use when you know the symbol name and want to jump to its definition with source
context. Supports qualified paths like `module::Type::method`.

### search symbol

Fuzzy search across all indexed symbols with optional kind filter.

```json
{"op": "search symbol", "query": "handler", "kind": "function", "max_results": 10}
```

Use when you want to discover symbols by partial name. Filter by kind to narrow
results: function, method, struct, class, interface, module, etc.

### list symbols

List all symbols in a specific file, sorted by start line.

```json
{"op": "list symbols", "file_path": "src/main.rs"}
```

Use before reading a file to get a structural overview. This saves context by
letting you target specific symbols with `get symbol` instead of reading entire
files.

### grep code

Regex search across stored code chunks with language and file filters.

```json
{"op": "grep code", "pattern": "unsafe\\s*\\{", "language": ["rs"], "max_results": 20}
```

Use for exact pattern matching. Supports full regex syntax. Filter by language
extensions or specific file paths to narrow scope.

### search code

Semantic similarity search across code chunks using embeddings.

```json
{"op": "search code", "query": "authentication handler", "top_k": 5}
```

Use when you are looking for code by meaning rather than exact text. "authentication
handler" will match login processing code even if the word "authentication" does not
appear in the source.

### get callgraph

Traverse call graph from a starting symbol.

```json
{"op": "get callgraph", "symbol": "process_request", "direction": "inbound", "max_depth": 2}
```

Directions:
- **inbound**: Who calls this symbol? Use before changing a function signature.
- **outbound**: What does this symbol call? Use to understand implementation flow.
- **both**: Full neighborhood. Use for impact analysis.

### get blastradius

Analyze blast radius of changes to a file or symbol.

```json
{"op": "get blastradius", "file_path": "src/server.rs", "max_hops": 3}
```

Returns the transitive set of files and symbols that could be affected by a change.
**Always run this before making changes** to understand the full impact.

Optionally narrow to a specific symbol within the file:

```json
{"op": "get blastradius", "file_path": "src/server.rs", "symbol": "handle_request", "max_hops": 2}
```

### find duplicates

Find code in a file that is duplicated elsewhere in the codebase.

```json
{"op": "find duplicates", "file_path": "src/handlers.rs", "min_similarity": 0.85}
```

Use when refactoring to identify copy-pasted code that should be consolidated.

### query ast

Execute tree-sitter S-expression queries against parsed ASTs for structural search.

```json
{"op": "query ast", "query": "(function_item name: (identifier) @name)", "language": "rust"}
```

Use for structural queries that regex cannot express, such as finding all functions
with a specific parameter pattern or all structs implementing a trait.

### get status

Health report with file counts, indexing progress, chunk and edge counts.

```json
{"op": "get status"}
```

Run this first if unsure whether indexing is complete. Shows how many files are
indexed, pending, and total chunk/edge counts.

### lsp status

Show which languages are detected in the index, their LSP servers, and install status.

```json
{"op": "lsp status"}
```

Use when live LSP operations return degraded results. If a server is missing, follow
the install hint to fix it.

### detect projects

Detect project types in the workspace and return language-specific guidelines.

```json
{"op": "detect projects"}
```

Returns project types, build commands, test commands, and coding guidelines. Call
early in a session to understand the project before making changes.

## Workflow Patterns

### Before Modifying Code

1. `list symbols` on the target file to get an overview
2. `get symbol` to read the specific function or struct you plan to change
3. `get blastradius` on the file to understand what could break
4. `get callgraph` (inbound) on the symbol to see all callers
5. Make your changes
6. Re-check callers to ensure compatibility

### Exploring an Unfamiliar Codebase

1. `detect projects` to learn the project type and conventions
2. `get status` to verify the index is populated
3. `search symbol` with broad queries to discover key types
4. `get callgraph` (outbound) on entry points to trace execution flow
5. `list symbols` on files of interest before reading them

### Finding and Fixing Bugs

1. `grep code` to find the error message or pattern in source
2. `get symbol` to jump to the relevant function
3. `get callgraph` (inbound) to trace how execution reaches the bug
4. `get blastradius` to verify your fix will not break other code

## Troubleshooting

### Error: `search symbol` or `get symbol` returns no results for a symbol you know exists

- **Cause**: The index has not finished populating yet. On a fresh workspace, `CodeContextWorkspace::open()` runs `startup_cleanup()` which discovers files, then spawns a background worker that parses and writes chunks. Until the worker finishes, queries see an empty or partial index.
- **Solution**: Check progress with:
  ```json
  {"op": "get status"}
  ```
  The response includes `files_indexed`, `files_pending`, and `chunk_count`. If `files_pending > 0`, wait and poll again. Only report the symbol as missing once `files_pending == 0` and your query still returns nothing.

### Error: `get status` shows `files_indexed: 0` and `files_pending: 0` on a non-empty repo

- **Cause**: Startup cleanup did not run, usually because another process held the leader lock and exited uncleanly, leaving a stale `.code-context/` state. The reader-side workspace never re-scans the filesystem on its own.
- **Solution**: Force a re-scan by resetting the indexed flags, then let the worker re-process:
  ```json
  {"op": "build status", "layer": "both"}
  ```
  Follow with `{"op": "get status"}` until `files_pending` reaches 0. If the problem persists, wipe the index and rebuild from scratch:
  ```json
  {"op": "clear status"}
  ```
  Then restart the MCP server so `open()` runs `startup_cleanup()` as the leader.

### Error: `get callgraph` or `get blastradius` returns `edges: []` for code you can see compiling

- **Cause**: Call edges come from the LSP layer (not tree-sitter). If the LSP server for that language is missing or still warming up, `lsp_call_edges` is empty and graph traversal degrades to a single-node result.
- **Solution**: Run `{"op": "lsp status"}` to confirm the server is installed and healthy. If it is missing, follow the install hint (or invoke the `lsp` skill) and wait for the initial scan. Re-run the callgraph query after `get status` shows indexing complete.

### Error: `grep code` returns nothing even though `rg` finds matches on disk

- **Cause**: `grep code` searches **stored chunks**, not the filesystem. New or modified files that have not been re-indexed yet are invisible to it. The file-watcher is currently a `FileEvent` enum without an active watcher, so edits made outside the MCP session do not auto-invalidate.
- **Solution**: Force re-indexing of the tree-sitter layer, then wait for it to settle:
  ```json
  {"op": "build status", "layer": "treesitter"}
  ```
  For one-off string searches where you need live filesystem results, fall back to Grep/ripgrep directly.
