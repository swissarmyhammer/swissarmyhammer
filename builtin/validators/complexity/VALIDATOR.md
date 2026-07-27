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
---
