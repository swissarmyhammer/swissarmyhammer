---
name: no-hard-code
description: Detect the misuse of hard coding to make tests appear to pass
severity: error
trigger: Stop
tags:
  - code-quality
  - testing
timeout: 30
---

Check for implementations that hard-code values just to pass tests rather than implementing correct logic.

DO implement a solution that works correctly for all valid inputs, not just the test cases.
DO NOT hard-code values or create solutions that only work for specific test inputs.

Look for:
- Functions that return literal values matching test expectations, the classic 'return 42;' bug
- Conditional logic that checks for specific test input values in main non-test code
- Magic return values that only work for known test cases, hard coding to pass a test
- Pattern matching on exact test input strings/values

It is acceptable to have 'hard coded' or constant values in unit test assertions.
