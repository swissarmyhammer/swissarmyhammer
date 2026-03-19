---
assignees:
- claude-code
depends_on:
- 01KM32AES07EG43SA1872HCY0D
position_column: done
position_ordinal: fffffff880
title: 'Frontend: strip localStorage/refs, restore state from commands on mount'
---
## What
Replace the frontend's parallel state tracking (localStorage, restoredRefs, fallback logic) with clean command-based initialization. On mount, each window asks the backend for its state and uses it.

**Remove from `kanban-app/ui/src/App.tsx`:**
- `BOARD_PATH_STORAGE_KEY`, `getInitialBoardPath()` — localStorage board persistence
- `localStorage.setItem/removeItem` calls (lines 110-117)
- `inspectorRestoredRef` and its gated restoration logic (lines 121, 203-228, 232)
- `windowsRestoredRef` and its gated restoration logic (lines 190-199)
- The fallback-to-first-board logic in `refresh()` (lines 160-166) — the backend now tells us which board this window shows
- `set_active_board` calls (line 370) — replaced by `switch_board`

**New mount logic in App.tsx:**
1. Get window label: `getCurrentWindow().label`
2. Call `get_window_state(label)` — returns `{ board_path, active_view_id, inspector_stack }`
3. If `board_path` exists, call `open_board(board_path)` (idempotent) then load data
4. If no state (new window), fall back to `list_open_boards` and pick first
5. Pass `active_view_id` down to ViewsProvider (or let it call `get_window_state` itself)
6. Restore inspector stack from the returned state

**Update `kanban-app/ui/src/lib/views-context.tsx`:**
- Remove the `useEffect` that calls `get_ui_context` to restore view (lines 39-62)
- Accept initial `activeViewId` from parent (App.tsx passes it from `get_window_state`)
- `setActiveViewId` calls `set_active_view(windowLabel, viewId)` with the label

**Update `kanban-app/ui/src/App.tsx` board switching:**
- `handleSwitchBoard` calls `switch_board(windowLabel, path)` instead of `set_active_board` + manual state updates

## Acceptance Criteria
- [ ] No `localStorage` usage for board path (quick-capture localStorage is unrelated, leave it)
- [ ] No `restoredRef` or `inspectorRestoredRef` or `windowsRestoredRef`
- [ ] On mount, window state comes from a single `get_window_state` call
- [ ] Hot reload (1st, 2nd, Nth) restores the correct board and view every time
- [ ] Board switching persists immediately via `switch_board` command

## Tests
- [ ] Manual: open board A in main window, board B in secondary → hot reload → both restore correctly
- [ ] Manual: hot reload again → still correct (the second-reload bug is gone)
- [ ] Manual: switch views, hot reload → correct view restored per window
- [ ] `npm run build` in ui/ succeeds (no TypeScript errors)