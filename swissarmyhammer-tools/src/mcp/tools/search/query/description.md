Perform semantic search across indexed files using vector similarity.

## Examples

```json
{
  "query": "error handling",
  "limit": 10
}
```

## Returns

Returns ranked results with file paths, chunk text, line numbers, similarity scores, language, and excerpts. Files must be indexed first using search_index tool.
