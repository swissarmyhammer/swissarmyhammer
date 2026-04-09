---
assignees:
- claude-code
depends_on:
- 01KN9KZH05YT847ZX8N3ZQT15Q
position_column: done
position_ordinal: ffffffffffffffffffdf80
title: Migrate filter-editor.tsx to useDispatchCommand
---
## What\nReplace `backendDispatch` (3 calls: perspective.filter, perspective.clearFilter x2) with `useDispatchCommand` in `kanban-app/ui/src/components/filter-editor.tsx`. Update `filter-editor.test.tsx` if needed.\n\n## Acceptance Criteria\n- [ ] No imports of `backendDispatch` or `dispatchCommand`\n- [ ] Scope chain automatic from context\n\n## Tests\n- [ ] `cd kanban-app/ui && pnpm test` — all unit tests pass\n\n## Critical Rule\nIf a module-level function calls `backendDispatch`/`dispatchCommand`, do NOT preserve it. Trace it to the component that calls it. The hook goes in that component. No module-level dispatch functions should exist. See feedback_no_module_level_dispatch.md.