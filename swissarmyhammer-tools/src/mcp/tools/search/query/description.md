# Search Query

Perform semantic search across indexed files using vector similarity. Returns ranked results based on semantic similarity to the query.

## Parameters

- `query` (required): Search query string
- `limit` (optional): Number of results to return (default: 10)

## Examples

Basic search:
```json
{
  "query": "error handling",
  "limit": 10
}
```

Search for async functions:
```json
{
  "query": "async function implementation",
  "limit": 5
}
```

## Returns

```json
{
  "results": [
    {
      "file_path": "src/main.rs",
      "chunk_text": "fn handle_error(e: Error) -> Result<()> { ... }",
      "line_start": 42,
      "line_end": 48,
      "similarity_score": 0.87,
      "language": "rust",
      "chunk_type": "Function",
      "excerpt": "...fn handle_error(e: Error) -> Result<()> {..."
    }
  ],
  "query": "error handling",
  "total_results": 1,
  "execution_time_ms": 123
}
```

## Search Quality

- Uses nomic-embed-code model for high-quality code embeddings
- Understands semantic similarity, not just keyword matching
- Works best with indexed code that has been parsed by TreeSitter
- Returns results ranked by similarity score (higher = more similar)

## Prerequisites

Files must be indexed first using the `search_index` tool before querying.
If no results are found, check that:
1. Files have been indexed with `search_index`
2. The search query is relevant to the indexed content
3. The similarity threshold allows for your query type

## Performance Notes

- Query performance is fast after initial model loading
- First query may be slower due to model initialization
- Embedding model is cached after first use
- Search time scales logarithmically with index size