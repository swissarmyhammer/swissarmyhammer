---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff8a80
title: Show loading spinner when switching boards
---
## What

When switching boards via the board selector, stale content from the previous board remains visible during the loading delay. The loading spinner that shows on initial startup should also appear during board switches.

**Root cause:** `handleSwitchBoard` in `kanban-app/ui/src/App.tsx` (line 528) calls `refresh()` which sets `loading=true`, but does NOT clear the `board` state first. The render condition at line 578 is `board && activeBoardPath ? (board content) : loading ? (spinner)`. Since `board` is still the old board object, the stale board content keeps rendering. The spinner only shows when `board` is null.

**Fix:** In `handleSwitchBoard`, clear `board` and `entitiesByType` before calling `refresh()`:

```typescript
const handleSwitchBoard = useCallback(
  async (path: string) => {
    setActiveBoardPath(path);
    activeBoardPathRef.current = path;
    setBoard(null);              // ← ADD: clear stale board so spinner shows
    setEntitiesByType({});       // ← ADD: clear stale entities
    try {
      await backendDispatch({...});
    } catch { /* ignore */ }
    refresh();
  },
  [refresh],
);
```

**Files to modify:**
- `kanban-app/ui/src/App.tsx` — add `setBoard(null)` and `setEntitiesByType({})` at the top of `handleSwitchBoard` (line 530, before the dispatch)

**Note:** After the container refactor, this logic moves to `WindowContainer`. The fix applies to whichever component owns `handleSwitchBoard` at that time.

## Acceptance Criteria
- [ ] Switching boards shows the loading spinner (same Loader2 animation as initial startup)
- [ ] Stale board content does not flash during the switch
- [ ] After loading completes, the new board renders correctly
- [ ] Switching back to the original board also shows the spinner

## Tests
- [ ] Add test case in `kanban-app/ui/src/App.tsx` test or a new `board-switching.test.tsx`: mock `refreshBoards` with a delay, trigger `handleSwitchBoard`, assert spinner is visible, then resolve and assert board content renders
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass
- [ ] Manual: open two boards, switch between them, verify spinner appears during each switch