---
name: command-safety
description: Block dangerous shell commands that could cause data loss or system damage
metadata:
  version: "{{version}}"
trigger: PreToolUse
match:
  tools:
    - Bash
    - .*shell.*
tags:
  - security
  - blocking
  - bash
severity: error
timeout: 30
---

# Command Safety RuleSet

Security validations that check shell commands before execution to prevent dangerous operations.

This RuleSet evaluates shell commands for potentially destructive patterns including:
- File system destruction
- System damage
- Network attacks
- Credential exposure
- Git safety violations
- Interactive editors that may hang
