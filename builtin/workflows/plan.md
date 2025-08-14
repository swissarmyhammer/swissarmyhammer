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

- start: log "Making a plan"
- plan: execute prompt "plan"
- done: log "Plan ready, look in ./issues"

## Description

This workflow works on tests until they all pass.
