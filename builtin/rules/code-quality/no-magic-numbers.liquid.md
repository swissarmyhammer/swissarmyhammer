---
title: No Magic Numbers
description: Detect unexplained numeric literals in code
category: code-quality
severity: error
tags: ["code-quality", "maintainability"]
denied_tools:
  - ".*"
---

This rule only applies to programming languages that use numeric literals.

This rule does not apply to data files like yaml or json.

Check {{ language }} code for magic numbers (unexplained numeric literals).

Flag numeric literals that should be named constants, except:
- 0, 1, -1 (common values)
- Loop indices and array bounds in simple cases
- Test assertions with expected values
- Mathematical constants if clearly understood in context

Look for:
- Hardcoded configuration values
- Buffer sizes or limits
- Timeout values
- Port numbers
- Status codes
- Array sizes
- Percentages or ratios

For each magic number found, report:
- The numeric value
- Line number
- Context where it's used
- Suggestion for a descriptive constant name

If no magic numbers are found, respond with "PASS".
