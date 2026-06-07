---
depends_on:
- 01KTED5F8DQ2XH5BB0WK1MRR3P
position_column: todo
position_ordinal: d980
project: ui-command-cleanup
title: Card F — Move board.* (newTask/firstColumn/lastColumn) to a plugin
---
## What
Move the three `board.*` command DEFINITIONS out of `apps/kanban-app/ui/src/components/board-view.tsx` into a PLUGIN.

Sites in board-view.tsx:
- `makeNewTaskCommand` → `board.newTask`: resolves the focused column, dispatches `entity.addTask` + focus. The column-resolve + add + focus is WEBVIEW orchestration → handler bus (Card B); the underlying `entity.addTask` is already a plugin command and stays the dispatch target.
- `makeNavCommand` ×2 → `board.firstColumn` / `board.lastColumn` → backend op `spatial_navigate` (first/last). These have a real backend op, so route their execute to `spatial_navigate` (no bus needed) — mirror Card A's nav directional handling.

Approach:
- New plugin `builtin/plugins/board-commands/index.ts` (mirror `builtin/plugins/file-commands/index.ts`): `board.firstColumn`/`board.lastColumn` route to `spatial_navigate` (first/last); `board.newTask` is marked "handled in webview" (id/name/keys/scope, menu where applicable).
- In board-view.tsx, delete `makeNewTaskCommand` and the two `makeNavCommand` defs; register a webview handler for `board.newTask` (column-resolve + entity.addTask + focus). firstColumn/lastColumn need no handler — they execute server-side.

## Acceptance Criteria
- [ ] `board.newTask`, `board.firstColumn`, `board.lastColumn` are plugin-defined; board-view.tsx no longer DEFINES them (`makeNewTaskCommand`/`makeNavCommand` removed).
- [ ] firstColumn/lastColumn route to `spatial_navigate`; newTask runs column-resolve + entity.addTask + focus via the bus.
- [ ] New-task and first/last-column behavior unchanged.
- [ ] GUARD (presentation-only invariant): the `board.newTask` handler is orchestration only — it resolves the focused column and focuses the new card, and performs the durable add by dispatching `entity.addTask` through `useDispatchCommand` (NOT inline). board-view.tsx must NOT import `@/lib/mcp-transport`. `webview-command-bus.guard.node.test.ts` stays green. (firstColumn/lastColumn are backend-op routes, not bus handlers — they are exactly the right case to keep OFF the bus.)

## Tests
- [ ] UI: extend `apps/kanban-app/ui/src/components/board-view.column-extremes.spatial.test.tsx` (first/last column → spatial_navigate) and `apps/kanban-app/ui/src/components/column-view.add-task-enter.spatial.test.tsx` (board.newTask adds a task in the focused column + focuses it via the bus).
- [ ] Plugin e2e: the three board.* ids registered with expected metadata + backend-op routing for first/last.
- [ ] `webview-command-bus.guard.node.test.ts` green with board-view.tsx as a registration site.
- [ ] Relevant vitest files green.

## Workflow
- Use `/tdd` — failing tests first, then implement. Automated tests only.