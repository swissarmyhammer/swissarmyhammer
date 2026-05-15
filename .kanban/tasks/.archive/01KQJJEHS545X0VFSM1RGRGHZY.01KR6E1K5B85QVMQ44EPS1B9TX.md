---
assignees:
- claude-code
position_column: todo
position_ordinal: b580
title: 'Fix board-view: Enter on focused column with remembered focus drills into remembered card'
---
**Pre-existing failure** (verified against HEAD without 01KQJDYJ4SDKK2G8FTAQ348ZHG changes).

File: `kanban-app/ui/src/components/board-view.enter-drill-in.browser.test.tsx:708`

Failing assertion:
```
expect(dispatchArgs?.scope_chain?.[0]).toBe("task:t2")
// Expected: "task:t2"
// Received: undefined
```

The drill-in handler is supposed to follow the kernel-returned remembered moniker but the dispatched `ui.setFocus` payload has no `scope_chain` populated. Same root cause as the sibling drill-in test. #test-failure