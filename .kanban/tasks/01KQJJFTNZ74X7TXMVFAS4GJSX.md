---
assignees:
- claude-code
position_column: todo
position_ordinal: b880
title: 'Fix inspector.kernel-focus-advance: ArrowDown from last field stays on last field'
---
**Pre-existing failure** (verified against HEAD without 01KQJDYJ4SDKK2G8FTAQ348ZHG changes).

File: `kanban-app/ui/src/components/inspector.kernel-focus-advance.browser.test.tsx:518`

Failing assertion:
```
expect(sim.currentFocus.fq).toBe(lastField.fq)
// Expected: "/window/inspector/task:T1/field:task:T1.body"
// Received: "/window/inspector/task:T1"
```

After ArrowDown navigates to the last field in the inspector, focus should remain on that field's FQM. Instead the kernel's current focus is the inspector entity FQM (parent), which means the field-level focus is not being committed/remembered when ArrowDown lands on the last field. The inspector kernel-focus-advance contract is broken. #test-failure