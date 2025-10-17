# MCP Notification Recommendations

**Date**: 2025-10-17
**Status**: Proposed

## Overview

This document proposes adding MCP progress notifications to SwissArmyHammer tools to provide real-time feedback during long-running operations. The MCP specification supports progress notifications that allow servers to send streaming updates to clients during tool execution.

## Current State

### Existing Notification Support

We currently have:
1. **Flow Notifications** (notifications.rs) - Used for workflow execution progress with structured metadata
2. **notify_create Tool** - Allows explicit user-triggered notifications
3. **File Watcher Notifications** - Sends prompts/listChanged when files change

### Notification Infrastructure

- `FlowNotification` struct with progress (0-100), message, and metadata
- `NotificationSender` with channel-based async notification delivery
- Flow-specific notification types: FlowStart, StateStart, StateComplete, FlowComplete, FlowError

## MCP Progress Notification Specification

Based on MCP spec analysis:
- Progress notifications allow servers to send incremental updates during tool execution
- Format: `notifications/progress` with `progressToken` and progress data
- Clients can display progress bars, streaming output, or status updates
- Useful for operations taking >1 second

## Recommended Notifications by Tool Category

### 1. Shell Execution (`shell_execute`)

**Current Behavior**: Executes command and returns all output at completion
**Problem**: No feedback during long-running commands (builds, tests, deployments)

**Recommended Notifications**:
- **Start notification**: Command execution starting
  ```rust
  {
    "progressToken": "shell_12345",
    "progress": 0,
    "message": "Executing: cargo build --release"
  }
  ```
- **Streaming output**: Each line of stdout/stderr as it arrives
  ```rust
  {
    "progressToken": "shell_12345",
    "message": "   Compiling swissarmyhammer v0.1.0",
    "metadata": {
      "stream": "stdout",
      "line_number": 42
    }
  }
  ```
- **Completion notification**: Final status with exit code
  ```rust
  {
    "progressToken": "shell_12345",
    "progress": 100,
    "message": "Command completed with exit code 0",
    "metadata": {
      "exit_code": 0,
      "duration_ms": 45230
    }
  }
  ```

**Implementation Location**: `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs`
**Priority**: HIGH - Shell commands are frequently long-running

### 2. Search Indexing (`search_index`)

**Current Behavior**: Indexes files and returns summary at completion
**Problem**: No feedback during indexing of large codebases (can take minutes)

**Recommended Notifications**:
- **Start notification**: Begin indexing
- **File progress**: Notification per file or per N files
  ```rust
  {
    "progressToken": "index_67890",
    "progress": 45,  // (files_processed / total_files) * 100
    "message": "Indexing: src/main.rs (234/520 files)",
    "metadata": {
      "current_file": "src/main.rs",
      "files_processed": 234,
      "total_files": 520,
      "chunks_created": 1842
    }
  }
  ```
- **Completion notification**: Final statistics

**Implementation Location**: `swissarmyhammer-tools/src/mcp/tools/search/index/mod.rs`
**Priority**: HIGH - Indexing large codebases is time-consuming

### 3. Web Search (`web_search`)

**Current Behavior**: Performs search and returns results
**Problem**: No feedback during search and content fetching

**Recommended Notifications**:
- **Search progress**: Searching, fetching results
  ```rust
  {
    "progressToken": "search_33221",
    "progress": 30,
    "message": "Searching DuckDuckGo for: rust async programming"
  }
  ```
- **Content fetching**: Per-URL progress when fetching content
  ```rust
  {
    "progressToken": "search_33221",
    "progress": 60,
    "message": "Fetching content from 5/10 URLs"
  }
  ```

**Implementation Location**: `swissarmyhammer-tools/src/mcp/tools/web_search/search/mod.rs`
**Priority**: MEDIUM - Web operations can be slow

### 4. Web Fetch (`web_fetch`)

**Current Behavior**: Fetches URL and converts to markdown
**Problem**: No feedback during slow network requests

