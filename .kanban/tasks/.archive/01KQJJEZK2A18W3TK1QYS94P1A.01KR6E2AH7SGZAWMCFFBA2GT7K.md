---
assignees:
- claude-code
position_column: todo
position_ordinal: b680
title: 'Fix entity-inspector: Enter on pill field drills into first pill'
---
**Pre-existing failure** (verified against HEAD without 01KQJDYJ4SDKK2G8FTAQ348ZHG changes).

File: `kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx:533`

Failing assertion:
```
expect(targetCall).toBeTruthy()
// Expected: true (i.e. a ui.setFocus dispatch with scope_chain[0] === first pill's moniker)
// Received: undefined
```

When Enter is pressed on a focused pill-field zone (e.g. tags, assignees), the drill-in path is supposed to dispatch `ui.setFocus` whose `scope_chain[0]` equals the first pill's moniker. No such dispatch is observed. Either the drill-in handler is not firing for pill fields, or it is firing but not setting `scope_chain`. #test-failure