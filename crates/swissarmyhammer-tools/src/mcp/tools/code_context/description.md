Code context operations for symbol lookup, search, grep, call graph, and blast radius analysis.

## Operations

- **get symbol**: Get symbol locations and source text from both LSP and tree-sitter indices with multi-tier fuzzy matching
- **search symbol**: Fuzzy search across all indexed symbols with optional kind filter
- **list symbols**: List all symbols in a specific file
- **grep code**: Regex search across stored code chunks
- **get callgraph**: Traverse call graph from a starting symbol
- **get blastradius**: Analyze blast radius of changes to a file or symbol
- **get status**: Health report with file counts, indexing progress, chunk/edge counts
- **rebuild index**: Mark files for re-indexing and synchronously drive the tree-sitter indexer to completion. Returns `files_marked`, `files_indexed`, `chunks_written`, `elapsed_ms`, `layer`, `hint`, and (for layers that include LSP) a `note` calling out that LSP rebuild remains background-driven. The synchronous contract applies to the tree-sitter layer only: `layer=treesitter` (and the tree-sitter portion of `layer=both`) reports real run stats; `layer=lsp` only flips dirty bits for the background LSP worker and the response carries `files_indexed=0, chunks_written=0, elapsed_ms~=0` with a `note` explaining the asynchronous LSP contract — poll `get status` and watch `lsp_indexed_percent` for LSP progress.
- **clear status**: Wipe all index data and return stats about what was cleared
- **lsp status**: Show which languages are detected in the index, their LSP servers, and install status
- **detect projects**: Detect project types in the workspace and return language-specific guidelines

## Examples

```json
{"op": "get symbol", "query": "MyStruct::new", "max_results": 5}
```

```json
{"op": "search symbol", "query": "handler", "kind": "function", "max_results": 10}
```

```json
{"op": "list symbols", "file_path": "src/main.rs"}
```

```json
{"op": "grep code", "pattern": "TODO|FIXME", "max_results": 20}
```

```json
{"op": "get callgraph", "symbol": "process_request", "direction": "outbound"}
```

```json
{"op": "get blastradius", "file_path": "src/server.rs", "max_hops": 3}
```

```json
{"op": "get status"}
```

```json
{"op": "rebuild index", "layer": "both"}
```

```json
{"op": "clear status"}
```

```json
{"op": "lsp status"}
```

```json
{"op": "detect projects"}
```

```json
{"op": "detect projects", "path": "/path/to/project", "max_depth": 5, "include_guidelines": false}
```
