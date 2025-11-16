---
title: Naming Consistency
description: Check that naming conventions match existing codebase patterns
category: code-quality
severity: error
tags: ["code-quality", "consistency", "style"]
---

Check {{ language }} code for naming inconsistencies compared to the existing codebase.

Look for:
- Variable names that don't match project conventions
- Function names that break established patterns
- Type names that don't follow project style
- Module or file names that deviate from standards

Check against:
- Existing similar functions in the codebase
- Project naming conventions document
- Language-specific style guides (e.g., Rust API guidelines)
- Common patterns in the same module or package

Do not flag:
- Names that match external library conventions
- Domain-specific terminology that's standard
- Acronyms or abbreviations that are well-known

{% include "_partials/report-format" %}

Report inconsistencies with:
- Current name and location
- Suggested name that matches project patterns
- Reference to similar existing code

{% include "_partials/pass-response" %}
