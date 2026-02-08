---
name: testing-as-documentation
description: Tests should demonstrate API usage, cover edge cases exhaustively, and name themselves after what they prove
---

# Testing as Documentation

dtolnay treats tests as the primary documentation for how code behaves. A test
suite should read like a specification. Each test demonstrates one behavior,
names itself after that behavior, and a new contributor should be able to
understand the API by reading tests alone.

## What to Check

Examine test functions, test organization, and test quality in the changed code:

1. **Test naming**: Test names should describe the scenario and expected behavior,
   not be numbered or vague. The name is the first thing you see in a failure report.
   - Bad: `test_parse`, `test_1`, `test_basic`, `it_works`
   - Good: `test_parse_empty_input_returns_none`, `test_trailing_comma_is_allowed`

2. **Regression tests named after issues**: When a test exists because of a bug
   report, name it after the issue. dtolnay uses `test_issue_123` or
   `test_issue_description` so you can trace back to the original report.

3. **One behavior per test**: A test function should assert one logical behavior.
   If you need a comment like "now test the other case" inside a test, split it.

4. **No `#[should_panic]`**: Use `Result`-returning tests or assert on the error
   value directly. `#[should_panic]` is fragile -- it passes if *any* panic
   occurs, including one from a completely unrelated bug.

5. **`trybuild` for compile-time tests**: If code is supposed to produce a
   compiler error (e.g., proc macro error messages, trait bound failures),
   use `trybuild` to assert the exact error message. Don't leave compile-time
   guarantees untested.

6. **Self-contained tests**: Each test should set up its own state. Shared
   mutable test fixtures across tests create ordering dependencies and
   flaky failures.

7. **Edge case coverage**: Tests should explicitly cover boundary conditions:
   empty input, single element, maximum values, unicode, nested structures.
   If a type has interesting edge cases, they should have named tests.

## What Passes

- `#[test] fn test_deserialize_missing_field_uses_default()`
- `#[test] fn test_issue_2107_lifetime_in_nested_borrow()`
- Tests that return `Result<()>` and use `?` for clean error propagation
- `trybuild::TestCases` for proc macro compile-error testing
- `assert_eq!` with descriptive values showing expected vs actual
- Dedicated test functions for empty input, single element, and typical cases
- Test helper functions that are private to the test module

## What Fails

- `#[test] fn test1()`, `#[test] fn test_it()`, `#[test] fn basic()`
- `#[should_panic] fn test_invalid_input()` (assert on the error instead)
- A single `#[test]` function with 15 assert statements testing different behaviors
- Tests that depend on being run in a specific order
- Tests that mutate global/static state without cleanup
- Missing tests for obvious edge cases (empty string, zero, None)
- Tests that only cover the happy path

## Why This Matters

serde's test suite is its real documentation. When someone asks "does serde
handle missing fields?" the answer is a test named `test_missing_field`. When
issue #2107 reported a lifetime bug, dtolnay added `test_issue_2107` -- years
later, anyone can find exactly why that code path exists. The test suite is a
living, machine-verified specification.
