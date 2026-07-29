---
name: complexity
description: >-
  Flag functions with high cognitive complexity. Look for deep nesting, many
  branches, and complex boolean logic. Also look for nested loops and long
  conditional chains. These functions are hard to read and hard to reason about.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
---
