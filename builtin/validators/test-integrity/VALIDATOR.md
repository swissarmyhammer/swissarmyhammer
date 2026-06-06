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
severity: error
---

# Test Integrity Validator

Re-homed into a focused review-time validator covering two one-concern rules:

- `no-test-cheating` — tests being skipped, disabled, commented out, over-mocked,
  or otherwise circumvented (migrated from the old `test-integrity` set).
- `no-hard-code` — implementations that hard-code a value to make a test appear
  to pass, the classic "return 42" bug (moved here from the deleted code-quality
  set).

Both are **in-file judgments** — they read the diff and need no engine probe, so
this validator declares none.

This concern used to fire in real time on the Stop hook, blocking the agent from
finishing while a test was circumvented. It is now a **review-time** validator: a
confirmed integrity violation stops work via the review-column gate (a blocker),
not a pre-stop block. The bar is unchanged — every test should be real and should
run when we run tests.
