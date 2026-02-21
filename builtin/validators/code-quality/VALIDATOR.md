---
name: code-quality
description: Code quality and maintainability checks for best practices
metadata:
  version: "{{version}}"
trigger: PostToolUse
match:
  tools:
    - .*write.*
    - .*edit.*
  files:
    - "@file_groups/source_code"
tags:
  - code-quality
  - maintainability
  - best-practices
severity: error
timeout: 30
---

# Code Quality RuleSet

Code quality validations that check for maintainability, readability, and best practices.

This RuleSet evaluates code for quality issues including:
- Code duplication
- Cognitive complexity
- Function length
- Missing documentation
- Missing tests
- Naming consistency
- Commented code
- Hardcoded values
- Log truncation
- Magic numbers
- String equality patterns

All rules in this RuleSet have error severity by default, though individual rules may override this.
