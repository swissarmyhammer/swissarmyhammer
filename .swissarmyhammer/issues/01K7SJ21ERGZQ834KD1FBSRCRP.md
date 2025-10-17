# Remove notify_create from CLI MCP Tools Registration Test

## Parent Issue
Eliminate notify_create Tool and Replace with Native MCP Notifications (01K7SHXK4XKMDREMXG7BFJ8YJ7)

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
