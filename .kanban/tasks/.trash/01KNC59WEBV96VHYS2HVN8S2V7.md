---
assignees:
- claude-code
position_column: todo
position_ordinal: ac80
title: Remove data props from view containers — BoardView/GridView read from hooks
---
## What

`ViewContainer` and `PerspectiveContainer` currently pass `board: BoardData`, `tasks: Entity[]`, and `boardPath` as props, threading them from App.tsx down through containers that shouldn't care about data. Containers are scope providers — they should take no data props.

All this data is already available via hooks:
- `boardPath` → `useActiveBoardPath()` (from `ActiveBoardPathProvider`)
- entities → `useEntityStore().getEntities(type)` (from `EntityStoreProvider`)
  - `board.columns` → `getEntities("column")` (sort by `position_ordinal`)
  - `board.swimlanes` → `getEntities("swimlane")`
  - `board.tags` → `getEntities("tag")`
  - `board.board` → `getEntities("board")[0]`
  - tasks → `getEntities("task")`
  - actors → `getEntities("actor")`

### Files to modify

- **`kanban-app/ui/src/components/view-container.tsx`** — Remove `board`, `tasks`, `boardPath` props. Just a scope container.
- **`kanban-app/ui/src/components/perspective-container.tsx`** — Remove `board`, `tasks`, `boardPath` props. `ViewDisplay` reads from hooks.
- **`kanban-app/ui/src/components/board-view.tsx`** — Remove `BoardViewProps`. Read `boardPath` from `useActiveBoardPath()`, columns/swimlanes/board entity from `useEntityStore()`, tasks from `useEntityStore().getEntities("task")`. Sort columns by `position_ordinal`.
- **`kanban-app/ui/src/components/grid-view.tsx`** — Already reads entities from `useEntityStore()`. Remove any remaining prop threading.
- **`kanban-app/ui/src/App.tsx`** — `<ViewContainer />` with no data props. Remove the prop threading from App's render.

### Key insight

A View isn't about tasks — it's about whatever entity type the `ViewDef.entity_type` specifies. `GridView` already gets this right (reads `view.entity_type`). `BoardView` hardcodes tasks but should also be driven by the view definition.

## Acceptance Criteria

- [ ] `ViewContainer` and `PerspectiveContainer` take zero data props
- [ ] `BoardView` reads columns, board entity, and tasks from `useEntityStore()`
- [ ] `BoardView` reads `boardPath` from `useActiveBoardPath()`
- [ ] `App.tsx` renders `<ViewContainer />` with no props
- [ ] No prop drilling of `board`, `tasks`, or `boardPath` through container hierarchy

## Tests

- [ ] `board-view.test.tsx` — update: provide data via `EntityStoreProvider` instead of props
- [ ] `pnpm vitest run board-view perspective-tab-bar` — all pass
- [ ] `cd kanban-app/ui && pnpm test` — no regressions