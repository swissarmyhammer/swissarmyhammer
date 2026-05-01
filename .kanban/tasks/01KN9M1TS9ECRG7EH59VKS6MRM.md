---
assignees:
- claude-code
depends_on:
- 01KN9KZH05YT847ZX8N3ZQT15Q
position_column: done
position_ordinal: ffffffffffffffffffffffe180
title: Migrate drag-session-context.tsx to useDispatchCommand
---
## What\nReplace `backendDispatch` (3 calls: drag.start, drag.cancel, drag.complete) with `useDispatchCommand` in `kanban-app/ui/src/lib/drag-session-context.tsx`. Note: drag.cancel currently uses `scopeChain: []` with a TODO to thread from caller — the hook fixes this automatically.\n\n## Acceptance Criteria\n- [ ] No imports of `backendDispatch` or `dispatchCommand`\n- [ ] Scope chain automatic for all drag commands (fixes the TODO)\n\n## Tests\n- [ ] `cd kanban-app/ui && pnpm test` — all unit tests pass