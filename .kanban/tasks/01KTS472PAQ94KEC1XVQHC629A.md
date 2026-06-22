---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvqs3k4wpf0ew8z761n30fhr
  text: |-
    Resolved as a subset of 55vgytq (01KTVRE0T89FGBH0XYF55VGYTQ). The 2 tests this card tracks — "context menu dispatches task.doThisNext through the backend, not task.move" and "context menu scope chain contains the task moniker" — are exactly the 2 genuinely-failing column-view context-menu tests fixed there.

    Root cause: the test harness mocked `command_tool_call` as `"ok"` instead of the real host-driven `ListCommandResult` shape `{ ok: true, commands: [...] }`, so the menu never populated (hence "expected undefined to be truthy"). Fixed by adding `serveContextMenuCommandTool` to column-view.test.tsx (commit 4bd0ff3b1).

    Verified now: `npx vitest run --project browser src/components/column-view.test.tsx` → Test Files 1 passed, Tests 15 passed (15). No additional change needed. Closing.
  timestamp: 2026-06-22T13:42:50.268793+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffd880
title: 'Bug: column-view.test.tsx "Do This Next" context-menu tests fail (pre-existing)'
---
Two tests in `apps/kanban-app/ui/src/components/column-view.test.tsx` fail deterministically on the `plugin` branch (observed while working 01KTCRW1RT0QD025QANC7GNYWX, which touched nothing they import):

- `ColumnView — Do This Next command > context menu dispatches task.doThisNext through the backend, not task.move`
- `ColumnView — Do This Next command > context menu scope chain contains the task moniker`

Both fail with `AssertionError: expected undefined to be truthy` and the DOM dump shows the context menu apparently never opened in headless Chromium. Reproduce: `npx vitest run src/components/column-view.test.tsx` in `apps/kanban-app/ui` (13 passed, 2 failed). The files are unmodified relative to HEAD, so this is a pre-existing break — likely the context-menu open path (right-click simulation) or the mock command list drifted. #bug