---
title: Review Documentation
description: Autonomously code review an correct the documentation.
---

## States

```mermaid
stateDiagram-v2
    [*] --> start
    start --> review
    review --> correct
    correct --> commit
    commit --> [*]
```

## Actions

- start: log "Reviewing documentation"
- review: execute prompt "docs/review"
- correct: execute prompt "docs/correct"
- commit: execute prompt "commit"

## Description

This workflow reviews all the documentation.
