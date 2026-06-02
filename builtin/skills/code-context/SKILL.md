---
name: code-context
profiles:
  - code-context
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

Structural code intelligence for AI agents — indexed symbol lookup, callgraph traversal, blast-radius analysis, semantic search, AST queries. Tree-sitter + optional live LSP.

## When to Use

- **Before modifying code**: `get blastradius` to know what depends on the target; `get callgraph` (inbound) before renaming or changing signatures.
- **Navigating**: `get symbol` (jump to definition), `list symbols` (file overview), `search symbol` (fuzzy name).
- **Pattern search**: `grep code` (regex with language/file filters).
- **Meaning search**: `search code` (semantic similarity).
- **Health checks**: `get status` (indexing), `lsp status` (servers), `detect projects` (types + build commands).

## Operations

### get symbol

```json
{"op": "get symbol", "query": "MyStruct::new", "max_results": 5}
```

Jump to definition with source context. Multi-tier fuzzy matching, supports qualified paths.

### search symbol

```json
{"op": "search symbol", "query": "handler", "kind": "function", "max_results": 10}
```

Fuzzy by partial name. Kinds: function, method, struct, class, interface, module, etc.

### list symbols

```json
{"op": "list symbols", "file_path": "src/main.rs"}
```

File overview before reading. Lets you target specific symbols with `get symbol` instead of reading the whole file.

### grep code

```json
{"op": "grep code", "pattern": "unsafe\\s*\\{", "language": ["rs"], "max_results": 20}
```

Regex over indexed chunks. Filter by language extensions or specific paths.

### search code

```json
{"op": "search code", "query": "authentication handler", "top_k": 5}
```

Semantic similarity — matches by meaning, not exact text.

### get callgraph

```json
{"op": "get callgraph", "symbol": "process_request", "direction": "inbound", "max_depth": 2}
```

- **inbound**: who calls this (use before signature changes)
- **outbound**: what this calls (implementation flow)
- **both**: full neighborhood (impact)

### get blastradius

```json
{"op": "get blastradius", "file_path": "src/server.rs", "max_hops": 3}
```

Transitive set of files/symbols affected by a change. **Always run before modifying.**

Narrow to a symbol:

```json
{"op": "get blastradius", "file_path": "src/server.rs", "symbol": "handle_request", "max_hops": 2}
```

### find duplicates

```json
{"op": "find duplicates", "file_path": "src/handlers.rs", "min_similarity": 0.85}
```

### query ast

```json
{"op": "query ast", "query": "(function_item name: (identifier) @name)", "language": "rust"}
```

Tree-sitter S-expression queries — structural search beyond regex.

### get status

```json
{"op": "get status"}
```

Indexing progress. Run first if unsure whether the index is ready.

### lsp status

```json
{"op": "lsp status"}
```

LSP server health per language. Missing? Follow the install hint.

### detect projects

```json
{"op": "detect projects"}
```

Project types, build/test commands, coding guidelines. Run early to learn conventions.

## Workflow Patterns

### Before modifying code

1. `list symbols` on the target file
2. `get symbol` to read the function/struct
3. `get blastradius` on the file
4. `get callgraph` (inbound) on the symbol
5. Make changes
6. Re-check callers for compatibility

### Exploring unfamiliar code

1. `detect projects` for type/conventions
2. `get status` to verify the index
3. `search symbol` with broad queries → discover key types
4. `get callgraph` (outbound) on entry points → trace flow
5. `list symbols` on files of interest before reading

### Bug fixes

1. `grep code` for the error message or pattern
2. `get symbol` for the relevant function
3. `get callgraph` (inbound) to trace how execution reaches it
4. `get blastradius` to verify the fix won't break other code

## Troubleshooting

### `search symbol` / `get symbol` returns nothing for a symbol you know exists

Index hasn't finished. On a fresh workspace, `CodeContextWorkspace::open()` runs `startup_cleanup()` then spawns a background worker. Until it finishes, queries see an empty/partial index.

```json
{"op": "get status"}
```

If `files_pending > 0`, wait and poll. Only report missing when `files_pending == 0`.

### `get status` shows `files_indexed: 0` and `files_pending: 0` on a non-empty repo

Startup cleanup didn't run — usually a stale leader lock from an uncleanly exited process. The reader-side workspace never re-scans on its own.

```json
{"op": "rebuild index", "layer": "both"}
```

Poll `get status` until `files_pending: 0`. Persistent → wipe and rebuild:

```json
{"op": "clear status"}
```

Restart the MCP server so `open()` runs cleanup as leader.

### `get callgraph` / `get blastradius` returns `edges: []` on visible compiling code

Call edges come from LSP. If LSP is missing or warming up, `lsp_call_edges` is empty and traversal degrades to a single node.

`{"op": "lsp status"}` — confirm installed/healthy. Missing → follow the install hint (or `/lsp`), wait for initial scan, re-run after `get status` shows complete.

### `grep code` returns nothing although `rg` finds it on disk

`grep code` searches **stored chunks**, not the filesystem. Files modified outside the MCP session aren't auto-invalidated (the file-watcher is currently a `FileEvent` enum without an active watcher).

```json
{"op": "rebuild index", "layer": "treesitter"}
```

For one-off live searches, fall back to Grep/ripgrep.
