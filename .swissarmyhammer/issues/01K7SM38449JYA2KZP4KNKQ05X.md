# Eliminate notify_create Tool and Replace with Native MCP Notifications

## Summary

The `notify_create` tool is a custom workaround that allows LLMs to send notifications to users through tool calls. However, the MCP specification provides native support for progress notifications that servers can send directly to clients without requiring a tool call. We should eliminate the `notify_create` tool and use MCP's native notification mechanism instead.

## Current State

### notify_create Tool
- **Location**: `swissarmyhammer-tools/src/mcp/tools/notify/create/mod.rs`
- **Purpose**: Sends notification messages from LLM to user through the logging system
- **Parameters**:
  - `message` (required): The notification message
  - `level` (optional): Notification level (info, warn, error)
  - `context` (optional): Structured JSON context data

### Tool Usage in Coding Standards
The `notify_create` tool is referenced in the CODING_STANDARDS memo in the refactoring section. This instruction should be removed as it encourages unnecessary tool calls for notifications.

## Problems with Current Approach

1. **Inefficient**: Requires LLM to explicitly call a tool just to send a notification
2. **Not Standard**: MCP has native notifications - we're reinventing the wheel  
3. **Limited**: Tool-based notifications are slower and less flexible than server-sent notifications
4. **Unnecessary Abstraction**: Adds complexity without providing real value
5. **Confuses Notification Types**: Mixes user-triggered notifications with progress updates

## Recommended Solution

### Replace with MCP Progress Notifications

The MCP specification supports server-sent progress notifications during long-running operations:

```rust
// Instead of requiring LLM to call notify_create
// Server sends notifications directly:
context.send_progress_notification(ProgressNotification {
    progressToken: "operation_123",
    progress: Some(50),
    message: "Processing files: 50% complete"
});
```

### Migration Path

1. **Add Progress Notification Infrastructure** (see `specification/mcp_notifications_recommendations.md`)
   - Create `ProgressNotification` type
   - Add `ProgressSender` to `ToolContext`
   - Implement notification channel from server to client

2. **Update Tools to Send Notifications Directly**
   - Long-running tools (shell, search_index, web_search) should send progress notifications
   - No LLM tool call required - server sends automatically

3. **Remove notify_create Tool**
   - Delete `swissarmyhammer-tools/src/mcp/tools/notify/create/`
   - Remove from tool registry
   - Delete tests in `tests/notify_integration_tests.rs`
   - Remove documentation references

4. **Update Coding Standards**
   - Remove instructions to use `notify_create`
   - Add guidance on when tools should send progress notifications

## Benefits

1. **Standard Compliance**: Use MCP's native notification mechanism
2. **Better UX**: Notifications appear immediately without LLM overhead
3. **More Efficient**: No tool call latency
4. **Cleaner Architecture**: Separates progress notifications from tool interface
5. **Future-Proof**: Aligns with MCP specification evolution

## Implementation Tasks

### Prerequisite
- **01K7SHZ4203SMD2C6HTW1QV3ZP**: Phase 1: Implement MCP Progress Notification Infrastructure

### Code Removal (in order)
1. **01K7SJ20RGNQWWE98DF0PY6W0Y**: Remove notify_create Tool Implementation
2. **01K7SJ20XYB4RM3QFS87B2P5WE**: Remove notify_create from Tool Registry
3. **01K7SJ213B62Y2MHASKSR6W9J3**: Delete notify_create Integration Tests
4. **01K7SJ2191XVYB02C4EAD2PWEN**: Remove notify_create from MCP Server Parity Tests
5. **01K7SJ21ERGZQ834KD1FBSRCRP**: Remove notify_create from CLI MCP Tools Registration Test
6. **01K7SJ21MP39KR5G7C8GKKTK37**: Remove notify_create from CLI Serve Integration Test

### Documentation Updates
7. **01K7SJ3984TVZSMSF2YZHRDJ7J**: Remove notify_create from Documentation - Tools Reference
8. **01K7SJ39E8T166Y25K2B0PXBR3**: Remove notify_create from Documentation - Features
9. **01K7SJ39MZ3CGNJRN78C1YDTV3**: Remove notify_create from CODING_STANDARDS Memo
10. **01K7SJ39VEAW73G7G1N251F5VT**: Remove notify_create References from Builtin Prompts

## References

- MCP Notification Recommendations: `specification/mcp_notifications_recommendations.md`
- Current notify_create Implementation: `swissarmyhammer-tools/src/mcp/tools/notify/create/mod.rs`
- MCP Specification: https://spec.modelcontextprotocol.io/

## Breaking Change

This is a breaking change that removes a documented MCP tool. However:
- The tool is not widely used (only in refactoring workflow instructions)
- Native MCP notifications provide better functionality
- Migration path is straightforward (remove tool calls, add server notifications)
