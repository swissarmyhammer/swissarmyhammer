---
title: WCAG Compliance
description: Check for WCAG 2.1 Level AA compliance issues
category: accessibility
severity: warning
tags: ["accessibility", "wcag", "a11y"]
---

Check {{ language }} code for WCAG 2.1 Level AA compliance issues.

Look for:
- Images without alt text
- Insufficient color contrast ratios
- Missing form labels
- Inaccessible interactive elements
- Missing language attributes
- Time limits without user control

Common issues:
- `<img>` without alt attribute
- Text with contrast ratio below 4.5:1
- Form inputs without associated labels
- Custom buttons without proper roles
- HTML without lang attribute
- Auto-playing media without controls

Do not flag:
- Decorative images with empty alt text
- Sufficient contrast ratios (4.5:1 for normal text, 3:1 for large text)
- Properly labeled form elements
- Semantic HTML with correct roles
- Appropriate language attributes

{% include "_partials/report-format" %}

Report violations with:
- WCAG criterion violated (e.g., 1.1.1, 1.4.3)
- Location and element type
- Suggested fix for compliance

{% include "_partials/pass-response" %}
