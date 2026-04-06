---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: b680
title: 'Frontend: stop passing boardPath, let scope chain resolve board'
---
## What

After the backend resolves boards from scope chain window monikers, the frontend no longer needs to pass `boardPath` as an explicit parameter. This card removes `boardPath` from the dispatch path.

**Changes:**

1. `kanban-app/ui/src/lib/command-scope.tsx` — `useDispatchCommand` (line ~383): stop including `boardPath` in the `backendDispatch` call. The scope chain already contains `window:label` which the backend uses to resolve the board. Remove `ActiveBoardPathContext` usage from `useDispatchCommand`. Keep the context provider for now (other consumers may read it).

2. `kanban-app/ui/src/components/board-view.tsx` — remove `boardPathRef`, `scope`/`scopeChain` manual construction. Replace 3 `backendDispatch` calls (`column.reorder`, `task.move` in `persistMove`, `task.add` in `handleAddTask`) with `useDispatchCommand()`. The scope chain from context provides the window and board identity.

3. `kanban-app/ui/src/App.tsx` — remove explicit `boardPath` and `scopeChain: windowScopeChain` from 6 `backendDispatch` calls. Replace with `useDispatchCommand()`. The window-level scope already has `window:label`.

4. `kanban-app/ui/src/components/column-view.tsx` — already migrated to `useDispatchCommand`, verify `boardPath` prop is no longer needed for dispatch (may still be needed for other purposes like drop zones).

**Files to modify:**
- `kanban-app/ui/src/lib/command-scope.tsx` — remove boardPath from backendDispatch in useDispatchCommand
- `kanban-app/ui/src/components/board-view.tsx` — 3 backendDispatch → useDispatchCommand
- `kanban-app/ui/src/App.tsx` — 6 backendDispatch → useDispatchCommand

## Acceptance Criteria
- [ ] `useDispatchCommand` does not pass `boardPath` to backend
- [ ] No `backendDispatch` calls in board-view.tsx
- [ ] No `backendDispatch` calls in App.tsx
- [ ] Board switching, task add, column reorder all still work
- [ ] Multi-window: each window operates on its own board

## Tests
- [ ] `cd kanban-app/ui && pnpm vitest run` — all unit tests pass
- [ ] Manual: open two windows with different boards, verify commands target correct board
- [ ] Manual: add task, reorder column, move task — all work