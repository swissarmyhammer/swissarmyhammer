---
assignees:
- claude-code
depends_on:
- 01KNC7MKFPZNR9MN1D4PYWMG0B
position_column: todo
position_ordinal: '8180'
position_swimlane: container-refactor
title: Extract WindowContainer from App.tsx
---
## What

Extract a `WindowContainer` component that owns the top-level window scope and all window-lifecycle concerns currently in App.tsx. This is the outermost container in the tree.

**Files to create/modify:**
- `kanban-app/ui/src/components/window-container.tsx` (NEW) — owns the `CommandScopeProvider moniker="window:{WINDOW_LABEL}"`, `TooltipProvider`, `Toaster`, `InitProgressListener`, `ActiveBoardPathProvider`
- `kanban-app/ui/src/App.tsx` — move window-level state (board switching, event listeners, `openBoards`, `activeBoardPath`, `panelStack`) into WindowContainer

**Current state:** App.tsx lines 133-225 contain window-level state management (board loading, entity event listeners, board switching), and lines 548-664 wrap everything in `CommandScopeProvider > TooltipProvider > ActiveBoardPathProvider`. The `InspectorSyncBridge` and all entity event listeners are also window concerns.

**Target:** `WindowContainer` owns:
1. `CommandScopeProvider moniker="window:{label}"`
2. Window-level state: `board`, `entitiesByType`, `openBoards`, `activeBoardPath`, `panelStack`
3. All Tauri event listeners (entity-created, entity-removed, entity-field-changed, board-opened, board-changed)
4. Board switching logic (`handleSwitchBoard`)
5. Inspector panel state + `InspectorSyncBridge`
6. Passes board data down via context or props

**Pattern:** One file, one container, one CommandScopeProvider, wraps children.

## Acceptance Criteria
- [ ] `WindowContainer` exists as a standalone component file
- [ ] App.tsx becomes a thin shell: `WindowContainer > RustEngineContainer > BoardContainer > ...`
- [ ] All window-level state and event listeners moved out of App.tsx
- [ ] Entity event listeners still fire and patch state correctly
- [ ] Board switching still works (board-opened, board-changed events)
- [ ] Inspector panels still render via panelStack

## Tests
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass
- [ ] Manual: open app, switch boards, verify entity updates render