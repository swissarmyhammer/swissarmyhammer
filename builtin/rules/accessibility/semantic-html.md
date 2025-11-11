---
title: Semantic HTML
description: Check for proper use of semantic HTML elements and ARIA attributes
category: accessibility
severity: info
tags: ["accessibility", "html", "aria", "semantics"]
---

Check {{ language }} code for semantic HTML and ARIA usage.

Look for:
- Non-semantic divs where semantic elements should be used
- Missing or incorrect ARIA roles
- Invalid ARIA attributes
- Redundant ARIA on semantic HTML
- Missing landmark regions
- Improper heading hierarchy

Common issues:
- `<div>` instead of `<button>`, `<nav>`, `<main>`, etc.
- ARIA roles on elements that already have implicit roles
- Invalid ARIA attribute values
- Multiple `<h1>` or skipped heading levels
- Missing `role="main"` or `<main>` element
- Improper use of ARIA states and properties

Do not flag:
- Proper semantic HTML elements
- ARIA used to enhance, not replace, semantics
- Valid ARIA attributes with correct values
- Logical heading hierarchy (h1, h2, h3...)
- Proper landmark regions
- Progressive enhancement with ARIA

{% include "_partials/report-format" %}

Report issues with:
- Semantic or ARIA issue type
- Current markup and location
- Suggested semantic HTML or proper ARIA

{% include "_partials/pass-response" %}
