# Builtin Rules

This directory contains built-in rules that are embedded in the SwissArmyHammer binary at build time.

## Directory Structure

```
builtin/rules/
├── security/           # Security-related rules
├── code-quality/       # Code quality and best practices rules
└── _partials/          # Shared template fragments
```

## Rule File Format

Rules are defined as markdown files with YAML frontmatter:

```markdown
---
title: No Hardcoded Secrets
description: Detects hardcoded API keys, passwords, and tokens in code
category: security
severity: error
tags: ["security", "secrets", "credentials"]
---

Check the following {{ language }} code for hardcoded secrets.

Look for:
- API keys (e.g., API_KEY = "sk_live_...")
- Passwords in plain text
- Auth tokens
- Private keys

If this file type doesn't contain code (e.g., markdown, config files), respond with "PASS".

Report any findings with line numbers and suggestions for {{ target_path }}.
```

## Available Variables

Rule templates have access to these context variables:
- `{{target_content}}` - The file content being checked
- `{{target_path}}` - Path to the file being checked
- `{{language}}` - Detected programming language

## Severity Levels

- `error` - Must be fixed (blocks checks)
- `warning` - Should be reviewed
- `info` - Informational findings
- `hint` - Suggestions for improvement

## Categories

- `security` - Security vulnerabilities and risks
- `code-quality` - Code maintainability and best practices
- `documentation` - Documentation completeness and accuracy
- `performance` - Performance issues and optimizations

## Adding New Rules

1. Create a `.md` file in the appropriate category directory
2. Add YAML frontmatter with required fields (title, description, severity)
3. Write the rule template content
4. Rules are automatically embedded during build via `build.rs`

## Partial Templates

Shared template fragments can be placed in `_partials/` and included in rules using Liquid's `{% render %}` tag (future feature).
