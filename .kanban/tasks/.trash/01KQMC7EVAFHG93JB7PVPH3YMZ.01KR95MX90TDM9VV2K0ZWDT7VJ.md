---
assignees: []
position_column: todo
position_ordinal: c280
title: 'flaky test: filter-editor.delete-scenario sporadically fails under full-suite load'
---
During a /test run on the `kanban` branch I observed a single flaky failure in `kanban-app/ui/src/components/filter-editor.delete-scenario.test.tsx`:

  expect(view.state.doc.toString()).toBe("");
  // Received: "#BLOCK"

The test types "#BLOCKED" into the CM6-backed filter editor, fires 8 backspaces, then asserts the doc is empty. It uses `await new Promise((r) => setTimeout(r, 500))` to wait for the autosave debounce. Under the full-suite load (1880+ tests, 60s wall clock), the 500ms timeout was insufficient and the doc still read "#BLOCK" (only 2 backspaces had been processed) — but in isolation the file passes 7/7 cleanly.

Reproduce:
- Run-to-fail: `cd kanban-app/ui && npm test` (full suite).
- Pass clean: `cd kanban-app/ui && npx vitest run src/components/filter-editor.delete-scenario.test.tsx`.

Fix direction: replace the `setTimeout(500)` debounce-wait with a `waitFor()` poll on the observable end state (e.g. `expect(lastFilter()).toBe("")` or `expect(view.state.doc.toString()).toBe("")`) so the test rides cooperative scheduling instead of wall-clock budget.

Filed during a /test run on commit 35a106634.