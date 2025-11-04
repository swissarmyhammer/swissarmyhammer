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

After analyzing the code, I'll use a two-phase approach:

#### Phase 1: Add Progress Callback to FileIndexer
The `FileIndexer::index_files` method currently uses an indicatif `ProgressBar` which is terminal-only. I'll add an optional progress callback parameter that can be called during file processing.

**Changes to `swissarmyhammer-search/src/indexer.rs`:**
- Modify `index_files` to accept an optional progress callback: `Option<Box<dyn Fn(usize, usize) + Send + Sync>>`
- Call the callback every time a file is processed
- Keep the existing ProgressBar for backward compatibility (it's harmless for MCP usage)

**Changes to `index_glob` method:**
- Pass the progress callback through to `index_files`

#### Phase 2: Implement Batched Notifications in MCP Tool
In the `search_index` MCP tool, wrap the indexer calls with progress tracking.

**Changes to `swissarmyhammer-tools/src/mcp/tools/search/index/mod.rs`:**
- Track total files processed across all patterns
- Send progress notification when starting each pattern
- Provide a progress callback that sends batched updates every 10 files
- Send final completion notification with all stats

### Batching Strategy

- **Batch Size**: 10 files (same as shell_execute's 10 lines)
- **Pattern Updates**: Every pattern start (per-pattern granularity)
- **File Updates**: Every 10 files within the indexing process
- **Non-Deterministic Progress**: Use file count, not percentage (total unknown upfront)

### Metadata to Include

Progress update metadata:
- `pattern`: Current pattern being processed (when applicable)
- `files_processed`: Total files processed so far
- `patterns_completed`: Number of patterns completed

Completion metadata (existing):
- `files_indexed`: Successful files
- `files_failed`: Failed files
- `files_skipped`: Skipped files
- `total_chunks`: Total chunks created
- `duration_ms`: Execution time

## Implementation Plan

1. Modify `FileIndexer::index_files` to accept optional progress callback
2. Modify `FileIndexer::index_glob` to pass through the progress callback
3. Update `search_index` MCP tool to create progress callback and send batched notifications
4. Add tests to verify progress notifications are sent correctly
5. Verify no performance regression

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


## Code Review Resolutions (2025-11-04)

All code review findings have been addressed:

### Critical Fixes
1. **Removed #[allow(dead_code)]** from `SingleFileReport.file_path` at swissarmyhammer-search/src/indexer.rs:721
   - Removed unused `file_path` field entirely per coding standards
   - Updated `SingleFileReport::new()` to no longer accept the path parameter
   - Build and tests pass successfully

### Documentation Enhancements
2. **Enhanced progress callback documentation** in both `index_glob_with_progress` and `index_files_with_progress`
   - Added detailed "Progress Callback Behavior" sections
   - Clarified that callbacks are invoked after each file (not just on batch boundaries)
   - Documented that callback failures don't affect indexing operations
   - Explained monotonic progress tracking

3. **Added comprehensive documentation to SearchIndexTool::execute method**
   - Documented complete progress notification flow (start, per-pattern, batched files, completion)
   - Explained error handling strategy for notification failures
   - Included metadata details for each notification type

### Testing Improvements
4. **Added test for progress callback error scenarios**
   - New test: `test_search_index_continues_when_progress_callback_panics`
   - Verifies indexing continues successfully even when progress callbacks have issues
   - Confirms errors are not related to progress notification failures

### Build Verification
- All tests pass: 3430 tests run, 3430 passed
- Cargo clippy clean: No warnings or errors
- All code compiles successfully

The implementation is ready and addresses all code review concerns while maintaining backward compatibility and following established patterns from the shell_execute implementation.
