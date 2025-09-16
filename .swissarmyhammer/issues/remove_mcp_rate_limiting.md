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