---
assignees:
- claude-code
position_column: review
position_ordinal: '8180'
title: Refactor oversized functions in column-view, grid-view, and board-view to pass 50-line validator
---
The code quality validator is blocking on functions exceeding 50 lines in files modified during the spatial navigation project. These are pre-existing functions — our changes only deleted code from them. But the validator checks all functions in changed files.

## Files and functions to refactor

### column-view.tsx
- [x] **ColumnView** (209 lines) — Extracted `useColumnViewState()` for hook orchestration, `ColumnHeader` and `AddTaskButton` sub-components for JSX, `useAutoScroll()` for rAF scroll logic, `useColumnCommands()` for command building, `useDragOverHandler()` for drag-over.
- [x] **VirtualColumn** (104 lines) — Extracted `VirtualRowItem` for per-row rendering, `virtualRowStyle` helper, shared `CardItemProps` interface.
- [x] **VirtualizedCardList** (86 lines) — Extracted `CardWithZone` helper component, shared `CARD_LIST_CONTAINER_CLASS` constant.

### grid-view.tsx
- [x] **useGridNavigation** (62 lines) — Extracted `useCellMonikers()` for moniker matrix building, `useGridInitialFocus()` for one-time focus seeding.
- [x] **useGridCallbacks** (62 lines) — Extracted `renderCellEditor()` pure helper function.
- [x] **buildGridEditCommands** (71 lines) — Compressed into `GRID_EDIT_DESCRIPTORS` data table + `gridEditExecutor()` switch function + map pattern.
- [x] **GridView** (63 lines) — Extracted `useGridViewState()` hook for all hook orchestration.

### board-view.tsx
- [x] **useTaskDragHandlers** (52 lines) — Extracted `useZoneDropHandler()` for the zone-drop callback.
- [x] **useColumnTaskBuckets** (52 lines) — Extracted `buildBaseLayout()` pure function for column bucketing.

## Rules
- Do NOT change any behavior
- Do NOT change any test assertions
- Keep every extracted function/hook under 50 lines
- Add docstrings to extracted functions
- Run tests and verify all pass (only allowed failure: board-integration.browser.test.tsx)