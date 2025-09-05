---
title: Test Variables Workflow
description: Test workflow with template variables
version: 1.0.0
---

```mermaid
stateDiagram-v2
    [*] --> start
    start --> end
    end --> [*]
```

## Actions

- start: Log "Hello {{ user_name | default: 'World' }}!"  
- end: Log "Value: {{ test_value }}"