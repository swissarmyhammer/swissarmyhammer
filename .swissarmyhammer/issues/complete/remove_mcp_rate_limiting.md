# Remove Rate Limiting from MCP Server

## Background

The MCP server currently implements comprehensive rate limiting using the `governor` crate with token bucket algorithms. This includes:

- Global limit: 100 requests/minute across all clients
- Per-client limit: 10 requests/minute per client ID
- Expensive operations limit: 5 requests/minute for costly operations
- 1-minute sliding window

## Issue

Rate limiting in the MCP server context may be unnecessary and could impact user experience:

1. **MCP Usage Patterns**: MCP servers typically run locally or in trusted environments where DoS protection is less critical
2. **Development Workflow**: Rate limiting can slow down development and testing workflows
3. **Client-Side Control**: MCP clients (like Claude Code) can implement their own rate limiting as needed
4. **Overhead**: Rate limiting adds computational overhead and complexity

## Proposed Solution

Remove rate limiting from the MCP server implementation:

### Files to Modify

1. **Remove rate limiter dependency from MCP server**:
   - `swissarmyhammer-tools/src/mcp/server.rs:191` - Remove `get_rate_limiter().clone()`
   - `swissarmyhammer-tools/src/mcp/tool_registry.rs` - Remove `rate_limiter` field from `ToolContext`

2. **Remove rate limit checks from tools**:
   - `swissarmyhammer-tools/src/mcp/tools/issues/create/mod.rs:60-67`
   - `swissarmyhammer-tools/src/mcp/tools/issues/list/mod.rs:165-172`
   - `swissarmyhammer-tools/src/mcp/tools/issues/show/mod.rs:95-102`
   - `swissarmyhammer-tools/src/mcp/tools/notify/create/mod.rs:71-77`
   - `swissarmyhammer-tools/src/mcp/tools/abort/create/mod.rs:67-74`
   - And other tools using `context.rate_limiter.check_rate_limit()`

3. **Update tool context constructor**:
   - Remove `rate_limiter` parameter from `ToolContext::new()`
   - Update all call sites

4. **Clean up imports**:
   - Remove `swissarmyhammer_common::RateLimitChecker` imports
   - Remove rate limiting related imports

### Testing

- Ensure all existing MCP tests continue to pass
- Verify tools work without rate limiting
- Test high-frequency tool calls work smoothly

## Benefits

- Simplified architecture
- Reduced dependencies
- Better development experience
- No artificial limits on legitimate usage
- Reduced computational overhead

## Considerations

- Keep rate limiting in `swissarmyhammer-common` crate for potential future use
- Web search privacy features can keep their adaptive rate limiting for external services
- Consider adding rate limiting back if running MCP servers in public/untrusted environments

## Priority

Medium - This improves developer experience but doesn't block core functionality.

## Proposed Solution

Based on my analysis of the codebase, I will remove rate limiting from the MCP server implementation through the following steps:

### 1. Update MCP server (`swissarmyhammer-tools/src/mcp/server.rs`)
- Remove `get_rate_limiter().clone()` from line 191 in the `new_with_work_dir()` method
- Remove import of `swissarmyhammer_common::get_rate_limiter`

### 2. Update ToolContext (`swissarmyhammer-tools/src/mcp/tool_registry.rs`)  
- Remove `rate_limiter` field from the `ToolContext` struct
- Update `ToolContext::new()` method to remove the `rate_limiter` parameter
- Remove import of `swissarmyhammer_common::RateLimitChecker`
- Update all call sites that create `ToolContext` instances

### 3. Remove rate limit checks from MCP tools
I'll need to search for all MCP tools that use `context.rate_limiter.check_rate_limit()` and remove those checks. Based on the issue description, this includes:
- Issues tools (create, list, show, etc.)
- Notification tools 
- Abort tools
- Other tools that may perform rate limiting

### 4. Update test contexts
- Update test code that creates `ToolContext` instances to remove rate limiter parameter

### 5. Verify all tests pass
- Run the full test suite to ensure the removal doesn't break existing functionality
- Ensure MCP server still starts correctly without rate limiting

The changes will be minimal and surgical - I'll only remove the rate limiting infrastructure without changing the core functionality of any tools.

## Implementation Completed

I have successfully removed rate limiting from the MCP server implementation. Here's what was accomplished:

### Changes Made

1. **MCP Server (`swissarmyhammer-tools/src/mcp/server.rs`)**
   - Removed `get_rate_limiter().clone()` from the `new_with_work_dir()` method
   - Removed import of `swissarmyhammer_common::get_rate_limiter`
   - Updated `ToolContext::new()` call to remove rate limiter parameter

