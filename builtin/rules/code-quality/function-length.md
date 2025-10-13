---
title: Function Length Limit
description: Functions should be less than 50 lines
category: code-quality
severity: warning
tags: ["code-quality", "maintainability"]
---

Check {{ language }} code for functions longer than 50 lines.

Count actual code lines (excluding comments and blank lines).

{% include "_partials/report-format" %}

Report any functions over 50 lines with:
- Function name
- Current line count
- Suggestion to break into smaller functions

{% include "_partials/pass-response" %}
