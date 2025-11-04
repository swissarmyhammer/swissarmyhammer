# Semantic Search

SwissArmyHammer Tools provides semantic code search using vector embeddings and tree-sitter parsing, enabling search by meaning rather than just keywords.

## Available Tools

### search_index
Index files for semantic search using vector embeddings.

### search_query
Query indexed code by meaning using similarity search.

## How It Works

1. **Parsing**: Tree-sitter parses code into structured tokens
2. **Chunking**: Code is split into meaningful segments
3. **Embedding**: Segments are converted to vector embeddings
4. **Indexing**: Embeddings are stored in SQLite database
5. **Querying**: Natural language queries find similar code

## Common Workflows

### Initial Index
```
1. Run search_index on all source files
2. Wait for indexing to complete
3. Use search_query to find code
```

### Find Similar Code
```
Ask: "Find code that handles authentication"
Returns: All code segments related to authentication
```

## Next Steps

- [File Operations](file-operations.md) - File tools
- [Examples](../examples/search.md) - Search examples
