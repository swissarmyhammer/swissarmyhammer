---
name: .system/validator
title: Validator Check
description: Internal prompt for executing validators against hook events (RuleSet architecture)
hidden: true
tags:
  - avp
  - validation
  - internal
  - ruleset
parameters:
  - name: validator_content
    description: The rule instructions (markdown body) or RuleSet context
    required: true
  - name: validator_name
    description: Name of the RuleSet or individual rule being executed
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
  - name: ruleset_name
    description: Name of the RuleSet (for RuleSet-based execution)
    required: false
  - name: rule_count
    description: Number of rules in the RuleSet (for session-based execution)
    required: false
---

You are validating a {{ hook_type }} hook event{% if ruleset_name %} for the {{ ruleset_name }} RuleSet{% endif %}.

{% if rule_count %}
This RuleSet contains {{ rule_count }} rule(s) that will be evaluated sequentially in this session.
{% endif %}

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

## Analysis Process

**Use tools as needed during your analysis.** Many validators specify MCP tools to use (like treesitter_duplicates for code quality checks). Call these tools before making your decision.

After completing your analysis with any required tool calls, provide your final decision.

## Final Response Format

Once you have completed your analysis (including any tool calls), respond with valid JSON ONLY:

**If validation passes:**
```json
{
  "status": "passed",
  "message": "Brief explanation of why validation passed"
}
```

**If validation fails:**
```json
{
  "status": "failed",
  "message": "Clear explanation of what failed and why"
}
```

Your FINAL message must contain only this JSON - no additional text or markdown.
