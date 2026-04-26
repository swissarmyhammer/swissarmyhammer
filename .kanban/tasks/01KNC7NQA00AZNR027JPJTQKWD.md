---
assignees:
- claude-code
depends_on:
- 01KNC7N6PKWCAXPXXT2CZ4228P
- 01KNCQ8YYZV7KB9JY6HF8JS8PT
position_column: done
position_ordinal: ffffffffffffffffffffffffe480
title: Extract BoardContainer from App.tsx
---
## What

Extract a `BoardContainer` component that owns board-level context and commands. Renders a "no board loaded" placeholder or loading spinner when no board is active, and wraps children with board data when a board is loaded.

**Files to create/modify:**
- `kanban-app/ui/src/components/board-container.tsx` (NEW) — owns `CommandScopeProvider moniker="board:{boardId}"`, `FileDropProvider`, `DragSessionProvider`
- `kanban-app/ui/src/components/board-container.test.tsx` (NEW) — TDD: tests written first
- `kanban-app/ui/src/App.tsx` — replace inline board-loading / no-board-loaded conditional with `<BoardContainer>`

**Important:** `AppShell` must NOT live in BoardContainer. It belongs in WindowContainer.

**Target:** `BoardContainer` owns:
1. `CommandScopeProvider moniker="board:{boardId}"`
2. `FileDropProvider`
3. `DragSessionProvider`
4. Conditional rendering: loading → spinner, no board → placeholder, board → children
5. Provides board data to children via a `BoardContext` (new context)

**Pattern:** One file, one container, one CommandScopeProvider, wraps children.

## TDD Process
1. Write `board-container.test.tsx` FIRST with failing tests
2. Tests verify: renders spinner when loading, renders placeholder when no board, renders children when board loaded, BoardContext provides board data to descendants, CommandScopeProvider has correct moniker
3. Implement until tests pass
4. Refactor

## Acceptance Criteria
- [ ] `BoardContainer` exists as a standalone component file
- [ ] `board-container.test.tsx` exists with tests written before implementation
- [ ] Board loading/empty/active states render correctly
- [ ] `AppShell` is NOT inside BoardContainer (it's in WindowContainer)
- [ ] File drop still works
- [ ] Drag-and-drop still works within board view

## Tests
- [ ] `board-container.test.tsx` — all pass (written first, RED → GREEN)
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass
- [ ] Manual: open app with no boards, see placeholder; open a board, see content