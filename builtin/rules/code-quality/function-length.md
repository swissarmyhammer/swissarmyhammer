---
title: Function Length Limit
description: Functions should be less than 50 lines
category: code-quality
severity: warning
tags: ["code-quality", "maintainability"]
---

Check {{ language }} code for functions longer than 50 lines.

Count actual code lines (excluding comments and blank lines).

Report any functions over 50 lines with:
- Function name
- Current line count
- Suggestion to break into smaller functions

If this file doesn't define functions, respond with "PASS".
