# Add Progress Notifications to shell_execute MCP Tool

## Background

The MCP specification includes support for progress notifications (`notifications/progress`) that allow long-running operations to send real-time updates to clients. Our `shell_execute` tool currently lacks this capability, which means users have no feedback during long-running shell commands.

## Implementation Complete ✅

### Changes Made

All code changes have been successfully implemented and tested:

1. **Batched Progress Notifications**: Changed from per-line notifications to batched notifications every 10 lines
2. **Line Count Tracking**: Progress now reports monotonically increasing line count instead of indeterminate progress
3. **Binary Detection Alert**: Added one-time notification when binary output is detected
4. **Updated Tests**: Fixed existing tests and added comprehensive new tests

### Files Modified

**`swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs`**

#### Key Changes:

1. **Added line count tracking** (lines 846-849):
```rust
// Track line count for batched progress notifications
let mut line_count: u32 = 0;
let mut binary_notified = false;
const BATCH_SIZE: u32 = 10;
```

2. **Updated stdout handler** (lines 857-892): 
   - Replaced per-line notifications with batched notifications every 10 lines
   - Added line count increment
   - Added binary detection notification

3. **Updated stderr handler** (lines 914-949):
   - Same batching logic as stdout
   - Shares line count with stdout (unified counter)

4. **Updated remaining output handlers** (lines 978-1057):
   - Applied same batching and binary detection to post-exit output processing

5. **Updated function signature** (line 829):
   - Changed return type to include line count: `Result<(ExitStatus, OutputBuffer, u32), ShellError>`

6. **Updated completion notification** (lines 1303-1318):
   - Changed from progress=100 to progress=line_count
   - Added line_count to metadata
   - Updated message to include line count

7. **Added comprehensive tests** (lines 3837-4003):
   - `test_batched_progress_notifications`: Verifies batching at 10-line intervals
   - `test_binary_detection_notification`: Verifies binary detection alert
   - `test_progress_without_sender`: Ensures graceful handling when no sender

8. **Fixed existing tests**:
   - Updated `test_shell_execute_sends_progress_notifications` to expect line count instead of 100%
   - Updated `test_shell_execute_completion_metadata` to expect line_count in metadata

### Test Results

All 609 tests in swissarmyhammer-tools pass successfully:

```
Summary [34.888s] 609 tests run: 609 passed (21 slow), 0 skipped
```

New tests verify:
- ✅ Progress notifications are batched every 10 lines
- ✅ Progress values increase monotonically
- ✅ Binary detection sends exactly one notification
- ✅ Execution succeeds without progress sender (graceful degradation)

## Implementation Details

### Progress Notification Flow

1. **Start**: Sends progress=0, message="Executing: {command}"
2. **Batched Updates**: Every 10 lines, sends progress={line_count}, message="Processing output: {line_count} lines"
3. **Binary Detection**: When binary detected, sends progress={line_count}, message="Binary output detected"
4. **Completion**: Sends progress={line_count}, message="Command completed: {line_count} lines, exit code {exit_code}"

### Metadata Included

Completion notification includes:
- `exit_code`: Command exit code
- `duration_ms`: Execution time in milliseconds
- `line_count`: Total lines processed
- `output_truncated`: Whether output was truncated

### Design Decisions

1. **Batch Size = 10**: Balances between too many notifications (spam) and too few (no feedback)
2. **Unified Line Counter**: Counts both stdout and stderr lines together for simplicity
3. **One Binary Notification**: Uses flag to ensure binary detection is only notified once
4. **Non-Deterministic Progress**: Uses line count without total, since command output length is unknown

### Performance Impact

- Minimal: Batching reduces notification frequency by 90% compared to per-line notifications
- Line counting adds negligible overhead (simple integer increment)
- No impact when progress_sender is None (all checks are short-circuited)

## MCP Specification Compliance

Our implementation follows the [MCP Progress Specification (2025-06-18)](https://modelcontextprotocol.io/specification/2025-06-18/schema#notifications%2Fprogress):

- ✅ Uses consistent `progressToken` throughout operation
- ✅ Progress values increase monotonically
- ✅ Messages provide human-readable context
- ✅ Notifications don't block operation execution
- ✅ Uses `progress` field for line count (no `total` since non-deterministic)

## References

- [MCP Progress Specification](https://modelcontextprotocol.io/specification/2025-06-18/schema#notifications%2Fprogress)
- [FastMCP Progress Example](https://deepwiki.com/punkpeye/fastmcp/8.4-logging-and-progress-reporting)
- [Shell Command MCP (kaznak)](https://www.magicslides.app/mcps/kaznak-shell-command)
- Existing implementation: `swissarmyhammer-tools/src/mcp/progress_notifications.rs`
- Example usage: `swissarmyhammer-tools/src/mcp/tools/search/index/mod.rs`

## Success Criteria - All Met ✅

- ✅ Long-running shell commands send batched progress notifications (every 10 lines)
- ✅ Progress increases monotonically from 0 to final line count
- ✅ Messages include useful context (line count)
- ✅ Binary output detection sends exactly one notification
- ✅ Notification failures don't affect command execution
- ✅ Tests verify progress notification behavior
- ✅ No performance regression in shell command execution
- ✅ All 609 tests pass