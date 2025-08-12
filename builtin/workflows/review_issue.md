---
title: Review Issue
description: Autonomously code review an correct the current open issue.
tags:
  - auto
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

- start: log "Reviewing an issue"
- review: execute prompt "issue/review"
- correct: execute prompt "issue/code_review"
- commit: execute prompt "commit"

## Description

This workflow reviews the documentation and then implements those corrections.
