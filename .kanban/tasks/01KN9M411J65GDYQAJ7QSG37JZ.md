---
assignees:
- claude-code
depends_on:
- 01KN9KZH05YT847ZX8N3ZQT15Q
position_column: todo
position_ordinal: a180
title: Migrate nav-bar.tsx to useDispatchCommand
---
## What\nReplace `dispatchCommand` (1 call) and `useExecuteCommand` with `useDispatchCommand` in `kanban-app/ui/src/components/nav-bar.tsx`.\n\n## Acceptance Criteria\n- [ ] No imports of `backendDispatch`, `dispatchCommand`, or `useExecuteCommand`\n- [ ] Scope chain automatic from context\n\n## Tests\n- [ ] `cd kanban-app/ui && pnpm test` — all unit tests pass