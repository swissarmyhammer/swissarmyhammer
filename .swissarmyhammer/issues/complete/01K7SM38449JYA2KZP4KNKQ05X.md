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



## Investigation Results (2025-10-18)

### Current State Analysis

After thorough investigation of the codebase, I've determined that **this issue has already been substantially completed**. Here are my findings:

#### 1. notify_create Tool Removal: ✅ COMPLETE

- **File**: `swissarmyhammer-tools/src/mcp/tools/notify/mod.rs`
- **Status**: The `notify_create` tool implementation has been removed
- **Current State**: Only a stub registration function remains for backward compatibility:
  ```rust
  pub fn register_notify_tools(_registry: &mut ToolRegistry) {
      // No tools to register - notification functionality replaced by MCP progress notifications
  }
  ```

#### 2. MCP Progress Notification Infrastructure: ✅ COMPLETE

The native MCP progress notification infrastructure is fully implemented:

- **ProgressNotification struct**: `swissarmyhammer-tools/src/mcp/progress_notifications.rs:63`
- **ProgressSender**: `swissarmyhammer-tools/src/mcp/progress_notifications.rs:101`
- **ToolContext integration**: `swissarmyhammer-tools/src/mcp/tool_registry.rs:326`
  - `progress_sender: Option<ProgressSender>` field added
  - Builder method `with_progress_sender()` available

#### 3. Tool Migration Status: ✅ SUBSTANTIALLY COMPLETE

The following tools have already been migrated to use native MCP progress notifications:

**HIGH Priority (Complete):**
- ✅ `shell_execute` - Sends streaming output notifications (mod.rs:840, 875, 920, 938, 1197, 1347)
- ✅ `search_index` - Sends indexing progress (mod.rs:105, 197)
- ✅ `web_search` - Sends search and fetch progress (mod.rs:295, 313, 367, 429)

**MEDIUM Priority (Complete):**
- ✅ `web_fetch` - Sends fetch/conversion progress (mod.rs:312, 336, 360)
- ✅ `outline_generate` - Sends parsing progress (mod.rs:161, 209, 268, 296, 348)

**LOW Priority (Complete):**
- ✅ `rules_check` - Sends checking progress (mod.rs:214, 235, 270, 315)
- ✅ `files_glob` - Sends matching progress (mod.rs:125, 156)
- ✅ `files_grep` - Sends search progress (mod.rs:596, 613, 627, 646)

#### 4. Documentation Status: ✅ MOSTLY COMPLETE

Per `DOCUMENTATION_REVIEW.md:27`:
- ✅ Complete removal of notify_create references from documentation

#### 5. Remaining References: 2 Files Only

Only two files still mention `notify_create`:
1. `specification/mcp_notifications_recommendations.md:16` - Historical spec document
2. `swissarmyhammer-tools/DOCUMENTATION_REVIEW.md:27` - Notes the removal

### What's Left to Do

The issue lists 10 implementation tasks, but investigation shows:

**Already Complete:**
- ✅ Task 1 (Prerequisite): MCP Progress Notification Infrastructure is implemented
- ✅ Tasks 2-6: Tool implementation and tests have been removed
- ✅ Tasks 7-8: Documentation updated
- ✅ Task 9: CODING_STANDARDS memo already removed (verified - no references found)

**Potentially Remaining:**
- ⚠️ Task 10: Remove notify_create references from builtin prompts (needs verification)

### Verification Needed

1. **Check builtin prompts** for any notify_create references
2. **Verify tool registry** doesn't still register notify_create
3. **Confirm all tests pass** with the removal

### Recommendation

This issue appears to be **95%+ complete**. The notify_create tool has been successfully removed and replaced with native MCP progress notifications across all high and medium priority tools. 

**Next Steps:**
1. Verify builtin prompts have no notify_create references
2. Remove the empty `register_notify_tools()` stub function if no longer needed
3. Optionally update specification document to mark as "Implemented" rather than "Proposed"
4. Run full test suite to confirm no regressions



## Implementation Completed (2025-10-18)

### Work Performed

Successfully completed the removal of all remaining vestiges of the `notify_create` tool from the codebase.

#### Files Modified

1. **swissarmyhammer-tools/src/mcp/server.rs**
   - Removed `register_notify_tools` from imports
   - Removed `register_notify_tools(&mut tool_registry)` call from tool registration

2. **swissarmyhammer-tools/src/mcp/tool_registry.rs**
   - Removed `register_notify_tools()` function definition
   - Removed call to `notify::register_notify_tools(registry)`

3. **swissarmyhammer-tools/src/mcp/mod.rs**
   - Removed `register_notify_tools` from public exports

4. **swissarmyhammer-tools/src/mcp/tools/mod.rs**
   - Removed `pub mod notify;` module declaration

5. **swissarmyhammer-tools/src/mcp/tools/notify/** (Directory)
   - Completely removed the notify tool module directory

### Verification Results

✅ **Build Status**: Clean compilation with no errors  
✅ **Test Suite**: All 3319 tests passed (32 slow), 3 skipped  
✅ **Code Formatting**: Applied `cargo fmt --all`  
✅ **No References**: Only 2 historical references remain in specification docs

### Status Summary

| Task | Status | Notes |
|------|--------|-------|
| notify_create tool removal | ✅ Complete | All code removed |
| MCP progress notifications | ✅ Complete | Infrastructure fully implemented |
| Tool migration (HIGH priority) | ✅ Complete | shell_execute, search_index, web_search |
| Tool migration (MEDIUM priority) | ✅ Complete | web_fetch, outline_generate |
| Tool migration (LOW priority) | ✅ Complete | rules_check, files_glob, files_grep |
| Documentation updates | ✅ Complete | All tool docs updated |
| CODING_STANDARDS update | ✅ Complete | No references found |
| Builtin prompts cleanup | ✅ Complete | No references found |
| Test suite verification | ✅ Complete | 3319/3319 tests passing |

### Benefits Achieved

1. **Standard Compliance**: Now using MCP's native notification mechanism
2. **Better UX**: Notifications appear immediately without LLM overhead
3. **More Efficient**: No tool call latency for progress updates
4. **Cleaner Architecture**: Separates progress notifications from tool interface
5. **Future-Proof**: Aligns with MCP specification evolution

### Tools Using Progress Notifications

The following tools are actively sending MCP progress notifications:

- **shell_execute**: Streaming command output (6 notification points)
- **search_index**: Indexing progress (2 notification points)
- **web_search**: Search and content fetch progress (4 notification points)
- **web_fetch**: URL fetch and conversion progress (3 notification points)
- **outline_generate**: File parsing progress (5 notification points)
- **rules_check**: Rule checking progress (4 notification points)
- **files_glob**: File matching progress (2 notification points)
- **files_grep**: Content search progress (4 notification points)

### Conclusion

This issue is now **100% COMPLETE**. The `notify_create` tool has been fully removed and replaced with native MCP progress notifications throughout the codebase. All tests pass, documentation is updated, and the system is using standard MCP notification mechanisms.
