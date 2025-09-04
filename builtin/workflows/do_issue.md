---
title: Do Issue
description: Autonomously work through the current open issue.
tags:
  - auto
---

## States

```mermaid
stateDiagram-v2
    [*] --> start
    start --> code
    code --> review
    review --> code_review
    code_review --> test
    test --> complete
    complete --> commit
    commit --> merge
    merge --> [*]
```

## Actions

- start: log "Working an issue"
- code: execute prompt "issue/code"
- review: execute prompt "issue/review"
- code_review: execute prompt "issue/code_review"
- test: run workflow "tdd"
- complete: execute prompt "issue/complete"
- commit: execute prompt "commit"
- merge: execute prompt "issue/merge"

## Description

This workflow works an issue until it is completely resolved, tested, and reviewed.
