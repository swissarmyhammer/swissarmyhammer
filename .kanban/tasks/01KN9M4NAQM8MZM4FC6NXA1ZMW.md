---
assignees:
- claude-code
depends_on:
- 01KN9KZH05YT847ZX8N3ZQT15Q
position_column: todo
position_ordinal: a580
title: Migrate data-table.tsx to useDispatchCommand
---
## What\nReplace `backendDispatch`/`dispatchCommand` (2 calls) with `useDispatchCommand` in `kanban-app/ui/src/components/data-table.tsx`.\n\n## Acceptance Criteria\n- [ ] No imports of `backendDispatch` or `dispatchCommand`\n- [ ] Scope chain automatic from context\n\n## Tests\n- [ ] `cd kanban-app/ui && pnpm test` — all unit tests pass