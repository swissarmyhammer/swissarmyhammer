# AVP - Agent Validator Protocol

This directory contains validators for Claude Code hooks. Validators are markdown
files with YAML frontmatter that define validation rules.

## Directory Structure

```
.avp/
├── README.md           # This file
├── log                 # Hook event log (auto-generated, gitignored)
└── validators/         # Your validator files
```

## Validator Format

Validators use markdown with YAML frontmatter:

```markdown
---
name: validator-name
description: What this validator does
severity: warn          # info, warn, or error (error = blocking)
trigger: PostToolUse    # Hook event that triggers this validator
match:                  # Optional: filter which events trigger this
  tools: [Write, Edit]  # Tool names (regex patterns)
  files: ["*.ts"]       # File globs
---

# Instructions for the validator

Describe what the validator should check and how it should respond.
```

## Triggers

- `PreToolUse` - Before a tool runs (can block)
- `PostToolUse` - After a tool succeeds
- `PostToolUseFailure` - After a tool fails
- `Stop` - When Claude finishes responding
- `SessionStart` - When a session begins
- `SessionEnd` - When a session ends
- `UserPromptSubmit` - When user submits a prompt

## Severity Levels

- `info` - Informational, logged but never blocks
- `warn` - Warning, logged but doesn't block (default)
- `error` - Error, blocks the action if validation fails

## More Information

See the AVP documentation for examples.
