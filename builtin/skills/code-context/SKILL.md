---
name: code-context
profiles:
  - code-context
description: >-
  Code context operations for symbol lookup, search, grep, call graph, and blast
  radius analysis. Use this skill when the user says "blast radius", "who calls this",
  "find symbol", "find references", "go to definition", "symbol lookup",
  "callgraph", "find callers", "what calls this function", or "what's affected
  if I change this". Also use this skill proactively before you modify code, to
  understand structure, dependencies, and impact — list symbols and get callgraph
  (inbound) to see who calls a symbol, and get blastradius when you change a
  shared symbol's signature. This skill provides indexed, structural code
  intelligence that is faster and more precise than raw text search.
license: MIT OR Apache-2.0
compatibility: This skill requires the `code_context` MCP tool for indexed symbol lookup, grep, callgraph, and blast-radius operations.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Code Context

Structural code intelligence for AI agents. It offers indexed symbol lookup, callgraph traversal, blast-radius analysis, semantic search, and AST queries. It uses tree-sitter, plus optional live LSP.

The `code_context` tool provides structural code intelligence: indexed symbol
lookup, call graphs, and blast-radius analysis, backed by tree-sitter and live
LSP. It is not optional background work. Lean on the cheap defaults below as
part of doing the task, not as extra work on top of it: use `list symbols` and
`get symbol` instead of reading whole files, and use `get callgraph` (inbound)
to find callers.

Do not read files top to bottom. Do not guess where a symbol lives or who
calls it. `code_context` answers those questions precisely and cheaply.

- **Before reading a file** — run `{"op": "list symbols", "file_path": "<file>"}` for a
  table of contents, then `{"op": "get symbol", "query": "<symbol>"}` to pull only
  the code you need. Reading a whole file is the fallback, not the default.
- **Before changing a symbol** — run `{"op": "get callgraph", "symbol": "<symbol>",
  "direction": "inbound"}` to see who calls it. When the symbol is shared or public
  and you are changing its signature, also run `{"op": "get blastradius", "file_path":
  "<file>"}` for the wider dependent set. If the result surprises you, you do not yet
  understand the change well enough to make it.
- **After you change a signature or behavior** — re-check the inbound callers the
  blast radius surfaced, and confirm each one still holds.
- **When a test or build fails** — run `{"op": "get callgraph", "symbol": "<failing
  symbol>"}` to see what the failure actually reaches, before you start fixing it.
- **To find code by name or pattern** — use `search symbol` or `grep code` instead of
  raw text search; they query the index, with kind and language filters.

If `{"op": "get status"}` shows indexing is incomplete, the live LSP ops
(`get definition`, `get hover`, `get references`, `search workspace_symbol`) still
work immediately — do not wait on the index. If callgraph or blast radius comes
back empty for code that clearly compiles, the language server is missing or
warming up: check `{"op": "lsp status"}` and invoke `/lsp` if needed.

Fall back to raw Read/Grep/Glob only for non-code files (TOML, YAML, Markdown),
string literals and config values not in the symbol index, or to confirm exact
syntax once code_context has already given you the location.

## When to Use

- **Before modifying code**: use `get callgraph` (inbound) to know who calls the target before you rename it or change its signature; use `get blastradius` for the wider dependent set when you change a shared symbol's signature.
- **Navigating**: `get symbol` jumps to a definition, `list symbols` gives a file overview, `search symbol` does a fuzzy name search.
- **Pattern search**: `grep code` runs a regex search with language and file filters.
- **Meaning search**: `search code` finds results by semantic similarity.
- **Health checks**: `get status` checks indexing, `lsp status` checks servers, `detect projects` reports project types and build commands.

## Operations

### get symbol

```json
{"op": "get symbol", "query": "MyStruct::new", "max_results": 5}
```

Jump to a definition with source context. It uses multi-tier fuzzy matching and supports qualified paths. Use this to avoid whole-file reads.

### search symbol

```json
{"op": "search symbol", "query": "handler", "kind": "function", "max_results": 10}
```

Searches by partial name, fuzzily. Kinds include function, method, struct, class, interface, and module. Use this to avoid whole-file reads.

### list symbols

```json
{"op": "list symbols", "file_path": "src/main.rs"}
```

