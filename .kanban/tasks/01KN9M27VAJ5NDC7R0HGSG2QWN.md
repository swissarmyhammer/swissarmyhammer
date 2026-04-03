---
assignees:
- claude-code
depends_on:
- 01KN9KZH05YT847ZX8N3ZQT15Q
position_column: todo
position_ordinal: '9580'
title: Migrate entity-focus-context.tsx to useDispatchCommand
---
## What\nReplace `backendDispatch` (2 calls: ui.setFocus) with `useDispatchCommand` in `kanban-app/ui/src/lib/entity-focus-context.tsx`. Update `entity-focus-context.test.tsx` if needed.\n\n## Acceptance Criteria\n- [ ] No imports of `backendDispatch` or `dispatchCommand`\n- [ ] Scope chain automatic from context\n\n## Tests\n- [ ] `cd kanban-app/ui && pnpm test` — all unit tests pass