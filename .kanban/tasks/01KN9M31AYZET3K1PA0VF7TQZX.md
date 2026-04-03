---
assignees:
- claude-code
depends_on:
- 01KN9KZH05YT847ZX8N3ZQT15Q
position_column: todo
position_ordinal: 9a80
title: Migrate filter-editor.tsx to useDispatchCommand
---
## What\nReplace `backendDispatch` (3 calls: perspective.filter, perspective.clearFilter x2) with `useDispatchCommand` in `kanban-app/ui/src/components/filter-editor.tsx`. Update `filter-editor.test.tsx` if needed.\n\n## Acceptance Criteria\n- [ ] No imports of `backendDispatch` or `dispatchCommand`\n- [ ] Scope chain automatic from context\n\n## Tests\n- [ ] `cd kanban-app/ui && pnpm test` — all unit tests pass