---
name: missing-docs
description: >-
  Flag public functions, types, and constants that lack documentation comments,
  and complex public APIs that need usage examples.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
severity: info
---

# Missing Docs Validator

Re-homed from the monolithic code-quality set into a focused, one-concern
validator: missing documentation on public APIs. It is an **in-file judgment** —
it reads the diff and needs no engine probe, so it declares none.
