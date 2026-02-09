---
name: session-lifecycle
description: Review session changes for consistency and completeness
version: "{{version}}"
trigger: Stop
tags:
  - session
  - review
severity: error
timeout: 30
---

# Session Lifecycle RuleSet

Validates overall session quality at the end of a coding session.

This RuleSet runs when the session ends (Stop trigger) and reviews:
- Session change consistency
- Task completion
- Code state at session end

Rules in this RuleSet have error severity but run at session end rather than blocking individual operations.
