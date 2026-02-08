---
name: test-coverage-intent
description: Every public function and every error path must have a test that exercises it
---

# Test Coverage by Intent

dtolnay doesn't chase coverage percentages. He ensures every behavior the
code promises has a test that proves it. If a function can return an error,
there's a test that triggers that error. If a type has an edge case, there's
a test named after it.

## What to Check

When code is added or modified, check that corresponding tests exist:

1. **New public functions without tests**: Any new `pub fn` or `pub async fn`
   must have at least one test that calls it. If the function can fail, there
   must be a test for the success path and a test for at least one failure path.

2. **New error variants without tests**: Adding a variant to an error enum
   means there's a code path that produces it. That code path needs a test
   that asserts the specific error variant is returned.

3. **New match arms without tests**: If a new variant or case is added to a
   match expression, there should be a test that exercises that arm.

4. **Boundary conditions**: Functions that accept numeric ranges, optional
   values, or collections should have tests for:
   - Zero / empty / None
   - Single element
   - Typical case
   - Boundary values (max, min, overflow edge)

5. **Modified behavior without updated tests**: Changing what a function does
   without updating or adding tests means the test suite no longer describes
   the actual behavior.

## What Passes

- A new `pub fn parse(input: &str) -> Result<Ast>` paired with tests for
  valid input, empty input, malformed input, and edge-case input
- A new `Error::NotFound` variant with a test that triggers a not-found
  condition and asserts the variant
- Modified function logic with updated assertions in existing tests
- Private helper functions tested indirectly through the public API they support

## What Fails

- A new `pub fn` with zero tests anywhere in the crate
- A new error variant that no test ever produces
- A function that handles `None` specially but no test passes `None`
- An `if` branch added to handle a corner case with no test for that corner case
- Changing a function's return value from `Vec` to `Iterator` with no test updates
- Tests that only cover the happy path when the function has explicit error handling

## What This Rule Does NOT Require

- 100% line coverage -- untestable glue code and trivial accessors are fine
- Tests for `derive`-generated code
- Tests for private functions that are exercised through public API tests
- Integration tests for every unit -- unit tests are sufficient when they
  cover the behavior

## Why This Matters

serde's test suite has thousands of tests. When dtolnay adds a `#[serde(...)]`
attribute, there are tests for it working, tests for the error message when
it's misused, tests for interaction with other attributes, and tests for
edge cases. This is why serde can evolve rapidly without breaking users --
the test suite *is* the contract.
