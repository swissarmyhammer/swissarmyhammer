---
assignees:
- claude-code
depends_on:
- 01KN9KZH05YT847ZX8N3ZQT15Q
position_column: todo
position_ordinal: 9d80
title: Migrate quick-capture.tsx to useDispatchCommand
---
## What\nReplace `backendDispatch` (2 calls) and `dispatchCommand` (3 calls) with `useDispatchCommand` in `kanban-app/ui/src/components/quick-capture.tsx`.\n\n## Acceptance Criteria\n- [ ] No imports of `backendDispatch` or `dispatchCommand`\n- [ ] Scope chain automatic from context\n\n## Tests\n- [ ] `cd kanban-app/ui && pnpm test` — all unit tests pass\n\n## Critical Rule\nIf a module-level function calls `backendDispatch`/`dispatchCommand`, do NOT preserve it. Trace it to the component that calls it. The hook goes in that component. No module-level dispatch functions should exist. See feedback_no_module_level_dispatch.md.