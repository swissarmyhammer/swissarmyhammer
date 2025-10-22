# Semantic Search

SwissArmyHammer provides vector-based semantic code search using tree-sitter parsing and embeddings for intelligent code navigation.

## Overview

Semantic search goes beyond keyword matching to understand the meaning and context of code. This enables AI assistants to find relevant code even when the exact keywords don't match.

## How It Works

1. **Indexing**: Source files are parsed with tree-sitter to extract meaningful code chunks
2. **Embedding**: Each chunk is converted to a vector embedding using a language model
3. **Storage**: Embeddings are stored in a SQLite database with full-text search
4. **Querying**: Search queries are embedded and matched using vector similarity
5. **Ranking**: Results are ranked by similarity score (0.0 to 1.0)

## Supported Languages

- Rust
- Python
- TypeScript
- JavaScript
- Dart

## Available Tools

### search_index

Index files for semantic search using vector embeddings.

**Parameters:**
- `patterns` (required): Array of glob patterns or files to index (e.g., `["**/*.rs"]`)
- `force` (optional): Force re-indexing of all files (default: false)

**Example:**
```json
{
  "patterns": ["src/**/*.rs", "tests/**/*.rs"],
  "force": false
}
```

**Returns:**
- Number of files indexed
- Number of files skipped (unchanged)
- Total chunks created
- Execution time

**Features:**
- Incremental indexing (only indexes changed files)
- Tree-sitter parsing for accurate code structure
- Language-aware chunking
- Progress tracking for large codebases

### search_query

Perform semantic search across indexed files using vector similarity.

**Parameters:**
- `query` (required): Search query string
- `limit` (optional): Number of results to return (default: 10)

**Example:**
```json
{
  "query": "error handling",
  "limit": 10
}
```

**Returns:**
Array of results with:
- File path
- Line numbers
- Code chunk text
- Similarity score (0.0 to 1.0)
- Language
- Contextual excerpt

**Features:**
- Vector similarity search
- Ranked results by relevance
- Full code context in results
- Fast query execution

## Indexing Strategy

### Initial Index

When starting with a new codebase:

```json
{
  "patterns": ["**/*.rs"],
  "force": false
}
```

This creates the initial index. Subsequent runs only index changed files.

### Force Re-index

After major refactoring or when results seem stale:

```json
{
  "patterns": ["**/*.rs"],
  "force": true
}
```

### Selective Indexing

Index only specific directories or file types:

```json
{
  "patterns": [
    "src/**/*.rs",
    "tests/**/*.rs"
  ]
}
```

## Search Strategies

### Conceptual Search

Find code by describing what it does:

```json
{
  "query": "parse command line arguments",
  "limit": 5
}
```

### Feature Search

Find code related to specific features:

```json
{
  "query": "authentication and authorization",
  "limit": 10
}
```

### Pattern Search

Find code implementing specific patterns:

```json
{
  "query": "builder pattern with fluent interface",
  "limit": 5
}
```

### Error Handling Search

Find error handling code:

```json
{
  "query": "error handling with custom error types",
  "limit": 10
}
```

## Understanding Results

### Similarity Scores

- **0.9 - 1.0**: Extremely relevant, likely exact match
- **0.7 - 0.9**: Highly relevant, strong semantic match
- **0.5 - 0.7**: Moderately relevant, related concepts
- **0.3 - 0.5**: Weakly relevant, tangential relationship
- **< 0.3**: Likely not relevant

### Result Context

Each result includes:
- **File Path**: Location of the matching code
- **Line Numbers**: Specific lines containing the match
- **Chunk Text**: The actual code that matched
- **Excerpt**: Contextual snippet with highlights
- **Language**: Source language for syntax awareness

## Storage and Performance

### Database Location

The search index is stored in `.swissarmyhammer/search.db`.

Add this to `.gitignore`:
```
.swissarmyhammer/search.db
```

### Database Size

Typical database sizes:
- Small project (1,000 lines): ~100 KB
- Medium project (10,000 lines): ~1 MB
- Large project (100,000 lines): ~10 MB

### Indexing Performance

- **Small projects**: < 1 second
- **Medium projects**: 1-5 seconds
- **Large projects**: 5-30 seconds

### Query Performance

- **Typical queries**: < 100ms
- **Complex queries**: < 500ms

## Best Practices

### Indexing

1. **Index Early**: Create the initial index when starting work
2. **Incremental Updates**: Let the tool handle incremental indexing
3. **Force Re-index Sparingly**: Only when results seem stale
4. **Target Patterns**: Index only the code you need to search

### Searching

1. **Be Descriptive**: Use natural language to describe what you're looking for
2. **Adjust Limits**: Start with default (10), increase for broader search
3. **Iterate**: Refine queries based on results
4. **Check Scores**: High scores indicate better matches

### Maintenance

1. **Re-index After Major Changes**: Large refactorings may benefit from force re-index
2. **Clean Database**: Delete `.swissarmyhammer/search.db` to start fresh
3. **Monitor Size**: Large databases may indicate over-indexing

## Common Use Cases

### Understanding New Codebase

```json
{
  "query": "main entry point initialization",
  "limit": 5
}
```

### Finding Similar Code

```json
{
  "query": "async function that processes files in parallel",
  "limit": 10
}
```

### Locating Features

```json
{
  "query": "user authentication with JWT tokens",
  "limit": 5
}
```

### Finding Examples

```json
{
  "query": "test cases for error handling",
  "limit": 10
}
```

## Limitations

- **Index Size**: Very large codebases (> 1M lines) may have slower indexing
- **Language Support**: Limited to supported languages (Rust, Python, TS, JS, Dart)
- **Query Precision**: Natural language queries may return unexpected results
- **Context Window**: Each chunk is limited in size (256 tokens)

## Troubleshooting

### No Results

- Verify files are indexed: check indexing output
- Try broader query terms
- Increase limit parameter
- Check file patterns match your code

### Poor Results

- Try more specific query terms
- Force re-index with `force: true`
- Check similarity scores
- Verify language is supported

### Slow Indexing

- Reduce patterns to necessary files
- Exclude test fixtures and generated code
- Check for very large files

## Next Steps

- [Code Analysis](./code-analysis.md): Generate code outlines
- [File Operations](./file-operations.md): Read and edit files
- [Issue Management](./issue-management.md): Track work items
