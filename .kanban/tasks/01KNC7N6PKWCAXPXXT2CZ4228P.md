---
assignees:
- claude-code
depends_on:
- 01KNC7MKFPZNR9MN1D4PYWMG0B
position_column: done
position_ordinal: fffffffffffffffffffd80
position_swimlane: container-refactor
title: Extract WindowContainer from App.tsx
---
## What

Extract a `WindowContainer` component that owns the top-level window scope and all window-lifecycle concerns currently in App.tsx. This is the outermost container in the tree.

**Files to create/modify:**
- `kanban-app/ui/src/components/window-container.tsx` (NEW) — owns the `CommandScopeProvider moniker="window:{WINDOW_LABEL}"`, `TooltipProvider`, `Toaster`, `InitProgressListener`, `ActiveBoardPathProvider`, and `AppShell`
- `kanban-app/ui/src/components/window-container.test.tsx` (NEW) — TDD: tests written first
- `kanban-app/ui/src/App.tsx` — move window-level state into WindowContainer

**Current state:** App.tsx lines 133-225 contain window-level state management (board loading, board switching), and lines 548-664 wrap everything in `CommandScopeProvider > TooltipProvider > ActiveBoardPathProvider`. AppShell wraps ALL content including the no-board placeholder.

**Target:** `WindowContainer` owns:
1. `CommandScopeProvider moniker="window:{label}"`
2. `TooltipProvider`, `Toaster`, `InitProgressListener`
3. `ActiveBoardPathProvider`
4. `AppShell` (global keybindings — must be here, NOT in BoardContainer, because Cmd+O/Cmd+N/undo/redo must work even with no board loaded)
5. Window-level state: `openBoards`, `activeBoardPath`
6. Board-level Tauri event listeners (board-opened, board-changed) — NOT entity events (those are in RustEngineContainer)
7. Board switching logic (`handleSwitchBoard`)
8. Calls `refreshEntities(boardPath)` from RustEngineContainer context on board switch
9. Passes board data down via context or props

**NOTE:** Entity state (`entitiesByType`) and entity event listeners (entity-created, entity-removed, entity-field-changed) do NOT belong here — they are owned by `RustEngineContainer`. Inspector panel state (`panelStack`, `InspectorSyncBridge`) does NOT belong here — it is owned by `InspectorContainer`.

**Pattern:** One file, one container, one CommandScopeProvider, wraps children.

## TDD Process
1. Write `window-container.test.tsx` FIRST with failing tests
2. Tests mock Tauri `invoke`/`listen`/`getCurrentWindow` APIs
3. Tests verify: window scope provider is present, board-opened/board-changed listeners wire up, board switching dispatches file.switchBoard, AppShell keybindings work with no board loaded
4. Implement until tests pass
5. Refactor

## Acceptance Criteria
- [ ] `WindowContainer` exists as a standalone component file
- [ ] `window-container.test.tsx` exists with tests written before implementation
- [ ] App.tsx becomes a thin shell: `WindowContainer > RustEngineContainer > BoardContainer > ...`
- [ ] Board switching still works (board-opened, board-changed events)
- [ ] AppShell global keybindings work even with no board loaded
- [ ] Entity events and panelStack are NOT in this component

## Tests
- [ ] `window-container.test.tsx` — all pass (written first, RED → GREEN)
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass
- [ ] Manual: open app, switch boards, verify Cmd+O and Cmd+N work with no board loaded