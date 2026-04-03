---
assignees:
- claude-code
depends_on:
- 01KN9KZH05YT847ZX8N3ZQT15Q
position_column: todo
position_ordinal: '9e80'
title: Migrate app-shell.tsx to useDispatchCommand
---
## What\nReplace `dispatchCommand` (2 calls) with `useDispatchCommand` in `kanban-app/ui/src/components/app-shell.tsx`. Also replace `useExecuteCommand` usage — `useDispatchCommand` subsumes it.\n\n## Acceptance Criteria\n- [ ] No imports of `backendDispatch`, `dispatchCommand`, or `useExecuteCommand`\n- [ ] Scope chain automatic from context\n\n## Tests\n- [ ] `cd kanban-app/ui && pnpm test` — all unit tests pass\n\n## Critical Rule\nIf a module-level function calls `backendDispatch`/`dispatchCommand`, do NOT preserve it. Trace it to the component that calls it. The hook goes in that component. No module-level dispatch functions should exist. See feedback_no_module_level_dispatch.md.