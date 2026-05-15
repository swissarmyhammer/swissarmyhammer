---
assignees:
- claude-code
position_column: todo
position_ordinal: b480
title: 'Fix board-view: Enter on focused column drills into first card carries scope_chain'
---
**Pre-existing failure** (verified against HEAD without 01KQJDYJ4SDKK2G8FTAQ348ZHG changes).

File: `kanban-app/ui/src/components/board-view.enter-drill-in.browser.test.tsx:637`

Failing assertion:
```
expect(dispatchArgs?.scope_chain?.[0]).toBe("task:t1")
// Expected: "task:t1"
// Received: undefined
```

The Enter-key drill-in handler dispatches `ui.setFocus` but the dispatch's `scope_chain` is undefined where the test expects the resolved child moniker (e.g. `task:t1`) at the head. The drill-in machinery must populate `scope_chain` with the child moniker so the kernel knows where focus moved to.

Same root cause as the sibling test below. #test-failure