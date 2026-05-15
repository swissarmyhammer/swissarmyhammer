---
assignees:
- claude-code
depends_on:
- 01KN9KZH05YT847ZX8N3ZQT15Q
position_column: done
position_ordinal: fffffffffffffffffffffff380
title: Migrate entity-commands.ts to useDispatchCommand
---
## What\nReplace `backendDispatch` (2 calls) in `kanban-app/ui/src/lib/entity-commands.ts`. Two functions to update:\n- `useEntityCommands` (hook) — use `useDispatchCommand` directly\n- `buildEntityCommandDefs` (non-hook factory) — accept a dispatch function as parameter instead of calling `backendDispatch` directly. Callers of `buildEntityCommandDefs` must pass the dispatch function from the hook.\n\n## Acceptance Criteria\n- [ ] No imports of `backendDispatch` or `dispatchCommand`\n- [ ] `useEntityCommands` uses `useDispatchCommand` internally\n- [ ] `buildEntityCommandDefs` accepts dispatch function parameter\n\n## Tests\n- [ ] `cd kanban-app/ui && pnpm test` — all unit tests pass\n\n## Critical Rule\nIf a module-level function calls `backendDispatch`/`dispatchCommand`, do NOT preserve it. Trace it to the component that calls it. The hook goes in that component. No module-level dispatch functions should exist. See feedback_no_module_level_dispatch.md.