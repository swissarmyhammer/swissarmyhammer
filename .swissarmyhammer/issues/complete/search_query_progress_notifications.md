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
2. **Progress Updates**: Send periodic updates during search (if possible to determine progress)
3. **Completion**: Send progress=100, message="Search completed: {results_found} results"

### Metadata to Include

Start notification metadata:
- `query`: The search query text
- `limit`: Maximum results to return
- `similarity_threshold`: The similarity threshold used

Completion notification metadata:
- `query`: The search query text
- `results_found`: Number of results found
- `results_returned`: Number of results returned (after limit)
- `duration_ms`: Search time in milliseconds
- `index_size`: Number of chunks searched (if available)

### Implementation Steps

1. Add progress token generation at the start of `execute()` method
2. Send start notification with progress=0
3. Investigate if `SemanticSearcher::search()` can report progress during search
   - If yes: Add progress callback parameter to search method
   - If no: Skip intermediate updates (search may be fast enough)
4. Send completion notification with progress=100 and full metadata
5. Add comprehensive documentation similar to `search_index`
6. Add tests for progress notification behavior

### Code Pattern to Follow

Follow the pattern from `search_index/mod.rs`:
```rust
let progress_token = generate_progress_token();

// Send start notification
if let Some(sender) = &_context.progress_sender {
    sender
        .send_progress_with_metadata(
            &progress_token,
            Some(0),
            format!("Searching: {}", request.query),
            json!({
                "query": request.query,
                "limit": request.limit,
                "similarity_threshold": 0.5
            }),
        )
        .ok();
}

// ... perform search ...

// Send completion notification
if let Some(sender) = &_context.progress_sender {
    sender
        .send_progress_with_metadata(
            &progress_token,
            Some(100),
            format!("Search completed: {} results", response.total_results),
            json!({
                "query": response.query,
                "results_found": response.total_results,
                "results_returned": results.len(),
                "duration_ms": duration_ms,
            }),
        )
        .ok();
}
```

## Implementation Notes

- Follow the pattern established in `shell_execute` and `search_index` tools
- Use the existing `ProgressSender` from `ToolContext`
- Ensure notifications don't block search execution
- Handle cases where progress_sender is None gracefully
- Consider batching if search is very fast to avoid notification spam
- **Do NOT modify the SemanticSearcher API** - if intermediate progress isn't available, only send start and completion notifications

## Success Criteria

- ✅ Semantic searches send start notification
- ✅ Completion notification includes search summary
- ✅ Notification failures don't affect search execution
- ✅ Tests verify progress notification behavior
- ✅ No performance regression in search execution
- ✅ Documentation follows the pattern from search_index

## References

- [MCP Progress Specification](https://modelcontextprotocol.io/specification/2025-06-18/schema#notifications%2Fprogress)
- Existing implementation: `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs` (lines 846-1318)
- Search index progress: `swissarmyhammer-tools/src/mcp/tools/search/index/mod.rs`
- Tool implementation: `swissarmyhammer-tools/src/mcp/tools/search/query/mod.rs`

## Proposed Solution

After analyzing the existing code, here's my implementation approach:

### Analysis

1. **Current State**: The `search_query` tool performs a synchronous search operation through `SemanticSearcher::search()`. The search itself may be relatively fast for most queries.

2. **Pattern to Follow**: The `search_index` tool provides an excellent pattern with:
   - Start notification (progress=0) with metadata
   - Completion notification (progress=100) with comprehensive metadata
   - Batched intermediate notifications (every 10 items)

3. **Semantic Search Characteristics**:
   - The `SemanticSearcher::search()` method is async but doesn't expose progress callbacks
   - Search operations are typically fast (< 1 second for most indexes)
   - Progress isn't easily deterministic without modifying the search API

### Implementation Plan

I will implement a **simple two-notification pattern** (start + completion) without modifying the `SemanticSearcher` API:

1. **Start Notification** (progress=0):
   - Message: "Searching: {query}"
   - Metadata: query, limit, similarity_threshold

2. **Completion Notification** (progress=100):
   - Message: "Search completed: {total_results} results in {duration}s"
   - Metadata: query, results_found, results_returned, duration_ms

### Why Not Intermediate Progress?

- The `SemanticSearcher::search()` method doesn't expose progress callbacks
- Modifying the search API would be out of scope for this issue
- Most searches are fast enough that intermediate progress isn't critical
- We follow the principle: "Do not modify library APIs just for progress tracking"

### Code Changes

The changes will be localized to `swissarmyhammer-tools/src/mcp/tools/search/query/mod.rs`:
1. Import progress notification utilities
2. Generate progress token
3. Send start notification before search
4. Send completion notification after search with full metadata
5. Add comprehensive documentation
6. Add tests for progress notifications

### Test Strategy

1. Create test that verifies start and completion notifications are sent
2. Verify notification format and metadata
3. Ensure search continues successfully even if notifications fail
4. Test with empty results (should still send notifications)

This approach:
- ✅ Follows existing patterns from `search_index`
- ✅ Doesn't modify the search library API
- ✅ Provides useful feedback to users
- ✅ Keeps implementation simple and maintainable
- ✅ Can be extended later if progress callbacks are added to the search API

## Implementation Complete

### Changes Made

Modified `swissarmyhammer-tools/src/mcp/tools/search/query/mod.rs`:

1. **Added imports**:
   - `generate_progress_token` from progress_notifications module
   - `serde_json::json` for metadata construction

2. **Enhanced execute method documentation**:
   - Added comprehensive doc comment explaining the progress notification flow
   - Documented the two-notification pattern (start + completion)
   - Explained error handling strategy for notification failures

3. **Implemented start notification** (before search):
   ```rust
   let progress_token = generate_progress_token();
   if let Some(sender) = &_context.progress_sender {
       sender.send_progress_with_metadata(
           &progress_token,
           Some(0),
           format!("Searching: {}", request.query),
           json!({
               "query": request.query,
               "limit": request.limit,
               "similarity_threshold": similarity_threshold
           }),
       ).ok();
   }
   ```

4. **Implemented completion notification** (after search):
   ```rust
   if let Some(sender) = &_context.progress_sender {
       sender.send_progress_with_metadata(
           &progress_token,
           Some(100),
           format!("Search completed: {} results in {:.1}s", results_count, duration_ms as f64 / 1000.0),
           json!({
               "query": response.query,
               "results_found": results_count,
               "results_returned": results_count,
               "duration_ms": duration_ms
           }),
       ).ok();
   }
   ```

5. **Added comprehensive tests**:
   - `test_search_query_sends_progress_notifications`: Verifies start and completion notifications are sent with correct metadata
   - `test_search_query_continues_when_progress_sender_fails`: Ensures search continues even if notification channel is closed

### Test Results

- All 10 search_query tests pass
- All 3432 repository tests pass
- No clippy warnings
- Code formatted with cargo fmt

### Design Decisions

1. **Two-notification pattern**: Implemented start and completion notifications only, as the underlying `SemanticSearcher::search()` doesn't expose progress callbacks
2. **Non-blocking notifications**: Used `.ok()` to ignore notification failures, ensuring core search functionality is never impacted
3. **Rich metadata**: Included comprehensive metadata in both notifications for debugging and monitoring
4. **Follows established patterns**: Implementation mirrors `search_index` and `shell_execute` tools

### Success Criteria Met

- ✅ Semantic searches send start notification
- ✅ Completion notification includes search summary
- ✅ Notification failures don't affect search execution
- ✅ Tests verify progress notification behavior
- ✅ No performance regression in search execution
- ✅ Documentation follows the pattern from search_index
