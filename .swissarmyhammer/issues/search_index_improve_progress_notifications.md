# Improve Progress Notifications for search_index MCP Tool

## Background

The `search_index` tool currently has basic progress notifications (start and completion), but lacks intermediate progress updates during long-running indexing operations. When indexing large codebases, users can experience 2-5 minutes of silence with no feedback, making it unclear if the operation is progressing or hung.

## Current State

### What Exists ✅
- **Start notification** (progress=0): "Starting indexing: {N} patterns"
- **Completion notification** (progress=100): "Indexed {N} files ({M} chunks) in {X}s"

### What's Missing ❌
- No intermediate progress updates during indexing
- No indication of which pattern is currently being processed
- No per-file or batched per-file progress updates

### The Problem

Users indexing large codebases see:
1. "Starting indexing: 5 patterns" at 0%
2. *[Complete silence for 2-5 minutes]*
3. "Indexed 1500 files (3200 chunks) in 3.2s" at 100%

Additionally, the indexer uses console `ProgressBar` (indicatif crate) which is only visible in the terminal, not to MCP clients.

## Comparison with shell_execute

The `shell_execute` tool implements batched progress notifications correctly:
- Start at progress=0
- Batched updates every 10 lines with monotonically increasing line count
- Completion with final line count

`search_index` should follow the same pattern.

## Proposed Solution

Implement batched intermediate progress notifications following the shell_execute pattern:

### Progress Notification Flow

1. **Start**: Send progress=0, message="Starting indexing: {N} patterns"
2. **Per-Pattern Updates**: When starting each pattern, send progress update
3. **Batched File Updates**: Every 10 files indexed, send progress update
4. **Completion**: Send final progress with complete stats

### Implementation Approach

#### Phase 1: Per-Pattern Progress (Quick Win)
Add progress notifications in the pattern loop:

```rust
for (index, pattern) in request.patterns.iter().enumerate() {
    // Send pattern progress
    if let Some(sender) = &_context.progress_sender {
        sender.send_progress_with_metadata(
            &progress_token,
            Some(files_processed), // or calculate percentage
            format!("Processing pattern {}/{}: {}", index+1, request.patterns.len(), pattern),
            json!({"pattern": pattern, "files_processed": files_processed})
        ).ok();
    }
    
    let report = indexer.index_glob(pattern, request.force).await?;
    // Update files_processed counter
}
```

#### Phase 2: Per-File Progress (Better UX)
Add progress callback to FileIndexer API:

```rust
// In swissarmyhammer-search/src/indexer.rs
pub async fn index_glob_with_progress<F>(
    &mut self, 
    pattern: &str, 
    force: bool,
    progress_callback: F
) -> SearchResult<IndexingReport>
where
    F: Fn(usize, usize) + Send
{
    // In the file processing loop:
    if processed_files % BATCH_SIZE == 0 {
        progress_callback(processed_files, total_files);
    }
}
```

Then use it in the MCP tool:

```rust
let report = indexer.index_glob_with_progress(
    pattern, 
    request.force,
    |processed, total| {
        if let Some(sender) = &_context.progress_sender {
            sender.send_progress_with_metadata(
                &progress_token,
                Some(processed),
                format!("Indexing files: {}/{}", processed, total),
                json!({"pattern": pattern, "files_processed": processed})
            ).ok();
        }
    }
).await?;
```

### Batching Strategy

- **Batch Size**: 10 files (same as shell_execute's 10 lines)
- **Pattern Updates**: Every pattern start
- **File Updates**: Every 10 files within each pattern
- **Non-Deterministic Progress**: Use file count, not percentage (total unknown upfront)

### Metadata to Include

Progress update metadata:
- `pattern`: Current pattern being processed
- `files_processed`: Total files processed so far
- `patterns_completed`: Number of patterns completed

Completion metadata (existing):
- `files_indexed`: Successful files
- `files_failed`: Failed files
- `files_skipped`: Skipped files
- `total_chunks`: Total chunks created
- `duration_ms`: Execution time

## Implementation Notes

- Follow the batched notification pattern from `shell_execute` (lines 846-1318)
- Ensure notifications don't block indexing operations
- Handle cases where progress_sender is None gracefully
- Consider both MCP tool level updates (per-pattern) and indexer level updates (per-file)
- Remove or keep console progress bars (they're harmless for CLI usage)

## Success Criteria

- ✅ Per-pattern progress updates sent when processing each pattern
- ✅ Batched per-file progress updates sent every 10 files
- ✅ Progress values increase monotonically
- ✅ Users see real-time feedback during long indexing operations
- ✅ Notification failures don't affect indexing operations
- ✅ Tests verify progress notification behavior
- ✅ No performance regression in indexing operations

## References

- [MCP Progress Specification](https://modelcontextprotocol.io/specification/2025-06-18/schema#notifications%2Fprogress)
- Shell execute implementation: `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs` (lines 846-1318)
- Current implementation: `swissarmyhammer-tools/src/mcp/tools/search/index/mod.rs` (lines 102-215)
- Indexer with console progress: `swissarmyhammer-search/src/indexer.rs` (lines 266-297)