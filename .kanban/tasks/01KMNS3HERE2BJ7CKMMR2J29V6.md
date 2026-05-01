---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffa480
title: Fix useActiveBoardPath returning wrong/undefined path with multiple boards
---
## What

`useActiveBoardPath()` in `kanban-app/ui/src/lib/command-scope.tsx:34` is a trivial `useContext` wrapper around `ActiveBoardPathContext`. The context value is set in `App.tsx:524` from React state initialized at `App.tsx:133`.

### The bug

For the **main window** (no `?board=` URL param), `activeBoardPath` starts as `undefined`. It only gets a value after an async round trip: `get_ui_state` → `file.switchBoard` → `setActiveBoardPath` (around `App.tsx:278`). During that startup gap, all 6 consumers see `undefined`:

1. `field-update-context.tsx:47` — field updates silently fail (no board path to dispatch to)
2. `drag-session-context.tsx:70` — drag sessions can't start
3. `entity-commands.ts:113` — entity context menu commands dispatched without board routing
4. `app-shell.tsx:150` — keyboard shortcuts dispatched without board routing
5. `grid-view.tsx:36` — grid operations target wrong board
6. `command-scope.tsx:259` (`useExecuteCommand`) — all command dispatch misrouted

With **multiple boards open**, switching boards calls `setActiveBoardPath` but the React state update is async — rapid switching can leave consumers with a stale path for one render cycle, causing commands to route to the wrong board.

### Fix approach

The `ActiveBoardPathProvider` value in `App.tsx:524` should not render children that depend on `activeBoardPath` until the path is resolved. Two options:

**Option A (simpler)**: Guard the provider — don't render the board-dependent subtree until `activeBoardPath` is defined. Show the loading state already present in App.tsx during the gap. This is a 2-3 line change in `App.tsx`.

**Option B (more robust)**: Move `activeBoardPath` into a ref-backed context (like `EntityFocusContext` uses for scopes) so the value is synchronous. Heavier change, but eliminates the stale-by-one-render issue on board switch.

Recommend Option A for now — it fixes the `undefined` gap. The stale-by-one-render on switch is less critical and can be a follow-up.

### Files to modify
- `kanban-app/ui/src/App.tsx` — guard rendering until `activeBoardPath` is defined (around line 524)

### Files to add tests
- `kanban-app/ui/src/lib/command-scope.test.tsx` — unit tests for `useActiveBoardPath` and `ActiveBoardPathProvider`

## Acceptance Criteria
- [ ] `useActiveBoardPath()` never returns `undefined` to board-dependent components (field updates, drag sessions, entity commands, keyboard shortcuts)
- [ ] Switching between multiple open boards updates `useActiveBoardPath()` before any command dispatch occurs
- [ ] Loading/splash state shown while `activeBoardPath` is being resolved on startup

## Tests
- [ ] `kanban-app/ui/src/lib/command-scope.test.tsx` (new) — test that `ActiveBoardPathProvider` propagates value to `useActiveBoardPath()` consumers; test that updating the provider value is reflected immediately
- [ ] `kanban-app/ui/src/lib/command-scope.test.tsx` — test that `useActiveBoardPath()` throws or returns `undefined` when no provider is present (documents the edge case)
- [ ] `kanban-app/ui/src/lib/command-scope.test.tsx` — test that `dispatchCommand` includes `boardPath` in invoke args when `useActiveBoardPath()` returns a value
- [ ] Run `cd kanban-app/ui && npx vitest run` — all tests pass