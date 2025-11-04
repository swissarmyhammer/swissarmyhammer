# Add Progress Notifications to search_query MCP Tool

## Background

The MCP specification includes support for progress notifications (`notifications/progress`) that allow long-running operations to send real-time updates to clients. Our `search_query` tool currently lacks this capability, which means users have no feedback during semantic searches that may take time on large code indexes.

## Problem

Semantic search operations can be slow due to:
- Large vector indexes with many embeddings
- Complex similarity calculations across thousands of code chunks
- Result ranking and filtering
- Multiple files being searched

Users currently have no visibility into:
- Search progress through the index
- How many results have been found
- Whether the search is still running or hung

## Comparison with Related Tools

- `search_index` already has progress notifications for indexing operations
- `files_grep` has progress notifications for content searching
- `search_query` should follow the same pattern

## Proposed Solution

Implement progress notifications for semantic search that report:
- Search start
- Progress through the index (if deterministic)
- Results found
- Completion with result summary

### Progress Notification Flow

1. **Start**: Send progress=0, message="Searching: {query}"
2. **Progress Updates**: Send periodic updates during search
3. **Completion**: Send progress={results_found}, message="Search completed: {results_found} results"

### Metadata to Include

Completion notification should include:
- `query`: The search query text
- `results_found`: Number of results found
- `results_returned`: Number of results returned (after limit)
- `duration_ms`: Search time in milliseconds
- `index_size`: Number of chunks searched

## Implementation Notes

- Follow the pattern established in `shell_execute` and `search_index` tools
- Use the existing `ProgressSender` from `ToolContext`
- Ensure notifications don't block search execution
- Handle cases where progress_sender is None gracefully
- Consider batching if search is very fast to avoid notification spam

## Success Criteria

- ✅ Semantic searches send start notification
- ✅ Progress updates sent during long searches
- ✅ Completion notification includes search summary
- ✅ Notification failures don't affect search execution
- ✅ Tests verify progress notification behavior
- ✅ No performance regression in search execution

## References

- [MCP Progress Specification](https://modelcontextprotocol.io/specification/2025-06-18/schema#notifications%2Fprogress)
- Existing implementation: `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs` (lines 846-1318)
- Search index progress: `swissarmyhammer-tools/src/mcp/tools/search/index/mod.rs`
- Tool implementation: `swissarmyhammer-tools/src/mcp/tools/search/query/mod.rs`