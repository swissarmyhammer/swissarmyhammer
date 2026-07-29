---
name: no-commented-code
description: >-
  Flag large blocks of commented-out code, such as disabled functions,
  disabled classes, or many consecutive commented code lines. Version control
  preserves history. Dead code in comments only clutters the code and misleads
  readers.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
---
