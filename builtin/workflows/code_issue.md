---
title: Code Issue
description: Autonomously implement a solution to the current open issue.
tags:
  - auto
---

## States

```mermaid
stateDiagram-v2
    [*] --> start
    start --> issue
    issue --> test
    test --> commit
    commit --> [*]
```

## Actions

- start: log "Coding an issue"
- issue: execute prompt "issue/code"
- test: run workflow "tdd"
- commit: execute prompt "commit"

## Description

This workflow works an issue until it is coded and tested.
