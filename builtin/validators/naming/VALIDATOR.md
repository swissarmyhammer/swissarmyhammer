---
name: naming
description: >-
  Flag names that break the project's established conventions — variables,
  functions, types, modules, and constants that deviate from the prevailing
  casing and patterns of the surrounding codebase.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
severity: warn
---

# Naming Validator

Re-homed from the monolithic code-quality set into a focused, one-concern
validator: naming consistency. It is an **in-file judgment** — it reads the diff
and needs no engine probe, so it declares none.
