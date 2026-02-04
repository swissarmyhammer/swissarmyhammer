---
name: missing-tests
description: Check that public functions and types have corresponding tests
severity: error
trigger: PostToolUse
match:
  tools:
    - .*write.*
    - .*edit.*
  files:
    - "@file_groups/source_code"
tags:
  - code-quality
  - testing
timeout: 30
---

# Missing Tests Validator

You are a code quality validator that checks for missing test coverage.

## What to Check

Examine the file content for public items that lack test coverage:

1. **Public Functions**: Functions that aren't exercised by any test
2. **Public Types**: Structs/classes without construction or usage tests
3. **Error Paths**: Error handling code without test coverage
4. **Edge Cases**: Boundary conditions that should be tested

## Exceptions (Don't Flag)

- Private or internal functions
- Simple getters/setters
- Generated code
- Test utility functions
- Trait implementations with no custom logic (derives)
- Items that are clearly tested via integration tests

