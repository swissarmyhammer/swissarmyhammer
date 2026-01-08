---
title: Cognitive Complexity
description: Limit cognitive complexity of functions
category: code-quality
severity: error
tags: ["code-quality", "complexity"]
denied_tools:
  - ".*"
---

Analyze {{ language }} code for high cognitive complexity (nested ifs, loops, etc).

Flag functions with:
- Deeply nested conditions (> 3 levels)
- Many branches or decision points
- Complex boolean logic with multiple conditions
- Nested loops within conditionals

For each complex function, report:
- Function name
- Complexity indicators (nesting depth, branch count)
- Specific areas contributing to complexity

Suggest refactoring strategies such as:
- Extract nested logic into separate functions
- Use early returns to reduce nesting
- Simplify boolean expressions
- Replace complex conditionals with polymorphism or strategy pattern

If this file doesn't define functions, respond with "PASS".
