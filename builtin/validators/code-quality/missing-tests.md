---
name: missing-tests
description: Check that public functions and types have corresponding tests
severity: error
trigger: Stop
tags:
  - code-quality
  - testing
timeout: 30
---

Check code for public functions, methods, and types that lack corresponding test coverage.

Look for:
- Public functions without any test functions
- Public structs/classes without test coverage
- Public APIs that are not exercised by tests

Do not flag:
- Private/internal functions
- Simple getters/setters
- Generated code
- Test utility functions
