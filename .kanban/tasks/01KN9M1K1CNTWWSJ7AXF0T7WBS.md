---
assignees:
- claude-code
depends_on:
- 01KN9KZH05YT847ZX8N3ZQT15Q
position_column: done
position_ordinal: ffffffffffffffffffffffdc80
title: Migrate perspective-context.tsx to useDispatchCommand
---
## What\nReplace `backendDispatch` (3 calls) with `useDispatchCommand` in `kanban-app/ui/src/lib/perspective-context.tsx`.\n\n## Acceptance Criteria\n- [ ] No imports of `backendDispatch` or `dispatchCommand`\n- [ ] Scope chain and boardPath are automatic from context\n\n## Tests\n- [ ] `cd kanban-app/ui && pnpm test` — all unit tests pass