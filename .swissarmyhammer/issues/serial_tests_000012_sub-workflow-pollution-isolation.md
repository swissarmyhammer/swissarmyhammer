# Remove Serial Tests from sub_workflow_state_pollution_tests.rs

Refer to /Users/wballard/github/swissarmyhammer/ideas/serial_tests.md

## Goal
Remove the `#[serial_test::serial]` attributes from 3 tests in `swissarmyhammer/src/workflow/actions_tests/sub_workflow_state_pollution_tests.rs` and replace them with proper `IsolatedTestEnvironment` usage.

## Current State
- File: `swissarmyhammer/src/workflow/actions_tests/sub_workflow_state_pollution_tests.rs`
- 3 tests with `#[serial_test::serial]` attributes at lines 34, 145, and 249

## Context
This file specifically tests for state pollution between sub-workflows, making isolation critical for both test correctness and parallel execution.

## Tasks
1. **Remove Serial Attributes**
   - Remove `#[serial_test::serial]` from all 3 test functions
   
2. **Implement Isolation**
   - Add `IsolatedTestEnvironment::new()` guard at start of each test
   - Update any hardcoded paths to use the isolated environment
   - Remove manual temp directory creation if present
   
3. **Fix Sub-Workflow State Isolation**
   - Ensure tests use isolated filesystem for sub-workflow execution
   - Update any global sub-workflow state caching to work with isolated environments
   - Remove any in-memory caches that prevent parallel execution
   - Fix any shared workflow state between tests that could cause pollution
   
4. **Verify State Pollution Prevention**
   - Ensure each test operates with completely isolated sub-workflow state
   - Remove any shared state between tests
   - Verify the pollution detection logic still works with isolation
   - Ensure tests can run in parallel without false positives/negatives

5. **Test Validation**
   - Run each test multiple times to ensure consistency
   - Run tests in parallel to verify no race conditions
   - Run all 3 tests together to ensure they don't interfere
   - Ensure all assertions pass and pollution detection works correctly

## Acceptance Criteria
- [ ] `#[serial_test::serial]` attributes removed from all 3 tests
- [ ] All tests use `IsolatedTestEnvironment::new()` pattern
- [ ] Tests pass when run individually  
- [ ] Tests pass when run in parallel with other tests
- [ ] State pollution detection logic still functions correctly
- [ ] No manual temp directory creation
- [ ] All existing test logic preserved
- [ ] Sub-workflow execution is properly isolated per test

## Implementation Notes
- These tests are specifically about state pollution, so isolation is doubly important
- Ensure sub-workflows use the isolated environment's directories
- Look for any global workflow state that could leak between tests
- The tests should verify that sub-workflows don't pollute each other's state, even with isolation
- Use the isolated environment's `.swissarmyhammer_dir()` and `.home_path()` methods consistently