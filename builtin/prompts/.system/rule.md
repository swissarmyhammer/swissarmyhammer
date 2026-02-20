---
name: .system/rule
title: Rule Evaluation
description: Internal prompt for evaluating individual rules within a RuleSet session
hidden: true
tags:
  - avp
  - validation
  - internal
  - ruleset
parameters:
  - name: rule_name
    description: Name of the rule being evaluated
    required: true
  - name: rule_description
    description: Description of what this rule checks
    required: true
  - name: rule_severity
    description: Severity level (error, warn, info)
    required: true
  - name: rule_body
    description: The full rule instructions (markdown body)
    required: true
  - name: hook_context
    description: The hook event context (JSON) being validated
    required: false
---

# Rule: {{ rule_name }}

**Description**: {{ rule_description }}
**Severity**: {{ rule_severity }}

{% if hook_context %}
## Hook Context

```json
{{ hook_context }}
```
{% endif %}

{{ rule_body }}

## Required Response Format

Respond with valid JSON ONLY:

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

Your response must contain only this JSON - no additional text or markdown.
