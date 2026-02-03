Detect duplicate or similar code across the codebase using semantic similarity analysis.

**IMPORTANT: Use this tool for code quality validation to find duplication that manual analysis would miss.**

This tool uses AI embeddings to identify code duplication by semantic meaning, not just textual similarity. It finds:
- Exact duplicates
- Refactored duplicates (same logic, different variable names)
- Copy-pasted code with minor modifications
- Similar algorithms implemented differently

**When to use:** Always use when validating code changes for duplication, as it provides comprehensive project-wide analysis that's impossible to do manually.

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