**Recommended Notifications**:
- **Fetching**: Downloading content
- **Converting**: Converting HTML to markdown
  ```rust
  {
    "progressToken": "fetch_44556",
    "progress": 50,
    "message": "Converting HTML to markdown",
    "metadata": {
      "url": "https://example.com",
      "content_length": 125000
    }
  }
  ```

**Implementation Location**: `swissarmyhammer-tools/src/mcp/tools/web_fetch/fetch/mod.rs`
**Priority**: MEDIUM - Network requests vary in duration

### 5. Outline Generation (`outline_generate`)

**Current Behavior**: Parses files and generates outline
**Problem**: No feedback when processing many files

**Recommended Notifications**:
- **File processing**: Progress per file
  ```rust
  {
    "progressToken": "outline_77889",
    "progress": 65,
    "message": "Parsing: src/lib.rs (13/20 files)",
    "metadata": {
      "files_processed": 13,
      "total_files": 20,
      "symbols_found": 234
    }
  }
  ```

**Implementation Location**: `swissarmyhammer-tools/src/mcp/tools/outline/generate/mod.rs`
**Priority**: MEDIUM - Large codebases take time

### 6. Rules Checking (`rules_check`)

**Current Behavior**: Checks rules against files
**Problem**: No feedback during checking of many files

**Recommended Notifications**:
- **File checking progress**
  ```rust
  {
    "progressToken": "rules_99001",
    "progress": 40,
    "message": "Checking rules: src/main.rs (45/112 files)",
    "metadata": {
      "files_checked": 45,
      "total_files": 112,
      "violations_found": 12
    }
  }
  ```

**Implementation Location**: `swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs`
**Priority**: LOW - Usually fast unless many files

### 7. File Operations

**Current Behavior**: Perform operations and return results
**Problem**: Generally fast, notifications less critical

**Recommended Notifications**:
- **files_grep**: Progress when searching many files
- **files_glob**: Progress when matching large directory trees

**Priority**: LOW - Usually complete quickly

### 8. Issue Operations

**Current Behavior**: Create, list, update issues
**Problem**: Generally fast operations

**Priority**: LOW - Usually fast, notifications not needed

### 9. Git Operations

**Current Behavior**: Git operations like `git_changes`
**Problem**: Git operations usually complete quickly

**Priority**: LOW - Usually fast, notifications not needed

## Implementation Strategy

### Phase 1: Infrastructure (Week 1)

1. **Create Generic Progress Notification Types**
   - Extend existing `FlowNotification` or create `ProgressNotification`
   - Support `progressToken`, `progress` (0-100), `message`, `metadata`
   - Add progress notification sender to `ToolContext`

2. **Add Notification Support to Tool Execution**
   - Modify `McpTool` trait to support optional progress channel
   - Update `execute` signature to accept `Option<ProgressSender>`
   - Pass progress sender through from server to tools

3. **Test Infrastructure**
   - Unit tests for progress notification types
   - Integration tests for notification delivery

### Phase 2: High-Priority Tools (Week 2)

1. **shell_execute**: Add streaming output notifications
   - Notification on command start
   - Notification per output line (or buffered by time/size)
   - Notification on completion with exit code

2. **search_index**: Add indexing progress
   - Notification on start with file count estimate
   - Notification every N files (or percentage)
   - Notification on completion with statistics

### Phase 3: Medium-Priority Tools (Week 3)

1. **web_search**: Add search progress
2. **web_fetch**: Add fetch/conversion progress
3. **outline_generate**: Add parsing progress

### Phase 4: Polish & Documentation (Week 4)

1. **rules_check**: Add checking progress
2. **files_grep**: Add search progress (if needed)
3. Update documentation with notification examples
4. Performance testing and optimization

## Technical Design

### Progress Notification Structure

```rust
/// Progress notification for MCP tool operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressNotification {
    /// Unique token for this operation
    pub progress_token: String,

    /// Progress percentage (0-100), None for indeterminate
    pub progress: Option<u32>,

    /// Human-readable progress message
    pub message: String,

    /// Tool-specific metadata
    #[serde(flatten)]
    pub metadata: Option<serde_json::Value>,
}
```

### Progress Sender Integration

