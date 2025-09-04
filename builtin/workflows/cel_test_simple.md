---
title: Simple CEL Test
description: Test CEL expressions with actual result data
tags:
  - test
---

## States

```mermaid
stateDiagram-v2
    [*] --> setup
    setup --> check
    check --> success: result.contains("PASS")
    check --> fail: default
    success --> [*]
    fail --> [*]
```

## Actions

- setup: set result="PASS"
- check: log "checking result"  
- success: log "Found PASS in result!"
- fail: log "Did not find PASS in result"

## Description

This workflow tests if CEL expressions work with simple result matching.