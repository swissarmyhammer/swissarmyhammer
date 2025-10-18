# Remove notify_create from CLI MCP Tools Registration Test

## Parent Issue
Eliminate notify_create Tool and Replace with Native MCP Notifications (01K7SM38449JYA2KZP4KNKQ05X)

## Summary
Remove notify_create tool references from CLI integration tests.

## Location
`swissarmyhammer-cli/tests/mcp_tools_registration_test.rs`

## Tasks

1. Search file for "notify_create" references
2. Remove from expected tools list or test assertions
3. Verify tests pass after removal

## Dependencies

Must be completed **after**:
- Remove notify_create from Tool Registry

## Verification

- [ ] All references to notify_create removed
- [ ] Tests pass: `cd swissarmyhammer-cli && cargo nextest run mcp_tools`



## Proposed Solution

The issue requires removing `register_notify_tools` from the CLI MCP tools registration test. Based on my analysis:

**Current State:**
- `register_notify_tools` function exists in the codebase but is now a no-op (does not register any tools)
- The function is still being called in the test file at lines 8, 22, and 142
- The parent issue has already removed the actual notify_create tool

**Implementation Steps:**
1. Remove the import of `register_notify_tools` from line 8
2. Remove the call to `register_notify_tools(&mut registry)` from the first test (line 22)
3. Remove the call to `register_notify_tools(&mut registry)` from the second test (line 142)
4. Run tests to verify they still pass: `cd swissarmyhammer-cli && cargo nextest run mcp_tools`

**Expected Outcome:**
- Tests should continue to pass because `register_notify_tools` was already a no-op
- The test file will no longer reference the deprecated notify functionality
- Tool count assertions should remain valid as no tools are being lost



## Implementation Notes

Successfully removed all references to `register_notify_tools` from the CLI MCP tools registration test.

**Changes Made:**
1. Removed `register_notify_tools` from the import statement on line 8
2. Removed `register_notify_tools(&mut registry)` call from `test_mcp_tools_are_registered` function
3. Removed `register_notify_tools(&mut registry)` call from `test_cli_categories_are_available` function

**Test Results:**
- All 6 tests in the mcp_tools test suite passed
- No functionality was impacted as `register_notify_tools` was already a no-op function
- Tool count assertions remain valid (tools > 20 still passes)
- CLI category tests continue to work correctly

**File Modified:**
- `swissarmyhammer-cli/tests/mcp_tools_registration_test.rs`

**Verification:**
```
cargo nextest run --failure-output immediate --hide-progress-bar --status-level fail --final-status-level fail mcp_tools
```
Result: 6 tests run, 6 passed, 1113 skipped

The implementation is complete and all tests pass.