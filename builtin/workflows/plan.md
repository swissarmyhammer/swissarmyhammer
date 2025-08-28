---
title: Planning Workflow
description: Turn specifications into multiple step plans
tags:
  - auto
parameters:
  - name: plan_filename
    description: Path to the specification file to process
    required: true
    type: string
    default: specification/index.md
    pattern: '^.*\.md$'

parameter_groups:
  - name: input
    description: Specification input configuration
    parameters: [plan_filename]
---

# Planning Workflow

This workflow processes specification files and generates detailed implementation plans.

## States

```mermaid
stateDiagram-v2
    [*] --> start
    start --> plan
    plan --> done
    done --> [*]
```

## Actions

- start: log "Making the plan {{ plan_filename }}"
- plan: execute prompt "plan" with plan_filename="{{ plan_filename }}"
- done: log "Plan ready"
