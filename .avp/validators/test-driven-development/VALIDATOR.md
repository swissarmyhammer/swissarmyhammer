---
name: test-driven-development
description: Enforce test-driven development — write tests first, keep tests in sync with code
version: "0.1.0"
trigger: PostToolUse
match:
  tools:
    - .*write.*
    - .*edit.*
  files:
    - "@file_groups/source_code"
    - "@file_groups/test_files"
tags:
  - testing
  - tdd
  - quality
severity: error
timeout: 30
---

# Test-Driven Development RuleSet

Enforces the discipline of test-driven development: write a failing test first, then write
the minimum code to make it pass, then refactor. Tests are not an afterthought — they are
the specification.

This RuleSet validates that:
- New production code is accompanied by tests
- Behavior changes are reflected in updated tests
- Tests describe intent, not implementation details

Rules are automatically discovered from the `rules/` directory.