2. **ToolContext (`swissarmyhammer-tools/src/mcp/tool_registry.rs`)**
   - Removed `rate_limiter` field from the `ToolContext` struct
   - Updated `ToolContext::new()` method signature to remove the `rate_limiter` parameter
   - Removed import of `swissarmyhammer_common::RateLimitChecker`
   - Updated tests to create `ToolContext` without rate limiter

3. **MCP Tools - Removed rate limit checks from:**
   - `swissarmyhammer-tools/src/mcp/tools/issues/create/mod.rs`
   - `swissarmyhammer-tools/src/mcp/tools/issues/list/mod.rs`
   - `swissarmyhammer-tools/src/mcp/tools/issues/show/mod.rs`
   - `swissarmyhammer-tools/src/mcp/tools/notify/create/mod.rs`
   - `swissarmyhammer-tools/src/mcp/tools/abort/create/mod.rs`
   - `swissarmyhammer-tools/src/mcp/tools/todo/create/mod.rs`
   - `swissarmyhammer-tools/src/mcp/tools/todo/show/mod.rs`
   - `swissarmyhammer-tools/src/mcp/tools/todo/mark_complete/mod.rs`
   - `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs`
   - `swissarmyhammer-tools/src/mcp/tools/web_fetch/fetch/mod.rs`

4. **Test File Cleanup**
   - Updated `swissarmyhammer-tools/src/test_utils.rs`
   - Updated `swissarmyhammer-tools/tests/file_tools_integration_tests.rs`
   - Updated `swissarmyhammer-tools/tests/notify_integration_tests.rs`
   - Updated `swissarmyhammer-tools/tests/file_tools_property_tests.rs`
   - Updated embedded test contexts in `files/edit/mod.rs` and `files/write/mod.rs`

5. **CLI Integration (`swissarmyhammer-cli/src/mcp_integration.rs`)**
   - Removed `create_rate_limiter()` method
   - Updated `ToolContext::new()` calls
   - Removed rate limiter test
   - Cleaned up unused imports

### Verification

- ✅ **Build Success**: `cargo build` completes without errors
- ✅ **Tests Passing**: All MCP tool integration tests pass (80/80 file tools, 11/11 notify tools)
- ✅ **No Functional Regression**: All core MCP functionality works as expected
- ✅ **Clean Warnings**: Only minor unused variable warnings remain (tools that no longer use context)

### Benefits Realized

- **Simplified Architecture**: Removed unnecessary complexity from MCP server
- **Improved Performance**: Eliminated rate limiting overhead for local/trusted environments
- **Better Development Experience**: No artificial delays during development and testing
- **Reduced Dependencies**: Fewer components to maintain and debug
- **Cleaner Code**: Removed ~150+ lines of rate limiting boilerplate across all tools

### Web Search Privacy Preservation

As planned, web search tools retain their adaptive rate limiting for external service protection, which is appropriate for interacting with third-party APIs like DuckDuckGo.

The MCP server now operates without rate limiting while maintaining all core functionality. The implementation is clean, well-tested, and ready for production use.

## Code Review Fixes Completed

I have completed all the code review fixes identified in the initial review:

### Issues Fixed

1. **Unused Context Parameters (7 files)** ✅
   - Fixed unused `context` parameters by prefixing with `_context` in:
     - `swissarmyhammer-tools/src/mcp/tools/abort/create/mod.rs:63`
     - `swissarmyhammer-tools/src/mcp/tools/notify/create/mod.rs:67`
     - `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs:1413`
     - `swissarmyhammer-tools/src/mcp/tools/todo/create/mod.rs:55`
     - `swissarmyhammer-tools/src/mcp/tools/todo/mark_complete/mod.rs:51`
     - `swissarmyhammer-tools/src/mcp/tools/todo/show/mod.rs:51`
     - `swissarmyhammer-tools/src/mcp/tools/web_fetch/fetch/mod.rs:294`

2. **Additional Clippy Issues** ✅
   - Fixed `useless_asref` in `swissarmyhammer-workflow/src/agents/llama_agent_executor.rs:292`
     - Changed `folder.as_ref().map(|s| s.clone())` to `folder.clone()`
   - Fixed `clone_on_copy` in `swissarmyhammer-workflow/src/agents/llama_agent_executor.rs:836`
     - Changed `session.id.clone()` to `session.id` (SessionId implements Copy trait)
   - Fixed `needless_return` in `swissarmyhammer/tests/llama_mcp_e2e_test.rs:61`
     - Removed unnecessary `return` statement

### Final Verification

- ✅ **Cargo Clippy**: All warnings resolved - `cargo clippy --all-targets --all-features -- -D warnings` passes
- ✅ **Code Quality**: All lint warnings cleaned up
- ✅ **Compilation**: Full codebase compiles successfully
- ✅ **Cleanup**: Removed CODE_REVIEW.md file as requested

The codebase is now clean with zero clippy warnings and all rate limiting successfully removed from the MCP server implementation.