# Remove notify_create from MCP Server Parity Tests

## Parent Issue
Eliminate notify_create Tool and Replace with Native MCP Notifications (01K7SM38449JYA2KZP4KNKQ05X)

## Summary
Remove notify_create from the expected tools list in MCP server parity tests.

## Location
`swissarmyhammer-tools/tests/mcp_server_parity_tests.rs`

## Tasks

1. Find the expected tools list (around line 99)
2. Remove the line: `"notify_create",`
3. Verify test passes after removal

## Code Change

```rust
// Remove this line from the expected tools array:
"notify_create",
```

## Dependencies

Must be completed **after**:
- Remove notify_create from Tool Registry

## Verification

- [ ] Line removed from expected tools list
- [ ] Test `test_unified_server_tool_parity` passes
- [ ] `cargo nextest run mcp_server_parity` succeeds

## Proposed Solution

After analyzing the code, I found that:

1. The `register_notify_tools` function in `swissarmyhammer-tools/src/mcp/tools/notify/mod.rs` (line 52) is now a no-op - it doesn't register any tools
2. The test file at `swissarmyhammer-tools/tests/mcp_server_parity_tests.rs` still calls `register_notify_tools(&mut registry)` on line 114
3. The test file also imports `register_notify_tools` on line 11
4. The expected tools list in `get_http_static_tools()` function (lines 74-100) correctly does NOT include "notify_create"

However, there's a mismatch - the issue says to remove "notify_create" from line 99, but I don't see it there. Let me verify what the actual issue is by running the test to see if it's failing.

### Steps to implement:

1. First, run the test to see the current state
2. Based on the test output, determine what needs to be removed
3. Remove the `register_notify_tools` call and import since it's now a no-op
4. Run the test again to verify it passes

## Analysis Complete

The issue description is outdated. According to git history (commit fad9c219), `notify_create` was already removed from the expected tools list. The current state is:

1. ✅ `notify_create` is already removed from the expected tools list
2. ✅ All 3 tests in mcp_server_parity_tests.rs are passing
3. ⚠️ The import and call to `register_notify_tools` are still present but harmless (it's a no-op)

**Current Issue**: The remaining cleanup is to remove the now-unnecessary `register_notify_tools` import and call from the test file, since it no longer registers any tools.

### Actual Implementation Steps:

1. Remove `register_notify_tools` from the imports on line 11
2. Remove the `register_notify_tools(&mut registry);` call on line 114
3. Run tests to verify everything still passes


## Implementation Complete

Successfully removed the now-unnecessary `register_notify_tools` references from the MCP server parity tests.

### Changes Made

1. **Removed import** (line 11): Removed `register_notify_tools` from the import list
2. **Removed function call** (line 114): Removed `register_notify_tools(&mut registry);` from `get_stdin_registry_tools()`

### Files Modified

- `swissarmyhammer-tools/tests/mcp_server_parity_tests.rs`

### Verification Results

✅ All 3 tests in `mcp_server_parity_tests.rs` pass:
- `test_http_stdin_mcp_tool_parity`
- `test_mcp_tool_definitions_return_sufficient_tools`
- `test_mcp_tool_definitions_include_core_tools`

✅ Project builds successfully with no compilation errors

### Rationale

The `register_notify_tools` function is now a no-op (does nothing) since the `notify_create` tool was removed in commit fad9c219. The function exists only for backward compatibility but doesn't register any tools. Removing these references cleans up the test code and makes it clear that no notify tools are being registered.


## Code Review Completion

All code review action items have been completed successfully.

### Additional Fix Applied

During code review, a compilation error was discovered in an unrelated test file:

**File**: `swissarmyhammer-tools/tests/test_issue_show_enhanced.rs:84`

**Issue**: Struct field name mismatch
- Struct declaration on line 33: `_temp_dir: TempDir` 
- Field initialization on line 84: `temp_dir,` (missing underscore)

**Fix Applied**: Changed line 84 from `temp_dir,` to `_temp_dir: temp_dir,` to match the struct field name.

### Final Verification

✅ **Compilation**: All packages build successfully
✅ **Tests**: 3298 tests run, 3298 passed, 3 skipped  
✅ **Linting**: `cargo clippy` passes with no warnings
✅ **Formatting**: `cargo fmt --all` applied

The compilation error fix ensures the entire test suite can build and run. This was a blocking issue preventing verification of the notify_create removal work.
