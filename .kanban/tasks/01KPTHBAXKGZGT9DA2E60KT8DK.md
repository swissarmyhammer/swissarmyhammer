---
assignees:
- claude-code
position_column: todo
position_ordinal: '9680'
title: Board view doesn't redraw after undoing a column drag — wire patchBoardData into entity-field-changed
---
## What

**Repro**: Drag a column to a new position on the board. The view shows the new order (correct). Hit Undo. The backend reverts the column orders on disk, but the board view does not redraw — columns stay visually in the post-drag order.

**Root cause (confirmed via code tracing)**: `patchBoardData` in `kanban-app/ui/src/lib/board-data-patch.ts` has 7 passing unit tests and was explicitly written to keep `BoardData` in sync with `entity-field-changed` events for structural entity types (board/column/swimlane). But it is **not imported anywhere in production code** — only in its own test file. Confirmed dead code:
- `git log --all -- kanban-app/ui/src/lib/board-data-patch.ts` shows it was added in commit `7b767ff69` ("fix: address 5 review findings — dedup perf, log level, board patching, useMemo, upsert") as a utility, never wired in.
- `grep -r 'import.*patchBoardData|from.*board-data-patch'` returns only the test file.

**Why it only surfaces on undo (not drag)**: The drag handler in `kanban-app/ui/src/components/board-view.tsx:363-409` holds an optimistic `virtualColumnOrder` state after drag-end. The `useEffect` that clears it only fires when `columnIdList` changes. Since `board.columns` is never patched on `entity-field-changed`, `columnIdList` never changes post-drag — so the virtual order stays visible and hides the underlying staleness. On undo, there's no virtual state to mask the bug: `board.columns` is still at initial load state, so the UI shows the initial order *plus* whatever `virtualColumnOrder` was left holding — not the reverted disk state.

**The data flow gap**:
1. `ColumnReorderCmd::execute` (`swissarmyhammer-kanban/src/commands/column_commands.rs:22-73`) runs N `UpdateColumn` ops, each emitting one `entity-field-changed` event.
2. `handleEntityFieldChanged` (`kanban-app/ui/src/components/rust-engine-container.tsx:360-387`) patches only `entitiesByType["column"]` via `setEntitiesFor`.
3. `board.columns` — the state `useColumnOrdering(board)` in `board-view.tsx:83-101` reads from — lives in `WindowContainerInner` (`window-container.tsx:183`) and is only mutated by full board refreshes (`setBoard(result.boardData)`), never by field events.
4. `BoardDataContext.Provider value={board}` (`window-container.tsx:533`) hands stale data to the whole board subtree.

**Fix**: wire `patchBoardData` into `handleEntityFieldChanged` so `board.columns` stays in sync with column field changes.

**Approach** (chosen over "full refresh on column field change" because it aligns with the project's event-architecture rule — "events are thin: entity-level + field-level. No enrichment reads, no re-fetch round-trips."):

1. In `kanban-app/ui/src/components/window-container.tsx`, expose `setBoard` upward via a new context (mirror the existing `SetEntitiesByTypeContext` / `useSetEntitiesByType` pattern at lines 118-130 of `rust-engine-container.tsx`). Add:
   - `SetBoardDataContext` created in `rust-engine-container.tsx` (or a new shared file).
   - `useSetBoardData()` hook.
   - Wrap the provider around `RustEngineContainer`'s children so downstream consumers can call it.
   - In `WindowContainerInner` (`window-container.tsx:178-232`), pass `setBoard` into the `SetBoardDataContext.Provider`.

   *Alternative*: move `BoardData` state up into `RustEngineContainer`. Skip if it causes a bigger ripple — the context approach is cheaper.

2. In `handleEntityFieldChanged` (`rust-engine-container.tsx:360-387`), after computing the patched entity:
   - When `entity_type` is `"board"` or `"column"`, construct the post-patch `Entity` object (same shape the setter-fn already builds).
   - Call `setBoard(prev => patchBoardData(prev, entity_type, id, patchedEntity) ?? prev)`.
   - `patchBoardData` returns `null` for non-structural types or when `board` is null; the `?? prev` keeps the call idempotent and safe for race conditions.

3. No changes to `patchBoardData` itself — its signature and behavior are exactly what's needed.

**Why not fix the per-op-undo issue too**: Column reorder creates N undoable entries (one per column). A single Undo reverts only the last `UpdateColumn`. The user's report ("undo does appear to work") suggests they pressed Undo multiple times or verified on disk. Grouping the N ops into a single undoable transaction is a separate concern — track it independently, don't bundle. The redraw bug exists regardless of undo granularity.

## Acceptance Criteria

- [ ] Drag a column to a new position, press Undo: the board view redraws columns in their pre-drag order without manual refresh or reopening the board.
- [ ] Rename a column via any command path: the column header in the board view updates immediately (this was also broken, same root cause).
- [ ] Rename the board via any command path: the board name in the nav bar updates immediately (same root cause for `entity_type: "board"`).
- [ ] No regressions: adding a column still shows it in the view; removing a column still removes it (both currently work via `refreshEntities` in `handleEntityCreated` / `handleEntityRemoved` — leave that path alone).
- [ ] Task-level field changes (e.g. changing a task's column) still work and do NOT trigger a board refetch (patchBoardData returns null for non-structural types).

## Tests

- [ ] New browser-mode test `kanban-app/ui/src/components/column-reorder-undo.browser.test.tsx`:
  1. Render the full board with 3 columns `[todo, doing, done]`.
  2. Drag `todo` to the end → assert DOM shows `[doing, done, todo]` via `virtualColumnOrder` optimistic render.
  3. Await the dispatch round-trip.
  4. Dispatch the `undo` command.
  5. Assert DOM reverts to `[todo, doing, done]` without any explicit `refresh` call.
- [ ] New integration test `kanban-app/ui/src/lib/board-data-patch-integration.test.tsx`:
  1. Mount `RustEngineContainer` + `WindowContainer` with an initial board.
  2. Fire a mocked Tauri `entity-field-changed` event for `entity_type: "column"` with a new `order` value.
  3. Read back `board.columns` from `BoardDataContext` and assert it reflects the change.
  4. Fire the same event shape for `entity_type: "task"` — assert `board.columns` is unchanged (patchBoardData null-branch).
- [ ] Existing tests still pass:
  - `kanban-app/ui/src/lib/board-data-patch.test.ts` (unit tests for patchBoardData — no signature changes)
  - `kanban-app/ui/src/components/column-reorder.browser.test.tsx` (existing drag behavior)
  - `kanban-app/ui/src/components/rust-engine-container.test.tsx` (event handler shape)
- [ ] Run: `cd kanban-app/ui && bun test` — all passing.

## Workflow

- Use `/tdd` — write the failing column-drag-undo browser test first, then wire `patchBoardData` into `handleEntityFieldChanged` to make it pass.
- Touch only the event plumbing; do not modify `ColumnReorderCmd`, `UndoCmd`, or the optimistic `virtualColumnOrder` hook. #bug #drag-and-drop #events #frontend