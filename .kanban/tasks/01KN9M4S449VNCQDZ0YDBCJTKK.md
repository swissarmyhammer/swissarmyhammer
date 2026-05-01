---
assignees:
- claude-code
depends_on: []
position_column: done
position_ordinal: ffffffffffffffffffffffffff8380
title: Migrate board-view.tsx to useDispatchCommand
---
## What

Replace 3 `backendDispatch` calls in `board-view.tsx` with `useDispatchCommand`. Blocked until the container refactor lands `StoreContainer` (store path moniker) and `BoardContainer` (board entity moniker) — these put the board identity in the scope chain so the backend can resolve the board handle without an explicit `boardPath` parameter.

**Current problems:**
1. `persistMove` (task.move) — hardcodes `scopeChain: [\"task:${taskId}\"]` and passes `descriptor.boardPath` explicitly
2. `column.reorder` — passes `boardPathRef.current` and manual `scopeChain`
3. `task.add` (handleAddTask) — same pattern

**After container refactor:** scope chain is `window:main → store:/path → board:01ABC → column:todo → task:task-1`. The backend resolves the board from the store/board moniker. All 3 calls become plain `useDispatchCommand()` dispatches with args only.

**Files to modify:**
- `kanban-app/ui/src/components/board-view.tsx` — replace 3 `backendDispatch` calls, remove `boardPathRef`, scope/scopeChain manual construction

## Acceptance Criteria
- [ ] No imports of `backendDispatch`, `scopeChainFromScope`, `CommandScopeContext`
- [ ] No `boardPathRef` or manual `scopeChain` construction
- [ ] Board identity comes from scope chain via StoreContainer/BoardContainer
- [ ] Column reorder, task drag, task add all still work

## Tests
- [ ] `cd kanban-app/ui && pnpm vitest run` — all unit tests pass
- [ ] Manual: drag task between columns, reorder columns, add task via \"+\"