# search - Semantic Code Search

The `search` command provides powerful semantic search functionality for indexing and searching code files using vector embeddings and TreeSitter parsing.

## Synopsis

```bash
swissarmyhammer search [SUBCOMMAND] [OPTIONS]
```

## Description

SwissArmyHammer's search system uses advanced vector embeddings and TreeSitter code parsing to provide semantic code search capabilities. Index your codebase and perform intelligent queries that understand code semantics, not just keyword matching.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| [`index`](#index) | Index files for semantic search using TreeSitter |
| [`query`](#query) | Perform semantic search with vector similarity |

---

## index

Index files for semantic search using vector embeddings and TreeSitter parsing.

### Usage

```bash
swissarmyhammer search index <PATTERNS>... [OPTIONS]
```

### Arguments

- `<PATTERNS>...` - Glob patterns or files to index (e.g., "**/*.rs", "src/**/*.py")

### Options

- `--force` - Force re-indexing of all files, even if unchanged

### Supported Languages

- Rust (.rs)
- Python (.py) 
- TypeScript (.ts)
- JavaScript (.js)
- Dart (.dart)

Files that fail to parse with TreeSitter are indexed as plain text.

### Examples

```bash
# Index all Rust files
swissarmyhammer search index "**/*.rs"

# Index multiple file types
swissarmyhammer search index "**/*.rs" "**/*.py" "**/*.ts"

# Force re-index all files
swissarmyhammer Search index "**/*.rs" --force

# Index specific files
swissarmyhammer search index "src/main.rs" "src/lib.rs"
```

---

## query

Perform semantic search with vector similarity across the indexed codebase.

### Usage

```bash
swissarmyhammer search query <QUERY> [OPTIONS]
```

### Arguments

- `<QUERY>` - Search query string

### Options

- `--limit <N>` - Number of results to return (default: 10)

### Examples

```bash
# Basic semantic search
swissarmyhammer search query "error handling"

# Search for async patterns  
swissarmyhammer search query "async function implementation"

# Search with limited results
swissarmyhammer search query "database connection" --limit 5

# Search for specific patterns
swissarmyhammer search query "trait implementation"
swissarmyhammer search query "unit tests"
swissarmyhammer search query "HTTP client setup"
```

## Complete Workflow Example

```bash
# 1. Index your Rust project
swissarmyhammer search index "**/*.rs"

# 2. Search for error handling patterns
swissarmyhammer search query "error handling patterns"

# 3. Search for async/await usage
swissarmyhammer search query "async await tokio"

# 4. Find test implementations
swissarmyhammer search query "unit tests assert"

# 5. Re-index after making changes
swissarmyhammer search index "**/*.rs" --force
```

## Output Format

### Index Output
```
Successfully indexed 45 files
📁 Indexed files: 45
⏭️ Skipped files: 3
📦 Total chunks: 234
⏱️ Execution time: 1.234s
```

### Query Output
```
🔍 Search Results for "error handling"

📄 src/error.rs:42-48 (87% similarity)
Language: rust, Type: Function
Excerpt: fn handle_error(e: Error) -> Result<()> { ... }

📄 src/lib.rs:123-135 (82% similarity) 
Language: rust, Type: Implementation
Excerpt: impl ErrorHandler for MyStruct { ... }

📄 tests/error_tests.rs:15-25 (75% similarity)
Language: rust, Type: Function
Excerpt: #[test] fn test_error_propagation() { ... }

Total results: 3, Execution time: 123ms
```

## Architecture and Storage

### Index Storage
- Index stored in `.swissarmyhammer/search.db` (DuckDB database)
- Automatically added to .gitignore
- Contains file content, embeddings, and metadata

### Embedding Model
- Uses nomic-embed-code model for high-quality code embeddings
- Model downloaded on first use (~100MB)
- Cached locally for subsequent runs

### TreeSitter Parsing
- Parses code into semantic chunks (functions, classes, etc.)
- Preserves code structure and context
- Falls back to plain text for unsupported languages

## Performance Notes

- **First-time indexing**: Downloads embedding model, may take several minutes
- **Subsequent runs**: Uses cached model for faster startup
- **Query performance**: Fast after initial model loading
- **Index updates**: Only changed files are re-indexed unless `--force` is used

## Prerequisites

Files must be indexed before querying. If no results are found:

1. Check that files have been indexed with `search index`
2. Verify the search query is relevant to indexed content
3. Ensure supported file types are being indexed

## Use Cases

- **Code Discovery**: Find similar functions or patterns in large codebases
- **Learning**: Discover how certain concepts are implemented
- **Refactoring**: Find all instances of similar code patterns
- **Documentation**: Locate examples of specific functionality
- **Code Review**: Find related code that might be affected by changes

## See Also

- [Search Architecture](./search-architecture.md) - Technical implementation details
- [Search Guide](./search-guide.md) - Advanced usage patterns
- [API Reference](./api-reference.md) - Programmatic access to search functionality