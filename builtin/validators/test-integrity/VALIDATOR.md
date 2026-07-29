---
name: test-integrity
description: >-
  Flag test cheating. Test cheating includes skipped tests, disabled tests,
  commented-out test bodies, too much mocking, trivial assertions, and
  swallowed failures. Also flag code that hard-codes values to make a test
  pass, for example "return 42". A confirmed integrity violation is a
  blocker.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
    - "@file_groups/test_files"
---
