---
title: Document
description: Autonomously document the code
tags:
  - auto
---

## States

```mermaid
stateDiagram-v2
    [*] --> start
    start --> document
    document --> review
    review --> [*]
```

## Actions

- start: log "Documenting"
- document: execute prompt "docs/project"
- review: run workflow "review_docs"
- done: log "Complete"

## Description

This workflow documents and reviews the documentation.
