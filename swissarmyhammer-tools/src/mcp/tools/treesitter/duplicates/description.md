Detect duplicate or similar code across the codebase using semantic similarity.

This tool identifies code duplication by comparing semantic embeddings of code chunks. It can find both exact duplicates and semantically similar code that may have been copy-pasted and slightly modified.

## Examples

Find all duplicate code clusters in the project:
```json
{}
```

Find duplicates with stricter similarity threshold:
```json
{
  "min_similarity": 0.95,
  "min_chunk_bytes": 150
}
```

Find code similar to chunks in a specific file:
```json
{
  "file": "src/handlers/user.rs"
}
```

## Returns

Returns clusters of similar code with:
- Average similarity score for the cluster
- File locations and line numbers
- The actual code content for each duplicate
