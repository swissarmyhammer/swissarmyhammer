---
name: test-for-behavior-change
description: Changed behavior must be reflected in updated or new tests
---

# Test for Behavior Change

When you change what code does, change what the tests verify. A test suite that describes
yesterday's behavior is worse than no tests — it gives false confidence. If the behavior
changed, the tests must change with it.

## What to Check

When production code is modified, verify that tests are updated to match:

1. **Changed return values or output**: If a function now returns a different type,
   format, or value, existing tests must be updated to assert the new behavior. Stale
   assertions are silent lies.

2. **Changed function signatures**: Adding, removing, or reordering parameters means
   callers changed. Tests that call the function must be updated. If a parameter adds
   new behavior, new tests must cover it.

3. **Changed branching or control flow**: Adding a new conditional branch, case, or
   error path means new behavior exists. That new branch needs a test. Removing a
   branch means tests for it should be removed or updated.

4. **Changed error handling**: Switching from crashing to returning errors, changing
   error types, or adding new error conditions all change observable behavior. Tests
   must reflect the new error contract.

5. **Changed configuration or defaults**: If default values change, tests that relied
   on the old defaults must be updated. If new configuration options are added, tests
   should cover both the default and non-default cases.

6. **Removed functionality**: When code is deleted, tests that exercised it should
   either be removed (if the behavior is gone) or updated (if responsibility moved
   elsewhere). Orphaned tests that test deleted code are noise.

## What Passes

- Changing a function's return type and updating all tests to handle both success
  and failure cases
- Adding a new error case and adding a test that triggers it
- Changing a default timeout from 30s to 60s and updating the test that asserts the
  default value
- Removing a deprecated function and removing its tests
- Adding an optional parameter and adding tests for both with and without it

## What Fails

- Changing a function's behavior without touching any test file
- Adding a new code path with no test for the new path
- Changing an error message or error type without updating the test that asserts it
- Modifying validation logic without adding tests for the new validation rules
- Changing a public API contract where existing tests still assert the old contract

## Why This Matters

The TDD cycle is Red-Green-Refactor. If you're changing behavior, you should change the
test first (Red), then update the implementation (Green). Tests that don't track behavior
changes erode trust in the test suite — and a test suite nobody trusts is a test suite
nobody runs.
