---
name: test-integrity
description: >-
  Flag test cheating — skipped/disabled tests, commented-out test bodies,
  over-mocking, trivial assertions, swallowed failures — and implementations that
  hard-code values to make a test pass ("return 42"). A confirmed integrity
  violation is a blocker.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
    - "@file_groups/test_files"
---
