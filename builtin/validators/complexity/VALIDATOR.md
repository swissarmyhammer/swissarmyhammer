---
name: complexity
description: >-
  Flag functions with high cognitive complexity — deep nesting, many branches,
  complex boolean logic, nested loops, long conditional chains — that are hard to
  read and reason about.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
severity: warn
---

# Complexity Validator

Re-homed from the monolithic code-quality set into a focused, one-concern
validator: cognitive complexity of functions. It is an **in-file judgment** —
it reads the diff and needs no engine probe, so it declares none.
