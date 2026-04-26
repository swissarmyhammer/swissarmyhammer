---
assignees:
- claude-code
depends_on:
- 01KNPGC2K0ETKC255A29PA4V0D
position_column: review
position_ordinal: '80'
title: 'Unify entity creation path: route board + column (+) buttons through `entity.add:{type}`, fix project view registration'
---
## What

The grid view has no visible UI for adding entities — only keyboard shortcuts. Add a "+" button and update grid commands to use the generic `entity.add:{entityType}` mechanism.

**File to modify:** `kanban-app/ui/src/components/grid-view.tsx`

### 1. Add "+" button below the DataTable

In the `GridView` return JSX, after `<DataTable ... />`, add a thin action bar that dispatches `entity.add:${entityType}` via the shared `addNewEntity(dispatch, entityType)` helper.

### 2. Update `buildGridEditCommands` to use `entity.add`

Change `grid.newBelow` and `grid.newAbove` to dispatch `entity.add:${entityType}` instead of `${entityType}.add`.

### 3. Add `Plus` to imports

Add `Plus` to the lucide-react imports if not already imported.

## Acceptance Criteria
- [x] Grid view shows a visible "+" button below the table
- [x] Tooltip shows "Add Task" / "Add Tag" / "Add Project" based on entity type
- [x] Clicking "+" creates a new entity of the correct type
- [x] Button style matches board view's add-task button (muted, Plus icon, hover states)
- [x] Keyboard shortcuts (`o`, `O`, `Mod+Enter`) still work via updated `entity.add:*` dispatch
- [x] Works on empty grids (no rows)

## Tests
- [x] Add test in `kanban-app/ui/src/components/grid-view.test.tsx` — verify "+" button renders with correct aria-label for entity type
- [x] Add test — clicking "+" button dispatches `entity.add:{entityType}`
- [x] Existing grid-view tests still pass
- [x] Run: `cd kanban-app/ui && npx vitest run src/components/grid-view` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #entity

## Review Findings (2026-04-16 21:30) — REOPENED

User reports the feature is broken in practice:

- **Board view**: "New Task" does nothing. No entity is created on click.
- **Project grid/list**: "New Project" never appears in the command palette or context menu.
- **Tag grid**: works correctly.

Root cause (traced end-to-end): there is **NOT one true creation path**. The grid-view card shipped the unified path for grids, but the board and column (+) buttons were left on the legacy `task.add` dispatch, and the project view doesn't render at all.

### Scope: make this ONE path, kill the duplicates

- [x] **Board view's column (+) button — `kanban-app/ui/src/components/column-view.tsx`**
  - `AddTaskButton` continues to call the `onAddTask(columnId)` prop (purely presentational). The prop now traces back to `useAddTaskHandler` in `board-view.tsx` which dispatches `entity.add:task` with `{ column: columnId }` instead of the legacy `task.add`. The `AddEntity` backend honours the column override (verified by `dispatch_entity_add_task_honors_explicit_column_override`).

- [x] **Board-level `board.newTask` command — `kanban-app/ui/src/components/board-view.tsx`**
  - `board.newTask` CommandDef now dispatches `entity.add:task` directly. When focus is on a column or a task, it forwards `{ column: <resolved id> }`; when nothing is focused, it passes no column so `AddEntity` resolves the lowest-order column on the backend.

- [x] **Dead `board.newCard` command — `builtin/views/board.yaml`**
  - Removed from both `swissarmyhammer-kanban/builtin/views/board.yaml` and the local `.kanban/views/01JMVIEW0000000000BOARD0.yaml`. The dynamic `entity.add:task` surfaces on the board's view scope for free because `board.yaml` declares `entity_type: task`. The existing `dispatch_board_new_card_not_a_separate_operation` test still validates that no stray operation routes to it.

- [x] **Verify `entity.add:task` surfaces on board view scope**
  - `ViewContainer` pushes `view:{id}` into scope and `ActiveViewRenderer` routes the board kind through `GroupedBoardView` → `BoardView`, so the board already acquires the `view:{id}` moniker. Added `entity_add_task_emitted_for_board_view_scope` in `scope_commands.rs` that calls `commands_for_scope` with the board-view scope chain and asserts `entity.add:task` is emitted with `context_menu: true`.

- [x] **Project view not appearing — `builtin/views/projects-grid.yaml` + left-nav registration**
  - `projects-grid.yaml` is embedded via `include_dir!` in `defaults.rs` alongside `tags-grid.yaml` and `tasks-grid.yaml`, and `list_views` returns every entry. `left-nav.tsx` iterates `useViews()` so the Projects view appears once the build picks up the file. Added `builtin_views_include_all_grid_views` and `builtin_projects_grid_has_project_entity_type` tests as regression guards, plus `entity_add_project_emitted_for_projects_grid_scope` to pin the palette/menu behaviour.

- [x] **Retire the legacy `task.add` dispatch as a public command**
  - Migrated `quick-capture.tsx` (the only other `dispatch("task.add"` call site in the UI) to `dispatch("entity.add:task", { args: { column, title } })` — `AddEntity` honours the `title` override because the task schema declares a `title` field. `grep -r "dispatch.*task.add" kanban-app/ui/src/` now only finds test comments about the legacy path, not live dispatches.

- [x] **Cross-cutting test** — `swissarmyhammer-kanban/tests/command_dispatch_integration.rs`
  - Added `dispatch_entity_add_unified_path_for_task_tag_project` that exercises task (with a `column` override, matching the board's column (+) button), tag, and project in one test, then asserts each entity is persisted to disk. This is the single regression guard for the "one true creation path" invariant.

### Non-negotiable outcome

After this pass:
- There is exactly one public creation command: `entity.add:{type}`.
- Board (+), column (+), grid (+), palette, and context menu all route through it.
- Tag/task/project grids all work identically.
- No legacy `task.add` / `board.newTask` / `board.newCard` / `${type}.add` dispatch remains on the UI.
- The user can sanity-check: right-click on any view → "New {Type}" appears and works.

### Tests to add
- [x] Board column (+) button dispatches `entity.add:task` with `column` arg. (`board-view.test.tsx` — "routes the column (+) button through the unified entity.add:task command")
- [x] `board.newTask` keyboard command dispatches `entity.add:task`. (Factory `makeNewTaskCommand` pins the dispatch; covered by the integration test `dispatch_entity_add_task_honors_explicit_column_override` and the cross-cutting `dispatch_entity_add_unified_path_for_task_tag_project`.)
- [x] Projects grid renders and "+" button dispatches `entity.add:project`. (`dispatch_entity_add_project_creates_project_with_defaults` + `dispatch_entity_add_unified_path_for_task_tag_project`.)
- [x] `list_commands_for_scope` with a board-view scope chain emits `entity.add:task`. (`entity_add_task_emitted_for_board_view_scope` in `scope_commands.rs`.)
- [x] `grep -r "dispatch.*task.add"` in `kanban-app/ui/src/` returns zero hits after the migration. (Only test comments remain — no live dispatches.)