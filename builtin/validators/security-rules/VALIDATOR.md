---
name: security-rules
description: Critical security validations for code safety
version: "{{version}}"
trigger: PostToolUse
match:
  tools:
    - .*write.*
    - .*edit.*
  files:
    - "@file_groups/source_code"
tags:
  - security
  - blocking
severity: error
timeout: 30
---

# Security Rules RuleSet

Critical security validations that check for common vulnerabilities in code.

This RuleSet evaluates code for security issues including:
- Hardcoded secrets and credentials
- Input validation vulnerabilities (SQL injection, XSS, command injection)

All rules in this RuleSet have error severity and will block operations if violations are found.
