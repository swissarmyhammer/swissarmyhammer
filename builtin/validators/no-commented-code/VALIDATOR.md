---
name: no-commented-code
description: >-
  Flag large blocks of commented-out code — disabled functions, classes, or
  consecutive commented code lines. Version control preserves history; dead code
  in comments only clutters and misleads.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
severity: info
---

# No Commented Code Validator

Re-homed from the monolithic code-quality set into a focused, one-concern
validator: commented-out code. It is an **in-file judgment** — it reads the diff
and needs no engine probe, so it declares none.
