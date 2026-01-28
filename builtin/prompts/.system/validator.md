---
name: .system/validator
title: Validator Check
description: Internal prompt for executing validators against hook events
hidden: true
tags:
  - avp
  - validation
  - internal
parameters:
  - name: validator_content
    description: The validator instructions (markdown body)
    required: true
  - name: validator_name
    description: Name of the validator being executed
    required: true
  - name: hook_context
    description: The hook event context as JSON
    required: true
  - name: hook_type
    description: The type of hook event (PreToolUse, PostToolUse, etc.)
    required: true
  - name: changed_files
    description: List of files that changed during this turn (optional, typically for Stop hooks)
    required: false
---

You are validating a {{ hook_type }} hook event against the following validator:

---
{{ validator_content }}
---

## Hook Event Context

```json
{{ hook_context }}
```

{% if changed_files %}
## Files Changed This Turn

The following files were modified during this turn:
{% for file in changed_files %}
- {{ file }}
{% endfor %}

When evaluating code quality validators, focus your analysis on these changed files.
{% endif %}

Analyze this hook event against the validator instructions above.

## Response Format

If validation passes, respond with exactly this JSON:

```json
{
  "status": "passed",
  "message": "Brief explanation of why validation passed"
}
```

If validation fails, respond with exactly this JSON:

```json
{
  "status": "failed",
  "message": "Clear explanation of what failed and why"
}
```

Respond with valid JSON only - no additional text or markdown outside the JSON.
