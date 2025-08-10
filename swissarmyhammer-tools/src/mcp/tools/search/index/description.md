# Search Index

Index files for semantic search using vector embeddings. Supports glob patterns and individual files. Uses TreeSitter for parsing source code into chunks and fastembed-rs for local embeddings.

## Parameters

- `patterns` (required): Array of glob patterns or specific files to index
  - Supports glob patterns like `"**/*.rs"`, `"src/**/*.py"`
  - Supports specific files like `["file1.rs", "file2.rs"]`
- `force` (optional): Force re-indexing of all files, even if unchanged (default: false)

## Examples

Index all Rust files:
```json
{
  "patterns": ["**/*.rs"],
  "force": false
}
```

Force re-index Python files:
```json
{
  "patterns": ["src/**/*.py"],
  "force": true
}
```

Index specific files:
```json
{
  "patterns": ["file1.rs", "file2.rs", "file3.rs"]
}
```

## Supported Languages

- Rust (.rs)
- Python (.py) 
- TypeScript (.ts)
- JavaScript (.js)
- Dart (.dart)

Files that fail to parse with TreeSitter are indexed as plain text.

## Storage

Index is stored in `.swissarmyhammer/search.db` (DuckDB database).
This file is automatically added to .gitignore.

## Returns

```json
{
  "message": "Successfully indexed 45 files",
  "indexed_files": 45,
  "skipped_files": 3,
  "total_chunks": 234,
  "execution_time_ms": 1234
}
```

## Performance Notes

- First-time indexing downloads the embedding model (~100MB)
- Subsequent runs use cached models for faster startup
- Only changed files are re-indexed unless `force: true`
- Large codebases may take several minutes for initial indexing