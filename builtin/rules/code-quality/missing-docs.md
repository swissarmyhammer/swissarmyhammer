---
title: Missing Documentation
description: Check that public functions and types have documentation comments
category: code-quality
severity: info
tags: ["code-quality", "documentation"]
---

Check {{ language }} code for public functions, methods, structs, and types that lack documentation comments.

Look for:
- Public functions without doc comments
- Public structs/classes without doc comments
- Public enums without doc comments
- Complex public APIs without usage examples

Do not flag:
- Private/internal items
- Test functions
- Obvious implementations (e.g., Display, Debug derives)
- Generated code

{% include "_partials/report-format" %}

Report any undocumented public items with:
- Item name and type
- File path and line number
- Suggestion for what documentation should include

{% include "_partials/pass-response" %}
