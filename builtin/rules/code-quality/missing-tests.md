---
title: Missing Tests
description: Check that public functions and types have corresponding tests
category: code-quality
severity: warning
tags: ["code-quality", "testing"]
---

Check {{ language }} code for public functions, methods, and types that lack corresponding test coverage.

Look for:
- Public functions without any test functions
- Public structs/classes without test coverage
- Public APIs that are not exercised by tests

Do not flag:
- Private/internal functions
- Simple getters/setters
- Generated code
- Test utility functions

{% include "_partials/report-format" %}

Report any untested public items with:
- Item name and signature
- File path and line number
- Suggestion for test coverage approach

{% include "_partials/pass-response" %}
