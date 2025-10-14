Index files for semantic search using vector embeddings.

## Parameters

- `patterns` (required): Array of glob patterns or files to index (e.g., `["**/*.rs"]`)
- `force` (optional): Force re-indexing of all files (default: false)

## Examples

```json
{
  "patterns": ["**/*.rs"],
  "force": false
}
```

## Returns

Returns indexed file count, skipped files, total chunks, and execution time. Index stored in `.swissarmyhammer/search.db`. Supports Rust, Python, TypeScript, JavaScript, and Dart.
