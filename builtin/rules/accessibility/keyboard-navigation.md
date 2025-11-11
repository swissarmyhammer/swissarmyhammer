---
title: Keyboard Navigation
description: Check that all interactive elements are keyboard accessible
category: accessibility
severity: warning
tags: ["accessibility", "keyboard", "a11y"]
---

Check {{ language }} code for keyboard navigation issues.

Look for:
- Interactive elements without keyboard support
- Missing or invisible focus indicators
- Improper tab order
- Keyboard traps
- Missing skip links for navigation
- Custom controls without keyboard handlers

Common issues:
- `<div>` or `<span>` used as buttons without tabindex
- `:focus` styles removed or invisible
- Positive tabindex values creating wrong order
- Modals that trap focus incorrectly
- Navigation without skip-to-content link
- Custom widgets missing keyboard event handlers

Do not flag:
- Proper semantic buttons and links
- Visible focus indicators
- Natural tab order (no tabindex or tabindex="0")
- Properly implemented focus management
- Skip links for navigation
- ARIA keyboard patterns implemented correctly

{% include "_partials/report-format" %}

Report issues with:
- Type of keyboard accessibility issue
- Location and affected element
- Suggested implementation for keyboard support

{% include "_partials/pass-response" %}
