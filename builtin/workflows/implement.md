---
title: Implement
description: Autonomously run until all issues are resolved
tags:
  - auto
---

## States

```mermaid
stateDiagram-v2
    [*] --> start
    start --> are_issues_complete
    are_issues_complete --> loop
    loop --> done: result.content.matches("(?i)YES")
    loop --> work: default
    work --> are_issues_complete
    done --> [*]
```

## Actions

- start: log "Implementing issues"
- are_issues_complete: execute prompt "are_issues_complete"
- work: run workflow "do_issue"
- done: log "Complete"

## Description

This workflow works on tests until they all pass.
