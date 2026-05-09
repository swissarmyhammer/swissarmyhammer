---
assignees:
- claude-code
position_column: todo
position_ordinal: b780
title: 'Fix entity-inspector: Escape from pill drills back to field zone'
---
**Pre-existing failure** (verified against HEAD without 01KQJDYJ4SDKK2G8FTAQ348ZHG changes).

File: `kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx:690`

Failing assertion:
```
expect(target).toBeTruthy()
// Expected: a ui.setFocus dispatch with scope_chain[0] === field zone moniker
// Received: undefined
```

Pressing Escape from a focused pill should drill the focus back up to the parent field zone moniker via `ui.setFocus`. The dispatch is not observed. Likely the same machinery issue as the field-enter-drill test above — the pill drill-in/out path is not wired. #test-failure