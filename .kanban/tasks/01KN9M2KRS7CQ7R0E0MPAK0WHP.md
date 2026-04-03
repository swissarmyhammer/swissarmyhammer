---
assignees:
- claude-code
depends_on:
- 01KN9KZH05YT847ZX8N3ZQT15Q
position_column: todo
position_ordinal: '9880'
title: Migrate App.tsx to useDispatchCommand
---
## What\nReplace `backendDispatch` (10 calls) and `dispatchCommand` (1 call) with `useDispatchCommand` in `kanban-app/ui/src/App.tsx`. Commands: ui.inspect, ui.inspector.close, ui.inspector.close_all, file.switchBoard (5x), view.switch.\n\n## Acceptance Criteria\n- [ ] No imports of `backendDispatch` or `dispatchCommand`\n- [ ] Scope chain and boardPath automatic from context\n\n## Tests\n- [ ] `cd kanban-app/ui && pnpm test` — all unit tests pass