Gives a file overview before you read it. Lets you target specific symbols with `get symbol` instead of reading the whole file.

### grep code

```json
{"op": "grep code", "pattern": "unsafe\\s*\\{", "language": ["rs"], "max_results": 20}
```

Runs a regex search over indexed chunks. Filter by language extension or specific path. Use this instead of built-in Grep tools, and instead of any bash or shell command.

### search code

```json
{"op": "search code", "query": "authentication handler", "top_k": 5}
```

Matches by meaning, using semantic similarity, not exact text.

### get callgraph

```json
{"op": "get callgraph", "symbol": "process_request", "direction": "inbound", "max_depth": 2}
```

- **inbound**: who calls this symbol — use before you change a signature
- **outbound**: what this symbol calls — the implementation flow
- **both**: the full neighborhood — the impact

### get blastradius

```json
{"op": "get blastradius", "file_path": "src/server.rs", "max_hops": 3}
```

This is the transitive set of files and symbols a change affects. Reach for it when you change a shared or public symbol's signature and need its dependents — it is not a gate on every edit. It is built from LSP call edges, so `edges: []` is common on compiling code when LSP is missing or warming up (see Troubleshooting); an empty result does not mean "no impact", so do not gate work on it — fall back to inbound `get callgraph` and targeted reads.

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

Tree-sitter S-expression queries. These give structural search beyond regex.

### get status

```json
{"op": "get status"}
```

Reports indexing progress. Run this first if you are unsure whether the index is ready.

### lsp status

```json
{"op": "lsp status"}
```

Reports LSP server health per language. If a server is missing, follow the install hint.

### detect projects

```json
{"op": "detect projects"}
```

Reports project types, build and test commands, and coding guidelines. Run this early to learn the project's conventions.

## Workflow Patterns

### Before modifying code

1. Run `list symbols` on the target file
2. Run `get symbol` to read the function or struct
3. Run `get callgraph` (inbound) on the symbol to find its callers
4. If you are changing a shared or public symbol's signature, run `get blastradius` on the file for the wider dependent set (skip this when it returns empty edges)
5. Make the changes
6. Re-check the callers for compatibility

### Exploring unfamiliar code

1. Run `detect projects` for the project type and conventions
2. Run `get status` to verify the index
3. Run `search symbol` with broad queries to discover key types
4. Run `get callgraph` (outbound) on entry points to trace the flow
5. Run `list symbols` on files of interest, before you read them

### Bug fixes

1. Run `grep code` for the error message or pattern
2. Run `get symbol` for the relevant function
3. Run `get callgraph` (inbound) to trace how execution reaches it
4. Run `get blastradius` to verify the fix will not break other code

## Troubleshooting

### `search symbol` / `get symbol` returns nothing for a symbol you know exists

The index has not finished. On a fresh workspace, `CodeContextWorkspace::open()` runs `startup_cleanup()` then spawns a background worker. Until it finishes, queries see an empty or partial index.

```json
{"op": "get status"}
```

If `files_pending > 0`, wait and poll. Report a symbol as missing only when `files_pending == 0`.

### `get status` shows `files_indexed: 0` and `files_pending: 0` on a non-empty repo

Startup cleanup did not run — usually a stale leader lock from a process that exited uncleanly. The reader-side workspace never re-scans on its own.

```json
{"op": "rebuild index", "layer": "both"}
```

Poll `get status` until `files_pending: 0`. If the problem persists, wipe and rebuild:

```json
{"op": "clear status"}
```

Restart the MCP server so `open()` runs cleanup as leader.

### `get callgraph` / `get blastradius` returns `edges: []` on visible compiling code

Call edges come from LSP. If LSP is missing or warming up, `lsp_call_edges` is empty and traversal degrades to a single node.

`{"op": "lsp status"}` — confirm it is installed and healthy. If it is missing, follow the install hint (or run `/lsp`), wait for the initial scan, and re-run once `get status` shows complete.

### `grep code` returns nothing although `rg` finds it on disk

`grep code` searches **stored chunks**, not the file system. Files modified outside the MCP session are not auto-invalidated (the file watcher is currently a `FileEvent` enum without an active watcher).

```json
{"op": "rebuild index", "layer": "treesitter"}
```

For one-off live searches, fall back to Grep or ripgrep.
