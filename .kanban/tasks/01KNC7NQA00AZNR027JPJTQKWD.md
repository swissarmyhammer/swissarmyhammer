---
assignees:
- claude-code
depends_on:
- 01KNC7N6PKWCAXPXXT2CZ4228P
position_column: todo
position_ordinal: '8280'
position_swimlane: container-refactor
title: Extract BoardContainer from App.tsx
---
## What

Extract a `BoardContainer` component that owns board-level context and commands. Renders a "no board loaded" placeholder or loading spinner when no board is active, and wraps children with board data when a board is loaded.

**Files to create/modify:**
- `kanban-app/ui/src/components/board-container.tsx` (NEW) — owns `CommandScopeProvider moniker="board:{boardId}"`, `FileDropProvider`, `DragSessionProvider`
- `kanban-app/ui/src/App.tsx` — replace inline board-loading / no-board-loaded conditional with `<BoardContainer>`

**Current state:** App.tsx lines 578-646 contain the board/no-board conditional rendering: board + activeBoardPath renders the main UI; otherwise shows a loading spinner or "No board loaded" placeholder. `FileDropProvider` and `DragSessionProvider` wrap the board content. `AppShell` also lives at this level.

**Target:** `BoardContainer` owns:
1. `CommandScopeProvider moniker="board:{boardId}"`
2. `AppShell` (global commands + keybindings)
3. `FileDropProvider`
4. `DragSessionProvider`
5. Conditional rendering: loading → spinner, no board → placeholder, board → children
6. Provides board data to children via a `BoardContext` (new context, or props)

**Pattern:** One file, one container, one CommandScopeProvider, wraps children.

## Acceptance Criteria
- [ ] `BoardContainer` exists as a standalone component file
- [ ] Board loading/empty/active states render correctly
- [ ] `AppShell` global keybindings still work (Cmd+Z, Cmd+Shift+Z, Cmd+K, etc.)
- [ ] File drop still works
- [ ] Drag-and-drop still works within board view

## Tests
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass
- [ ] Manual: open app with no boards, see placeholder; open a board, see content