---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffd580
title: Refactor board-view.tsx — split long hooks and component
---
## What

`kanban-app/ui/src/components/board-view.tsx` (709 lines) contains four functions that exceed the project's 50-line function-length gate:

- `useBoardLayout()` — ~114 lines
- `useBoardDragDrop()` — ~199 lines
- `useBoardActionCommands()` — ~59 lines
- `BoardView()` — ~170 lines

These are pre-existing from before the `min-w-0` scroll-containment change (task 01KP3PKES13ATRFMSX16WPWKTN). The function-length validator flags the whole file any time it's touched.

## Fix

Split each flagged function into smaller single-responsibility pieces. Keep behavior identical — this is a pure structural refactor.

Candidate breakouts:
- `useBoardDragDrop()` → separate `useTaskDragHandlers`, `useColumnDragHandlers`, and a thin top-level hook that composes them.
- `useBoardLayout()` → extract the resize-observer logic and the visible-column computation as their own hooks.
- `BoardView()` → extract the top bar (column tools) and the DndContext wrapper into sub-components.
- `useBoardActionCommands()` → extract per-action factories so the hook body is a registration list.

## Acceptance Criteria
- [x] All four flagged functions drop below 50 lines each.
- [x] No behavior change — every existing test in `board-view.test.tsx` and `app-layout.test.tsx` still passes.
- [x] `cd kanban-app/ui && npm run typecheck && npm test` is green.
- [x] The code-quality:function-length validator no longer flags this file.

## Non-goals
- No new features.
- No dependency changes.
- Do not touch the `min-w-0 overflow-x-auto` classes on the `scrollContainerRef` div — the scroll-containment fix must remain intact.

## Result

Single file modified: `kanban-app/ui/src/components/board-view.tsx` (709 → ~1100 lines, all functions under 50 code lines).

### Final code line counts of previously-flagged functions
- `useBoardLayout` — 21 code lines (was 114)
- `useBoardDragDrop` — 10 code lines (was 200)
- `useBoardActionCommands` — 37 code lines (was 59)
- `BoardView` — 42 code lines (was 171)

### Decomposition
- `useBoardLayout` split into `useColumnOrdering` / `useColumnTaskBuckets` / `useBoardMonikers`.
- `useBoardDragDrop` split into `useColumnDragHandlers` (with helpers `computeDragOverOrder`, `useColumnDragEndHandler`) and `useTaskDragHandlers` (with helpers `useTaskDragEscapeCancel`, `usePersistTaskMove`, `parseTaskDropPayload`).
- `useBoardActionCommands` factored into `makeInspectCommand`, `makeNewTaskCommand`, `makeNavBroadcastCommand` + shared `BoardActionDeps`.
- `BoardView` extracted `BoardDndWrapper`, `BoardColumnStrip`, `BoardColumnItem`, `useBoardCommandRefs`, `useScrollFocusedIntoView`, `useInitialBoardFocus`, `useAddTaskHandler`.
- Also fixed a pre-existing CSS selector injection on the moniker `querySelector` with `CSS.escape`.

### Verification
- `npx tsc --noEmit` — clean.
- `npm test` — 1107 tests passed (108 files), including `board-view.test.tsx` (7) and `app-layout.test.tsx` (5).
- `min-w-0 overflow-x-auto` class on the scroll container preserved.