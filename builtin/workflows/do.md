---
title: Do Todos
description: Autonomously work through all pending todo items
mode: implementer
tags:
  - auto
---

## States

```mermaid
stateDiagram-v2
    [*] --> start
    start --> do_todo
    do_todo --> loop
    loop --> done: are_todos_done
    loop --> do_todo: default
    done --> [*]
```

## Actions

- start: log "Working through todos"
- do_todo: execute prompt "do_todo"
- done: log "All todos complete!"

## Description

This workflow iterates through all pending todo items, completing them one by one until no todos remain.
