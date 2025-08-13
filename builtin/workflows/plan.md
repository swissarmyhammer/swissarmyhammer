---
title: Plan
description: Create a plan from a specification
tags:
  - auto
---

## States

```mermaid
stateDiagram-v2
    [*] --> start
    start --> plan
    plan --> done
    done --> [*]
```

## Actions

- start: log "Making the plan for {{ plan_filename }}"
- plan: execute prompt "plan" with plan_filename="{{ plan_filename }}"
- done: log "Plan ready, look in ./issues"

## Description

This workflow creates a step-by-step plan from a specific specification file.
