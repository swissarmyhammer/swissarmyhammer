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
    correct --> test
    test --> commit
    commit --> [*]
```

## Actions

- start: log "Reviewing an issue"
- review: execute prompt "issue/review"
- correct: execute prompt "issue/code_review"
- test: log "Would run TDD workflow"
- commit: execute prompt "commit"

## Description

This workflow reviews a working branch and then implements that review.
