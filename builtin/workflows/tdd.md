---
title: TDD
description: Autonomously run a TDD loop until all tests pass
tags:
  - auto
---

## States

```mermaid
stateDiagram-v2
    [*] --> start
    start --> are_tests_passing
    are_tests_passing --> loop
    loop --> done: result.content.contains("YES")
    loop --> test: default
    test --> are_tests_passing
    done --> [*]
```

## Actions

- start: log "Making tests pass"
- are_tests_passing: execute prompt "are_tests_passing"
- test: execute prompt "test"

## Description

This workflow works on tests until they all pass.
