---
assignees:
- claude-code
position_column: todo
position_ordinal: d380
project: task-card-fields
title: Auto-refresh board view on board/column entity file-change events
---
## What

### Bug symptom
When a new `column` entity file appears on disk (e.g. `.kanban/columns/review.yaml` added by the review skill, a git pull, or the MCP kanban CLI running in another process), the kanban-app board view does NOT redraw to include the new column. The user must close/reopen the board or switch boards to see it. Same applies to column removals and column field edits (rename, reorder).

### Root cause
The frontend has **two** stores of board state and only one of them is event-synced:

1. `entitiesByType` (owned by `kanban-app/ui/src/components/rust-engine-container.tsx`) — listens for `entity-created` / `entity-removed` / `entity-field-changed` Tauri events via `useEntityEventListeners` and stays fresh. Tasks, actors, projects, and tags render from this store and update live.

2. `board: BoardData` (owned by `kanban-app/ui/src/components/window-container.tsx`, exposed via `BoardDataContext` and `useBoardData()`) — populated from `get_board_data` and only updated on `refresh()`, the `board-opened` event, the `board-changed` event, and `handleSwitchBoard`. **Entity events never update it.** `board.columns` is what `BoardView` (`kanban-app/ui/src/components/board-view.tsx` line ~106: `[...board.columns].sort(...)`) iterates to render columns.

The file watcher (`kanban-app/src/watcher.rs`) correctly emits `entity-created { entity_type: "column", id: "review" }` when `review.yaml` appears. The event reaches the frontend. `handleEntityCreated` in `rust-engine-container.tsx` (line ~252) detects `entity_type === "column"` and calls `deps.refreshEntities(...)` — this refreshes `entitiesByType.column` but **discards the `result.boardData`** that `refreshBoards` (`kanban-app/ui/src/lib/refresh.ts`) also fetched. WindowContainer's `board` state never learns about the new column.

A `patchBoardData()` helper already exists at `kanban-app/ui/src/lib/board-data-patch.ts` (tested but **never called from production code**) — it handles field-changed for `board` and `column` entity types but has no add/remove operations.

### Fix

Wire WindowContainer's `board` state to entity events for `board` and `column` entity types. Reuse and extend the existing `patchBoardData` helper instead of introducing a parallel mechanism.

#### 1. Extend `kanban-app/ui/src/lib/board-data-patch.ts`

Add two new exported functions alongside the existing `patchBoardData`:

- `addColumnToBoardData(board: BoardData | null, column: Entity): BoardData | null` — return a new BoardData with the column appended (upsert by id — replace if already present, append if not). Return `null` when `board` is null.
- `removeColumnFromBoardData(board: BoardData | null, id: string): BoardData | null` — return a new BoardData with the column filtered out by id. Return `null` when `board` is null.
- Extend `patchBoardData` to also handle `entity_type === "tag"` → replace in `board.tags` by id (columns and board already covered; tag field-changes are currently dropped too).

Preserve referential equality for unchanged sub-arrays (so React memoization downstream still short-circuits).

#### 2. Add entity event wiring in `kanban-app/ui/src/components/window-container.tsx`

Inside `WindowContainerInner` (next to the existing `useEffect` that listens for `board-opened` / `board-changed`), add a new `useEffect` that subscribes via `listen()` to:

- `entity-created` — if `entity_type` is `"column"` or `"tag"` AND `board_path` matches `activeBoardPathRef.current`, call `addColumnToBoardData` / (for tags, the `patchBoardData` upsert path you extend above) to produce new BoardData, then `setBoard(next)`. Build the `Entity` from the event payload's `fields` map (same pattern as `handleEntityCreated` in `rust-engine-container.tsx`, lines ~257-262).
- `entity-removed` — if `entity_type` is `"column"` or `"tag"` AND `board_path` matches, call `removeColumnFromBoardData` (or tag-equivalent) and `setBoard`.
- `entity-field-changed` — if `entity_type` is `"board"`, `"column"`, or `"tag"` AND `board_path` matches, reconstruct the patched Entity (read current entity from `board`, merge `changes` array into `fields`), then call `patchBoardData(board, entity_type, id, patchedEntity)` and `setBoard(next)`. For the `board` entity type itself, patch `board.board.fields`.

The new effect depends on `[activeBoardPath]` (to guard board_path filtering via the ref) and must clean up listeners on unmount — follow the exact pattern of the existing effect at lines ~269-375.

Guard against `isBoardMismatch` the same way `rust-engine-container.tsx` does (lines ~237-240) — a separate `BoardWatchEvent` payload wrapper ships `board_path` which must match the window's active board.

