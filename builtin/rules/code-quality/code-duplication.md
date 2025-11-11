---
title: Code Duplication
description: Detect duplicate code blocks and similar logic patterns
category: code-quality
severity: warning
tags: ["code-quality", "maintainability", "refactoring"]
---

Check {{ language }} code for duplicated code blocks and similar logic patterns.

Look for:
- Identical or near-identical code blocks (>5 lines)
- Similar algorithms or business logic that could be abstracted
- Repeated constant values or configuration
- Duplicate test setup or assertion patterns

Suggest refactoring through:
- Extracting shared functions or methods
- Creating utility modules or helpers
- Defining shared constants or configuration
- Using parametric patterns or generics

Do not flag:
- Boilerplate required by the language or framework
- Code that is similar but serves different domains
- Small snippets (<5 lines) that are common patterns

{% include "_partials/report-format" %}

Report duplications with:
- Location of duplicate blocks (file and line numbers)
- Similarity description
- Specific refactoring suggestion

{% include "_partials/pass-response" %}
