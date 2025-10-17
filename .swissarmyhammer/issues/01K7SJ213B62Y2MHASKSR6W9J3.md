# Delete notify_create Integration Tests

## Parent Issue
Eliminate notify_create Tool and Replace with Native MCP Notifications (01K7SM38449JYA2KZP4KNKQ05X)

## Summary
Delete the integration test file for notify_create tool.

## Location
`swissarmyhammer-tools/tests/notify_integration_tests.rs`

## Tasks

1. Delete the entire file: `swissarmyhammer-tools/tests/notify_integration_tests.rs`
   - Contains ~350+ lines of tests for notify_create tool
   - Includes tests for:
     - Tool registration
     - Tool execution with various parameters
     - Validation errors
     - Different notification levels
     - Complex context handling
     - Unicode and special characters
     - Error recovery
     - Rate limiting
     - Resource cleanup
     - Realistic usage scenarios

2. Verify tests pass after deletion
   - Run `cargo nextest run --fail-fast`
   - Ensure no other tests depend on this file

## Dependencies

Must be completed **after**:
- Remove notify_create from Tool Registry

## Verification

- [ ] File deleted
- [ ] `cargo nextest run --fail-fast` succeeds
- [ ] No test failures from missing dependencies