#### 3. Remove the now-redundant column branch in `rust-engine-container.tsx`

In `handleEntityCreated` (around line 252) and `handleEntityRemoved` (around line 287), the special-case `if (entity_type === "column") { refreshEntities(...) }` branch exists to force a full refetch. After step 2, WindowContainer handles board state for column events directly via the patch path. The RustEngineContainer handler should fall through to the generic `setEntitiesFor(entity_type, ...)` upsert/remove path so `entitiesByType.column` stays in sync without the expensive full refetch. Do NOT delete the refetch logic entirely — it was a workaround for this bug and should be removed now that the bug is fixed.

### Out of scope

- **Task events and `BoardSummary` staleness**: task create/remove affects `board.summary` counters (`total_tasks`, `ready_tasks`, `blocked_tasks`, `done_tasks`, `percent_complete`) which are server-computed by `get_board_data`. Event-level patching can't recompute these without a round-trip. This is a real bug but a separate concern — create a follow-up card if needed. For this card, leave `summary` stale on task events (current behavior).
- **`virtual_tag_meta`**: server-provided metadata that doesn't change at runtime.
- **Projects, actors**: already handled correctly via `entitiesByType` (they don't live in `BoardData`).
- Backend changes: the file watcher (`kanban-app/src/watcher.rs`) and event emission (`kanban-app/src/commands.rs::flush_and_emit_for_handle`) are correct. This is a frontend-only fix.

## Acceptance Criteria

- [ ] Creating `.kanban/columns/<id>.yaml` externally (filesystem write, not via UI command) causes the board view in the kanban-app window to display the new column within ~500ms (file-watcher debounce + event dispatch)
- [ ] Deleting a column YAML file externally removes it from the board view
- [ ] Editing a column's `name` or `order` field externally updates it in-place in the board view (rename shows new name; reorder resorts columns)
- [ ] Editing the board entity's `name` field externally updates the window title / board name label in-place
- [ ] Adding/removing a tag entity externally updates `board.tags` in the view
- [ ] `patchBoardData` correctly handles `board`, `column`, and `tag` entity types for field-changed events; `addColumnToBoardData` and `removeColumnFromBoardData` handle create/remove
- [ ] No duplicate refetch of `get_board_data` for column events — the patch path runs without invoking `refreshBoards`
- [ ] `BoardData.summary` remains stale on task events (out of scope) but does NOT regress for column events

## Tests

- [ ] **Unit tests** in `kanban-app/ui/src/lib/board-data-patch.test.ts` — extend existing file:
  - `addColumnToBoardData appends a new column` — verify result has 3 columns when starting with 2
  - `addColumnToBoardData replaces existing column by id (upsert)` — verify no duplicate when same id
  - `addColumnToBoardData returns null when board is null`
  - `removeColumnFromBoardData filters out column by id`
  - `removeColumnFromBoardData is a no-op when id not found`
  - `removeColumnFromBoardData returns null when board is null`
  - `patchBoardData handles entity_type="tag"` — verify tag is replaced in board.tags
- [ ] **Integration test** in a new file `kanban-app/ui/src/components/window-container.test.tsx` (or extend the existing one if it covers similar ground):
  - Mount `WindowContainer` with a stubbed `refreshBoards` and initial BoardData containing 2 columns
  - Fire a simulated `entity-created` Tauri event for a new `column` with matching `board_path`
  - Assert that `useBoardData().columns` now has 3 columns **without** a second `refreshBoards` call
  - Fire `entity-removed` for one column — assert it disappears
  - Fire `entity-field-changed` for an existing column with a new `name` — assert the name updates
  - Fire an event with mismatched `board_path` — assert BoardData is unchanged
- [ ] **Regression test**: fire an `entity-field-changed` event for the `board` entity itself — assert the board name propagates to `board.board.fields.name`
- [ ] Run: `cd kanban-app/ui && bun test board-data-patch` — all patch tests pass
- [ ] Run: `cd kanban-app/ui && bun test window-container` — all window-container tests pass
- [ ] Run: `cd kanban-app/ui && bun test` — full UI test suite passes, no regressions
- [ ] **Manual verification**: build kanban-app, open a board in the UI, then from a terminal run `echo -e "name: Review\norder: 2" > .kanban/columns/review.yaml` — the new column appears in the UI within a second without user interaction. Repeat with `rm` and an edit to confirm remove/field-change cases.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.
- Before writing any code, read `kanban-app/ui/src/components/rust-engine-container.tsx` (lines 217-343) and `kanban-app/ui/src/components/window-container.tsx` (lines 150-425) end-to-end so the listener wiring pattern is clear.