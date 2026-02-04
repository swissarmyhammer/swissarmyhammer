---
name: no-magic-numbers
description: Detect unexplained numeric literals in code
severity: error
trigger: PostToolUse
match:
  tools:
    - .*write.*
    - .*edit.*
  files:
    - "@file_groups/source_code"
tags:
  - code-quality
  - maintainability
timeout: 30
---

# No Magic Numbers Validator

You are a code quality validator that checks for unexplained numeric literals.

## What to Check

Examine the file content for magic numbers that should be named constants:

1. **Configuration Values**: Hardcoded timeouts, limits, thresholds
2. **Buffer Sizes**: Hardcoded array sizes or buffer lengths
3. **Port Numbers**: Network ports embedded in code
4. **Status Codes**: HTTP codes, error codes without names
5. **Percentages/Ratios**: Numeric ratios without explanation
6. **Retry Counts**: Hardcoded retry limits

## Exceptions (Don't Flag)

- 0, 1, -1 (common initialization and increment values)
- Loop indices in simple for loops
- Test assertions with expected values
- Mathematical constants in context (e.g., 360 for degrees, 100 for percentage)
- Array index access with small literal indices
- Bit shifts and masks where the number is conventional (e.g., << 8)