```rust
/// Context passed to tool execution, extended with progress sender
pub struct ToolContext {
    // ... existing fields ...

    /// Optional progress notification sender
    pub progress_sender: Option<Arc<ProgressSender>>,
}

/// Progress notification sender
pub struct ProgressSender {
    sender: mpsc::UnboundedSender<ProgressNotification>,
}
```

### Tool Implementation Pattern

```rust
async fn execute(&self, args: Map<String, Value>, context: &ToolContext)
    -> Result<CallToolResult, McpError>
{
    let progress_token = generate_progress_token();

    // Send start notification
    if let Some(sender) = &context.progress_sender {
        sender.send_progress(&progress_token, 0, "Starting operation").ok();
    }

    // Perform work with periodic progress updates
    for (i, item) in items.iter().enumerate() {
        // Do work...

        if let Some(sender) = &context.progress_sender {
            let progress = ((i + 1) * 100 / items.len()) as u32;
            sender.send_progress(&progress_token, progress,
                &format!("Processing item {}/{}", i + 1, items.len())).ok();
        }
    }

    // Send completion
    if let Some(sender) = &context.progress_sender {
        sender.send_progress(&progress_token, 100, "Operation complete").ok();
    }

    Ok(result)
}
```

## Notification Best Practices

1. **Frequency**: Send notifications at most every 100ms to avoid flooding
2. **Buffering**: Buffer rapid updates (like shell output lines)
3. **Meaningful Progress**: Only send progress % when deterministic
4. **Error Handling**: Don't fail operation if notification fails
5. **Token Generation**: Use ULID or similar for unique progress tokens
6. **Metadata**: Include useful context but keep payloads small

## Testing Strategy

1. **Unit Tests**: Test notification generation and formatting
2. **Integration Tests**: Test notification delivery through channels
3. **Manual Testing**: Test with MCP Inspector to verify client display
4. **Performance Tests**: Ensure notifications don't slow operations significantly

## Success Metrics

1. **Coverage**: Notifications added to all HIGH priority tools
2. **Performance**: <5% overhead from notification infrastructure
3. **User Experience**: Visible progress in MCP clients during long operations
4. **Reliability**: No tool failures due to notification errors

## Open Questions

1. Should we use existing `FlowNotification` or create separate `ProgressNotification`?
2. How do we handle notification failures gracefully?
3. Should notifications be opt-in or opt-out per tool?
4. What's the optimal notification frequency for shell output streaming?

## References

- MCP Specification: https://spec.modelcontextprotocol.io/
- Existing FlowNotification: `swissarmyhammer-tools/src/mcp/notifications.rs`
- rmcp library: v0.6.4

## Appendix: Notification Examples

### Shell Command with Streaming Output

```
→ shell_execute("cargo build --release")

Notification 1:
{
  "progressToken": "shell_01K7...",
  "progress": 0,
  "message": "Executing: cargo build --release"
}

Notification 2-50 (streaming):
{
  "progressToken": "shell_01K7...",
  "message": "   Compiling swissarmyhammer v0.1.0",
  "metadata": {"stream": "stdout"}
}

Notification 51 (completion):
{
  "progressToken": "shell_01K7...",
  "progress": 100,
  "message": "Completed: exit code 0 in 45.2s",
  "metadata": {"exit_code": 0, "duration_ms": 45230}
}
```

### Search Indexing with Progress

```
→ search_index(["**/*.rs"])

Notification 1:
{
  "progressToken": "index_01K7...",
  "progress": 0,
  "message": "Starting indexing: found 520 files"
}

Notification 2-10 (periodic):
{
  "progressToken": "index_01K7...",
  "progress": 45,
  "message": "Indexed 234/520 files (1842 chunks)",
  "metadata": {
    "files_processed": 234,
    "total_files": 520,
    "chunks_created": 1842
  }
}

Notification 11 (completion):
{
  "progressToken": "index_01K7...",
  "progress": 100,
  "message": "Indexed 520 files (4250 chunks) in 32.1s",
  "metadata": {
    "files_indexed": 520,
    "total_chunks": 4250,
    "duration_ms": 32100
  }
}
```
