---
name: function-length
description: >-
  Flag functions longer than ~50 lines of actual code. Keep functions small and
  focused; long functions are hard to read, test, and reuse.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
severity: warn
---

# Function Length Validator

Re-homed from the monolithic code-quality set into a focused, one-concern
validator: over-long functions. It is an **in-file judgment** — it reads the
diff and needs no engine probe, so it declares none.
