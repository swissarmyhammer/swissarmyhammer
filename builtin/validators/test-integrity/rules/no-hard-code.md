---
name: no-hard-code
description: Detect hard-coded values that make a test appear to pass
---

# No Hard-Coded Test Values Validator

You are a code quality validator. Check for code that hard-codes values to pass a test.

## What to Check

Check the file content for patterns that hard-code values instead of using correct logic:

1. **Literal Return Values**: A function that returns a literal value matching the test's expected value. This is the classic `return 42;` bug.
2. **Test Input Matching**: Conditional logic in production code that checks for a specific test input value.
3. **Magic Returns**: A return value that only works for known test cases.
4. **Pattern Matching on Test Data**: An exact match on a test input string or value, in code that is not test code.

## Why This Matters

- A hard-coded solution passes the test but fails in production with real data.
- It shows that the implementation is not complete.
- It creates false confidence in test coverage.
- A hard-coded value that is correct by chance can hide a real bug.

## Exceptions (Do Not Flag)

- The literal in the expected-value position of an assertion. For example, in `assert_eq!(call(input), 42)`, the `42` is the value the test checks. It is not a hard-coded production return value.
- A constant that is truly constant, for example a configuration value or a limit.
- A lookup table that is correct for all inputs.
- A default value that fits the domain.

Note: Do not exempt a file just because its filename contains `test`, `_test`, `test_`, `.spec.`, or `.test.`. This rule catches code that hard-codes a return value to satisfy a test. This anti-pattern can appear in any file, including test files. For example, a stub helper in a `tests/` module can return the expected value directly to make a later assertion pass. Flag `return 42`, `if input == "known_value" { return ... }`, and similar shortcuts wherever they appear. The assertion-expected-value carve-out applies only to the literal in the assertion comparison position. It does not apply to the function body under test.
