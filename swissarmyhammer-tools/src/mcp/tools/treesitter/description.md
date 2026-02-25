Tree-sitter code intelligence operations for semantic search, AST queries, and duplicate detection.

## Operations

- **search code**: Semantic search for similar code chunks using embeddings
- **query ast**: Execute tree-sitter S-expression queries to find AST patterns
- **find duplicates**: Detect duplicate code clusters using semantic similarity
- **get status**: Check the current status of the code index

## Examples

```json
{"op": "search code", "query": "fn process_request(req: Request) -> Response", "top_k": 5}
```

```json
{"op": "query ast", "query": "(function_item name: (identifier) @name)", "language": "rust"}
```

```json
{"op": "find duplicates", "min_similarity": 0.9}
```

```json
{"op": "get status"}
```
