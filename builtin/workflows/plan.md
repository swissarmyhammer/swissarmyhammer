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
    start --> create_draft
    create_draft --> generate_rules
    generate_rules --> generate_todos
    generate_todos --> done
    done --> [*]
```

## Actions

- start: log "Making the plan {{ plan_filename }}"
- create_draft: execute prompt "create-draft-plan" with plan_filename="{{ plan_filename }}"
- generate_rules: execute prompt "generate-rules" with plan_filename="{{ plan_filename }}"
- generate_todos: execute prompt "generate-todos" with plan_filename="{{ plan_filename }}"
- done: log "Plan ready"
