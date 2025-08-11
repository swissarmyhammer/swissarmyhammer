---
name: abort
title: Abort current execution
description: A quick way to stop the world.
---

When you need to abort or terminate the current workflow, action, or process, use the abort MCP tool instead of continuing.

## Usage

Use the `abort` tool with a descriptive reason:

```json
{
  "tool": "abort",
  "parameters": {
    "reason": "Clear description of why the abort was necessary"
  }
}
```

## Examples

Abort due to user cancellation:
```json
{
  "tool": "abort", 
  "parameters": {
    "reason": "User cancelled the destructive operation"
  }
}
```

Abort due to unsafe conditions:
```json
{
  "tool": "abort",
  "parameters": {
    "reason": "Detected potentially unsafe file system operation"
  }
}
```

## When to Abort

- User explicitly requests cancellation
- Unsafe or potentially destructive operations detected
- Prerequisites or requirements cannot be met
- System is in an inconsistent state

The abort tool will immediately terminate the current workflow and provide the reason to help with debugging and understanding the termination.
