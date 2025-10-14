Perform semantic search across indexed files using vector similarity.

## Parameters

- `query` (required): Search query string
- `limit` (optional): Number of results to return (default: 10)

## Examples

```json
{
  "query": "error handling",
  "limit": 10
}
```

## Returns

Returns ranked results with file paths, chunk text, line numbers, similarity scores, language, and excerpts. Files must be indexed first using search_index tool.
