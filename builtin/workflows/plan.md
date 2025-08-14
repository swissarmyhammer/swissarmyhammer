---
title: Plan
description: Create a plan from a specification
tags:
  - auto
---

## Parameters

- plan_filename: The path to the specific plan file to process (optional, defaults to scanning ./specification directory)

## States

```mermaid
stateDiagram-v2
    [*] --> start
    start --> plan
    plan --> done
    done --> [*]
```

## Actions

- start: log "Making the plan{% if plan_filename %} for {{ plan_filename }}{% endif %}"
- plan: execute prompt "plan"{% if plan_filename %} with plan_filename="{{ plan_filename }}"{% endif %}
- done: log "Plan ready, look in ./issues"

## Description

This workflow creates a step-by-step plan from specification files.
When plan_filename is provided, plans the specific file.
When no parameter is given, scans the ./specification directory (legacy behavior).
