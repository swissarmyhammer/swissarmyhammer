---
name: test-before-implementation
description: New production code must be accompanied by tests
---

# Test Before Implementation

In TDD, the test comes first. When new production code is written, corresponding tests
must exist or be added in the same changeset. Code without tests is unverified behavior —
it might work today, but nothing proves it will work tomorrow.

## What to Check

When production code is added, verify that matching tests exist:

1. **New functions or methods without tests**: Any new public function, method, or
   endpoint must have at least one test that exercises it. If the function can fail,
   there must be a test for the success path and at least one failure path.

2. **New types with logic**: Types that contain behavior (methods, computed properties,
   validation) need tests that verify that behavior. Pure data types without logic
   are exempt.

3. **New modules or files without test coverage**: A new source file with production
   logic should have a corresponding test file or test module. The test file should
   exercise the module's public interface.

4. **New error paths without failure tests**: If new code introduces error handling,
   branching, or validation, there must be tests that trigger those paths. Untested
   error paths are the most common source of production bugs.

5. **New API endpoints or command handlers**: These are entry points to behavior. Each
   needs tests covering valid input, invalid input, and edge cases.

## What Passes

- A new function `validate_email` paired with tests for valid emails, empty strings,
  missing `@`, and Unicode edge cases
- A new REST endpoint with tests for 200, 400, 404, and 500 responses
- A new type with a constructor and validation, paired with tests for valid
  construction and each validation failure
- A refactored function that changes internal implementation but not observable behavior,
  where existing tests still cover all behavior
- Private helper functions tested indirectly through the public API they support

## What Fails

- A new public function with no tests anywhere in the project
- A new module with ten functions and zero tests
- A new error type with five cases, none of which any test triggers
- A new CLI subcommand with no integration test
- Code that handles missing values, empty strings, or zero-length collections, with
  no test exercising those cases
- A new validation function where only the happy path is tested

## Why This Matters

Tests written after implementation tend to verify what the code does, not what it should
do. Writing the test first forces you to think about the interface, the edge cases, and
the contract before writing a single line of production code. The test is the spec.
