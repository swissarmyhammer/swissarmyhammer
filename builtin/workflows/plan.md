---
title: Planning Workflow
description: Turn specifications into multiple step plans
tags:
  - auto
parameters:
  - name: plan_filename
    description: Path to the specification file to process
    required: false
    type: string
    pattern: '^.*\.md$'
    
parameter_groups:
  - name: input
    description: Specification input configuration
    parameters: [plan_filename]
---

# Planning Workflow

This workflow processes specification files and generates detailed implementation plans.

## Usage

Provide the path to your specification file:

```bash
# CLI switch
sah flow run plan --plan-filename "./specification/my-feature.md"

# Interactive mode
sah flow run plan --interactive

# Legacy --set support (during transition)
sah flow run plan --set plan_filename="./spec/feature.md"

# Scan ./specification directory (when no parameter provided)
sah flow run plan
```

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

This workflow processes specification files and generates detailed implementation plans:

1. **Start State**: Logs the planning initiation with optional filename context
2. **Plan State**: Executes the planning prompt with structured parameters
   - `plan_filename` - Path to specific specification file (optional string with .md pattern validation)
   - When provided, plans the specific file
   - When omitted, scans the ./specification directory (legacy behavior)
3. **Done State**: Logs completion and directs user to generated issues

The structured parameter provides type safety with pattern validation for markdown files, improved CLI experience with parameter switches, and maintains backward compatibility with existing usage patterns.
