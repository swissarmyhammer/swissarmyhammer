---
name: test-describes-intent
description: Tests should describe expected behavior, not implementation details
---

# Test Describes Intent

A good test is a specification. Someone reading the test name and body should understand
what the code promises without reading the implementation. Tests that mirror implementation
details break on every refactor and tell you nothing about whether the code is correct.

## What to Check

Examine test code for clarity of intent:

1. **Test naming**: Test names should describe the behavior being verified, not the
   function being called. The name should read like a requirement.
   - Good: `test_rejects_email_without_at_sign`, `test_returns_empty_list_when_no_matches`
   - Bad: `test_validate`, `test_1`, `test_process_data`, `test_function_works`

2. **Arrange-Act-Assert structure**: Each test should have a clear setup phase, a
   single action, and focused assertions. When these phases are tangled together,
   the test's purpose becomes obscure.

3. **One behavior per test**: Each test should verify one logical behavior. A test
   that checks five unrelated properties is five tests crammed into one — when it
   fails, you don't know which behavior broke.

4. **Testing public contracts, not internals**: Tests should exercise the public API
   and assert observable outcomes. Asserting internal state, private field values, or
   call counts on collaborators couples tests to implementation. Refactoring should
   not break tests if behavior is unchanged.

5. **Meaningful assertions**: Assertions should verify specific expected values.
   Asserting that a result is not null proves almost nothing. Asserting that the
   result has a specific status code, value, or shape proves the contract.

## What Passes

- `test_parse_returns_error_for_malformed_json` — name describes the scenario and expected outcome
- A test with clear variable names for input, action, and expected output
- Tests organized by behavior: one test for the happy path, one for each error case
- Tests that assert return values, output, or side effects visible through the public API
- Integration tests that verify end-to-end behavior through the system boundary

## What Fails

- `test_1`, `test_new`, `test_it_works` — names that tell you nothing
- A single test with 15 assertions covering unrelated behaviors
- Tests that access private fields or internal data structures to verify correctness
- Tests that assert the exact sequence of internal method calls (order-dependent mocking)
- Tests that only assert success/failure without checking the actual value
- Tests that duplicate the implementation logic to compute the expected value

## Why This Matters

Tests outlive the code they test. When you refactor, rename, or restructure, the tests
are your safety net. Tests that describe intent survive refactors because they verify
*what* the code does, not *how* it does it. Tests coupled to implementation break on
every change and become a maintenance burden instead of a safety net.
