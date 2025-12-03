---
title: Debug Result
description: Debug what's in the result variable
---

## States

```mermaid
stateDiagram-v2
    [*] --> check
    check --> done
    done --> [*]
```

## Actions

- check: execute prompt "are_tests_passing"
- done: log "Done"

