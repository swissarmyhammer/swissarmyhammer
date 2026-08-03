---
name: code-hygiene
description: >-
  Flag hygiene defects in changed source code — commented-out code, overlong
  or overly complex functions, missing documentation on public APIs,
  hardcoded values that should be data, and dead code with no inbound
  callers.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
probes:
  - callers
  - complexity
---
