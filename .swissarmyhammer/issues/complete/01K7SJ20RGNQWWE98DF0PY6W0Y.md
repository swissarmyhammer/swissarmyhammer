# Remove notify_create Tool Implementation

## Parent Issue
Eliminate notify_create Tool and Replace with Native MCP Notifications (01K7SM38449JYA2KZP4KNKQ05X)

## Summary
Delete the notify_create tool implementation from the codebase.

## Location
`swissarmyhammer-tools/src/mcp/tools/notify/create/mod.rs`

## Tasks

1. Delete the entire directory: `swissarmyhammer-tools/src/mcp/tools/notify/create/`
   - Includes `mod.rs` and `description.md`

2. Remove from parent module
   - File: `swissarmyhammer-tools/src/mcp/tools/notify/mod.rs`
   - Remove: `pub mod create;` export

3. Verify no compilation errors after deletion

## Dependencies

Must be completed **after**:
- Phase 1: Implement MCP Progress Notification Infrastructure (01K7SHZ4203SMD2C6HTW1QV3ZP)

## Verification

- [ ] Directory deleted
- [ ] Module export removed
- [ ] `cargo build` succeeds
- [ ] No references to `notify/create` remain in source code



## Proposed Solution

Based on code analysis, I've identified the following files that reference `notify_create`:

### Files to Modify/Delete:
1. **Delete**: `swissarmyhammer-tools/src/mcp/tools/notify/create/` (entire directory)
2. **Update**: `swissarmyhammer-tools/src/mcp/tools/notify/mod.rs` - Remove module and registration
3. **Update**: Test files that expect `notify_create` tool to exist:
   - `swissarmyhammer-tools/tests/notify_integration_tests.rs` - Delete entire file
   - `swissarmyhammer-tools/tests/mcp_server_parity_tests.rs` - Remove from expected tools list
   - `swissarmyhammer-cli/tests/sah_serve_integration_test.rs` - Remove from expected tools list
   - `swissarmyhammer-cli/tests/mcp_tools_registration_test.rs` - Remove from expected tools list
4. **Update**: `swissarmyhammer-tools/src/mcp/tool_registry.rs` - Remove from any hardcoded lists

### Implementation Steps:

1. Remove `notify_create` from test expectation lists in:
   - `swissarmyhammer-cli/tests/sah_serve_integration_test.rs:68,251`
   - `swissarmyhammer-cli/tests/mcp_tools_registration_test.rs:54`
   - `swissarmyhammer-tools/tests/mcp_server_parity_tests.rs:99`
   - `swissarmyhammer-tools/src/mcp/tool_registry.rs:1834`

2. Delete entire integration test file:
   - `swissarmyhammer-tools/tests/notify_integration_tests.rs`

3. Update `swissarmyhammer-tools/src/mcp/tools/notify/mod.rs`:
   - Remove `pub mod create;` line
   - Remove `registry.register(create::NotifyTool::new());` from `register_notify_tools`
   - Update module documentation to remove references to the create tool

4. Delete the entire directory:
   - `swissarmyhammer-tools/src/mcp/tools/notify/create/`

5. Verify compilation with `cargo build`

6. Run tests with `cargo nextest run --failure-output immediate --hide-progress-bar --status-level fail --final-status-level fail`



## Implementation Notes

### Changes Made

1. **Removed `notify_create` from test expectation lists** in the following files:
   - `swissarmyhammer-cli/tests/sah_serve_integration_test.rs:68` - Removed from EXPECTED_SAMPLE_TOOLS array
   - `swissarmyhammer-cli/tests/sah_serve_integration_test.rs:250` - Changed test_minimal_tool_executions to use `memo_get_all_context` instead of `notify_create`
   - `swissarmyhammer-cli/tests/mcp_tools_registration_test.rs:54` - Removed from expected_tools array
   - `swissarmyhammer-tools/tests/mcp_server_parity_tests.rs:99` - Removed from HTTP static tools list
   - `swissarmyhammer-tools/tests/mcp_server_parity_tests.rs:139,148` - Updated minimum tool count from 26 to 25

2. **Deleted entire integration test file**:
   - `swissarmyhammer-tools/tests/notify_integration_tests.rs` - Deleted file completely

3. **Updated `swissarmyhammer-tools/src/mcp/tools/notify/mod.rs`**:
   - Removed `pub mod create;` export
   - Updated `register_notify_tools` function to no longer register any tools
   - Added documentation noting that notification functionality has been replaced by MCP progress notifications
   - Function kept for backward compatibility but does not register tools

4. **Deleted the entire directory**:
   - `swissarmyhammer-tools/src/mcp/tools/notify/create/` - Removed directory and all contents

5. **Cleaned up test mock tools** in `swissarmyhammer-tools/src/mcp/tool_registry.rs`:
   - Removed `struct NotifyCreateTool` declaration
   - Removed `impl McpTool for NotifyCreateTool` implementation
   - Removed assertions referencing `NotifyCreateTool` from test functions

### Verification

- ✅ `cargo build` succeeded without errors
- ✅ All 3316 tests passed (3 skipped)
- ✅ No references to `notify_create` remain in the codebase
- ✅ Module structure is clean and consistent

### Impact

The `notify_create` tool has been successfully removed from the codebase. The functionality has been replaced by native MCP progress notifications as implemented in the parent issue.
