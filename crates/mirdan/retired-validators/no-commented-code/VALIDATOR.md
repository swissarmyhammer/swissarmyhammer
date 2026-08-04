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
---
