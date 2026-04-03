---
assignees:
- claude-code
depends_on:
- 01KN9KZH05YT847ZX8N3ZQT15Q
position_column: todo
position_ordinal: a880
title: Migrate group-selector.tsx to useDispatchCommand
---
## What\nReplace `backendDispatch` (2 calls) with `useDispatchCommand` in `kanban-app/ui/src/components/group-selector.tsx`. Update `group-selector.test.tsx` if needed.\n\n## Acceptance Criteria\n- [ ] No imports of `backendDispatch` or `dispatchCommand`\n- [ ] Scope chain automatic from context\n\n## Tests\n- [ ] `cd kanban-app/ui && pnpm test` — all unit tests pass