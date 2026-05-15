---
assignees:
- claude-code
depends_on:
- 01KN9KZH05YT847ZX8N3ZQT15Q
position_column: done
position_ordinal: ffffffffffffffffffffffdf80
title: Migrate context-menu.ts to useDispatchCommand
---
## What\nReplace `backendDispatch` (1 call) in `kanban-app/ui/src/lib/context-menu.ts`. Special case: `dispatchContextMenuCommand` is a module-level function, not a hook. The scope chain is captured at menu-open time (`pendingScopeChain`). Options:\n- Accept a dispatch function as parameter from `useContextMenu` (the hook caller)\n- Or keep using the private `_backendDispatch` with the captured scope chain since this is the one legitimate non-hook case\n\n## Acceptance Criteria\n- [ ] No imports of `backendDispatch` (public export)\n- [ ] Context menu dispatch still uses the scope chain captured at menu-open time\n\n## Tests\n- [ ] `cd kanban-app/ui && pnpm test` — all unit tests pass