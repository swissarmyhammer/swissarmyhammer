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
