---
title: Do Issue
description: Autonomously work through the next open issue.
tags:
  - auto
---

## States

```mermaid
stateDiagram-v2
    [*] --> start
    start --> code
    code --> review
    review --> test
    test --> complete
    complete --> commit
    commit --> [*]
```

## Actions

- start: log "Working an issue"
- code: execute prompt "code/issue"
- review: run workflow "review"
- test: run workflow "test"
- complete: execute prompt "issue/complete"
- commit: execute prompt "commit"

## Description

This workflow works an issue until it is completely resolved, tested, and reviewed.
