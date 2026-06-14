---
assignees:
- claude-code
position_column: todo
position_ordinal: e680
title: 'Bug: column-view.test.tsx "Do This Next" context-menu tests fail (pre-existing)'
---
Two tests in `apps/kanban-app/ui/src/components/column-view.test.tsx` fail deterministically on the `plugin` branch (observed while working 01KTCRW1RT0QD025QANC7GNYWX, which touched nothing they import):

- `ColumnView — Do This Next command > context menu dispatches task.doThisNext through the backend, not task.move`
- `ColumnView — Do This Next command > context menu scope chain contains the task moniker`

Both fail with `AssertionError: expected undefined to be truthy` and the DOM dump shows the context menu apparently never opened in headless Chromium. Reproduce: `npx vitest run src/components/column-view.test.tsx` in `apps/kanban-app/ui` (13 passed, 2 failed). The files are unmodified relative to HEAD, so this is a pre-existing break — likely the context-menu open path (right-click simulation) or the mock command list drifted. #bug