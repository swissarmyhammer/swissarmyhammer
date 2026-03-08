Code context operations for symbol lookup, search, grep, call graph, and blast radius analysis.

## Operations

- **find symbol**: Find symbol locations by exact name match (file, line, char coordinates)
- **get symbol**: Get symbol source text with multi-tier fuzzy matching
- **search symbol**: Fuzzy search across all indexed symbols with optional kind filter
- **list symbols**: List all symbols in a specific file
- **grep code**: Regex search across stored code chunks
- **get callgraph**: Traverse call graph from a starting symbol
- **get blastradius**: Analyze blast radius of changes to a file or symbol
- **get status**: Health report with file counts, indexing progress, chunk/edge counts
- **build status**: Mark files for re-indexing by resetting indexed flags
- **clear status**: Wipe all index data and return stats about what was cleared

## Examples

```json
{"op": "find symbol", "name": "process_request"}
```

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
{"op": "get callgraph", "symbol": "process_request", "direction": "outbound", "max_depth": 3}
```

```json
{"op": "get blastradius", "file_path": "src/server.rs", "max_hops": 3}
```

```json
{"op": "get status"}
```

```json
{"op": "build status", "layer": "both"}
```

```json
{"op": "clear status"}
```
