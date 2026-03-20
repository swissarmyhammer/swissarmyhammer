---
assignees:
- claude-code
position_column: todo
position_ordinal: '8680'
title: '[warning] Inspector close/open has async round-trip delay'
---
## `kanban-app/ui/src/App.tsx:120-160`\n\nThe old `closeTopPanel`, `dismissTopPanel`, and `closeAll` were synchronous — instant visual feedback. The new versions fire an async `invoke('dispatch_command', ...)` and update the panel stack in `.then()`. This introduces a round-trip delay before the UI responds.\n\nFor `inspectEntity` this is less noticeable since the panel needs to fetch entity data anyway. For close operations, the user may perceive lag.\n\n## Fix options\n1. Optimistic update: set panel stack immediately (same as before), then reconcile with backend result. If they diverge, backend wins.\n2. Accept the latency — it's likely under 10ms for a local IPC call.\n\n## Subtasks\n- [ ] Measure actual latency of dispatch_command round-trip\n- [ ] If perceivable, add optimistic updates for close operations\n- [ ] Verify fix works"