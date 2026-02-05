---
name: test-integrity
description: Ensure tests are not inappropriately skipped, disabled, or mocked
version: 1.0.0
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
  - blocking
  - quality
severity: error
timeout: 30
---

# Test Integrity RuleSet

Validates that tests are properly maintained and not being circumvented.

This RuleSet checks for:
- Tests being skipped or disabled inappropriately
- Test bodies commented out
- Excessive mocking that defeats test purpose
- Incomplete test implementations

All rules in this RuleSet have error severity and will block changes that compromise test quality.
