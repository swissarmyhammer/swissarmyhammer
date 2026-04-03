---
assignees:
- claude-code
depends_on:
- 01KN9KZH05YT847ZX8N3ZQT15Q
position_column: todo
position_ordinal: '9680'
title: Migrate views-context.tsx to useDispatchCommand
---
## What\nReplace `backendDispatch` (1 call: ui.view.set) with `useDispatchCommand` in `kanban-app/ui/src/lib/views-context.tsx`.\n\n## Acceptance Criteria\n- [ ] No imports of `backendDispatch` or `dispatchCommand`\n- [ ] Scope chain automatic from context\n\n## Tests\n- [ ] `cd kanban-app/ui && pnpm test` — all unit tests pass