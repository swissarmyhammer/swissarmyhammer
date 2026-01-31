Perform semantic code search to find similar code chunks using embeddings.

This tool uses tree-sitter parsing combined with code embeddings to find semantically similar code across the indexed codebase. It's useful for finding code patterns, similar implementations, or related functionality.

## Examples

Find code similar to a function signature:
```json
{
  "query": "fn process_request(req: Request) -> Response"
}
```

Search with custom parameters:
```json
{
  "query": "async fn fetch_data",
  "top_k": 5,
  "min_similarity": 0.8
}
```

Search in a specific workspace:
```json
{
  "query": "error handling pattern",
  "path": "/path/to/project"
}
```

## Returns

Returns matching code chunks with similarity scores, file locations, and the actual code content.
