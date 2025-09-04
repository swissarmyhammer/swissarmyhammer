---
title: CEL Test
description: Test CEL expression evaluation
tags:
  - test
---

## States

```mermaid
stateDiagram-v2
    [*] --> start
    start --> check_result
    check_result --> success: result.contains("YES")
    check_result --> failure: default
    success --> [*]
    failure --> [*]
```

## Actions

- start: log "Testing CEL evaluation"
- check_result: log "Result is YES"
- success: log "CEL worked - found YES"
- failure: log "CEL failed - did not find YES"

## Description

This workflow tests if CEL expressions work properly.