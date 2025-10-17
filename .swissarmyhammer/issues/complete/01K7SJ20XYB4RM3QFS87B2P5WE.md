# Remove notify_create from Tool Registry

## Parent Issue
Eliminate notify_create Tool and Replace with Native MCP Notifications (01K7SM38449JYA2KZP4KNKQ05X)

## Summary
Remove notify_create tool registration from the MCP tool registry.

## Location
`swissarmyhammer-tools/src/mcp/tool_registry.rs`

## Tasks

1. Remove NotifyCreateTool struct and implementation (around line 1797-1810)
   ```rust
   // DELETE THIS:
   #[async_trait::async_trait]
   impl McpTool for NotifyCreateTool {
       fn name(&self) -> &'static str {
           "notify_create"
       }
       // ... rest of implementation
   }
   ```

2. Remove from `register_notify_tools()` function
   - Remove line that registers NotifyCreateTool
   - Keep the function if other notify tools exist, otherwise remove entirely

3. Remove any imports related to NotifyCreateTool

## Dependencies

Must be completed **after**:
- Remove notify_create Tool Implementation

## Verification

- [x] NotifyCreateTool struct removed
- [x] Tool not registered in registry
- [x] `cargo build` succeeds
- [x] `cargo clippy` shows no warnings about unused code
- [x] Tool does not appear in `sah serve` tool list

## Proposed Solution

I'll remove the `notify_create` tool from the MCP tool registry by:

1. Examining the current notify tools registration module at `swissarmyhammer-tools/src/mcp/tools/notify.rs` to understand the structure
2. Removing the `NotifyCreateTool` struct and its implementation if it exists in that file
3. Updating the `register_notify_tools()` function to exclude the `notify_create` tool
4. Verifying that all imports and registrations are clean

The changes will be made to ensure:
- The tool no longer appears in the registry
- No compilation warnings about dead code
- Clean clippy output
- The tool doesn't appear when listing tools via `sah serve`

## Implementation Notes

After examining the codebase, I found that **this issue has already been completed**. The work was done in a previous commit.

### Current State

1. **NotifyCreateTool removed**: No references to `NotifyCreateTool` struct exist in the codebase
2. **Empty registration function**: The `register_notify_tools()` function in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-tools/src/mcp/tools/notify/mod.rs` is empty and contains a comment explaining the tool has been removed:
   ```rust
   pub fn register_notify_tools(_registry: &mut ToolRegistry) {
       // No tools to register - notification functionality replaced by MCP progress notifications
   }
   ```
3. **Backward compatibility maintained**: The registration function still exists and is called from multiple places, but it doesn't register any tools. This prevents breaking changes to the codebase.

### Verification Results

- ✅ `cargo build` succeeds
- ✅ `cargo clippy --all-targets -- -D warnings` passes with no warnings
- ✅ No references to `NotifyCreateTool` in the codebase
- ✅ The `register_notify_tools()` function exists but registers nothing

### Documentation References

The following files still reference `notify_create` in documentation/prompts:
- `builtin/prompts/coding_standards.md.liquid:65`
- `builtin/prompts/are_tests_passing.md:14`
- `builtin/prompts/test.md:39-40`
- `specification/mcp_notifications_recommendations.md:16`
- `swissarmyhammer-tools/doc/src/features.md:304`
- `doc/src/SUMMARY.md:55`
- `doc/src/05-tools/overview.md:47`
- `doc/src/05-tools/notification-tools/create.md:1`

These documentation references are outside the scope of this issue, which specifically focuses on removing the tool from the registry in `tool_registry.rs`.

## Conclusion

This issue has already been completed in a previous commit (likely commit `fad9c219 refactor: remove notify_create tool in favor of native MCP notifications` based on the git log). No code changes are needed.
