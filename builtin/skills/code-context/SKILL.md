---
name: code-context
description: Code intelligence using the unified code context index. Use this skill when exploring a codebase, finding symbols, tracing call graphs, assessing blast radius, or searching code by pattern. This is the primary tool for understanding code structure and relationships. Agents should prefer this over raw file reads when navigating unfamiliar code.
metadata:
  author: "swissarmyhammer"
  version: "1.0"
---

# Code Context

The `code_context` tool provides code intelligence powered by tree-sitter parsing, LSP symbol indexing, and call graph analysis. It is the primary tool for exploring and understanding a codebase.

**Use this tool first** when you need to understand code structure, find implementations, trace dependencies, or assess the impact of changes. It is faster and more accurate than grepping files or reading directory trees.

## Operations

### get status

Check if the index is ready before running queries. Always do this first if you're unsure whether indexing is complete.

```json
{"op": "get status"}
```

Returns: file counts, TS/LSP indexed percentages, chunk/edge counts, dirty file count.

### find symbol

Find exact symbol locations by name. Returns file path, line, and character coordinates. Use this when you know the symbol name and want to jump to it.

```json
{"op": "find symbol", "name": "process_request"}
```

```json
{"op": "find symbol", "name": "AuthService::new"}
```

**Parameters:**
- `name` (required): Symbol name or qualified path (e.g., `"MyStruct"`, `"auth::AuthService::new"`)

**When to use:**
- Jumping to a definition you know the name of
- Finding where a struct, function, or method is defined
- Locating symbols from error messages or stack traces

### get symbol

Get full symbol source text with multi-tier fuzzy matching. Tries exact match first, then suffix, case-insensitive, and finally fuzzy. Use this when you want to read the implementation.

```json
{"op": "get symbol", "query": "MyStruct::new"}
```

```json
{"op": "get symbol", "query": "process_request", "max_results": 5, "include_source": true}
```

**Parameters:**
- `query` (required): Symbol name or partial match
- `max_results`: Maximum results (default: 10)
- `include_source`: Include source text in results (default: true)

**When to use:**
- Reading the implementation of a function or method
- Understanding what a type looks like
- Getting source code for review or analysis

### search symbol

Fuzzy search across all indexed symbols. Use this when you have a vague idea of the name or want to explore what exists.

```json
{"op": "search symbol", "query": "handler", "kind": "function", "max_results": 10}
```

**Parameters:**
- `query` (required): Search query (fuzzy matched)
- `kind`: Filter by symbol kind (e.g., `"function"`, `"struct"`, `"method"`, `"class"`)
- `max_results`: Maximum results (default: 10)

**When to use:**
- Exploring what functions or types exist in a domain area
- Finding symbols when you only remember part of the name
- Discovering available APIs

### list symbols

List all symbols in a specific file. Use this to get an overview of what a file contains.

```json
{"op": "list symbols", "file_path": "src/main.rs"}
```

**Parameters:**
- `file_path` (required): Relative path to the file

**When to use:**
- Getting a table of contents for a file
- Understanding the structure of a module
- Finding all definitions in a file before reading it

### grep code

Regex search across stored code chunks. Use this for textual pattern matching when you need exact string or regex matches.

```json
{"op": "grep code", "pattern": "TODO|FIXME", "max_results": 20}
```

```json
{"op": "grep code", "pattern": "unsafe\\s*\\{", "language": "rust"}
```

```json
{"op": "grep code", "pattern": "process_", "file_pattern": "src/handlers/"}
```

**Parameters:**
- `pattern` (required): Regex pattern to search for
- `max_results`: Maximum results (default: 50)
- `language`: Filter by programming language
- `file_pattern`: Filter by file path pattern

**When to use:**
- Finding all uses of a pattern (TODOs, unsafe blocks, specific API calls)
- Searching for string literals or error messages
- Scoping searches to a directory or language

### get callgraph

Traverse the call graph from a starting symbol. Shows who calls what (or who is called by what). Uses LSP edges when available, falls back to tree-sitter heuristic edges.

```json
{"op": "get callgraph", "symbol": "process_request", "direction": "inbound", "max_depth": 2}
```

```json
{"op": "get callgraph", "symbol": "AuthService::validate", "direction": "both", "max_depth": 3}
```

**Parameters:**
- `symbol` (required): Symbol name or qualified path
- `direction`: `"inbound"` (callers), `"outbound"` (callees), or `"both"` (default: `"outbound"`)
- `max_depth`: Traversal depth 1-5 (default: 2)

**When to use:**
- Understanding how a function is used (inbound)
- Understanding what a function depends on (outbound)
- Tracing execution flow through the codebase
- Assessing whether a function is safe to change

### get blastradius

Analyze the blast radius of changes to a file or symbol. Shows how many symbols and files are transitively affected, organized by hop distance.

```json
{"op": "get blastradius", "file_path": "src/server.rs", "max_hops": 3}
```

```json
{"op": "get blastradius", "file_path": "src/auth.rs", "symbol_name": "validate_token", "max_hops": 5}
```

**Parameters:**
- `file_path` (required): File to analyze
- `symbol_name`: Specific symbol within the file (optional, defaults to all symbols in file)
- `max_hops`: Maximum hop distance 1-10 (default: 3)

**When to use:**
- Before making changes, understanding what could break
- Estimating the scope of a refactoring
- Deciding whether a change needs a broader review
- Prioritizing test coverage

### build status

Mark files for re-indexing. Use this to trigger a reindex after making changes.

```json
{"op": "build status", "layer": "both"}
```

**Parameters:**
- `layer`: `"treesitter"`, `"lsp"`, or `"both"`

### clear status

Wipe all index data and start fresh. Use only when the index is corrupted or you need a full rebuild.

```json
{"op": "clear status"}
```

## Semantic Diff (git tool)

The `git` tool also integrates with code context for semantic diffs:

```json
{"op": "get diff"}
```

```json
{"op": "get diff", "left": "src/main.rs@HEAD~1", "right": "src/main.rs"}
```

```json
{"op": "get diff", "left_text": "fn foo() {}", "right_text": "fn foo(x: i32) {}", "language": "rust"}
```

This returns entity-level changes (Added, Modified, Deleted, Moved, Renamed) rather than line-level diffs.

## Workflows

### Exploring unfamiliar code

1. `get status` -- confirm the index is ready
2. `list symbols` on key files to get the lay of the land
3. `search symbol` with domain keywords to find relevant code
4. `get symbol` to read implementations
5. `get callgraph` with `direction: "outbound"` to understand dependencies

### Understanding a function before changing it

1. `find symbol` to locate it
2. `get symbol` to read its source
3. `get callgraph` with `direction: "inbound"` to see all callers
4. `get blastradius` to understand the impact of changes

### Investigating a bug

1. `grep code` to find relevant error messages or patterns
2. `find symbol` on suspicious functions
3. `get callgraph` with `direction: "both"` to trace data flow
4. `get symbol` on each function in the chain

### Preparing a change

1. `get blastradius` on the file/symbol you plan to change
2. `get callgraph` inbound to understand who depends on it
3. Make the change
4. `git get diff` to see entity-level impact
5. `get blastradius` again to confirm the scope matches expectations
