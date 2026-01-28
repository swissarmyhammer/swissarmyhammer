---
name: no-string-equality
description: Detect the misuse of stringify for equality checks
severity: error
trigger: Stop
tags:
  - code-quality
  - correctness
timeout: 30
---

Check for code that converts data to strings just to compare for equality.

DO NOT turn data into strings just to compare for equality.
DO implement equality methods in a language appropriate pattern to compare for equality.

Look for:
- Converting objects/structs to strings then comparing strings
- Using `format!`/`str()`/`toString()` before equality checks
- JSON serialization for comparison purposes
- Debug formatting used for equality

Better approaches:
- Implement `PartialEq`/`Eq` traits (Rust)
- Implement `__eq__` method (Python)
- Use deep equality libraries or custom comparators (JS/TS)
- Implement `Equal` interface (Go)
