---
assignees:
- claude-code
position_column: todo
position_ordinal: c980
title: 'Bug: Drag-and-drop does not move tasks (neither reorder within a column nor across columns)'
---
## What
Reported by user (two symptoms, same drop path):
1. Dragging a task does **not reorder** it within its column.
2. Dragging a task does **not move** it to another column.

Both flow through the same code in `apps/kanban-app/ui/src/components/board-view.tsx`:
- `handleZoneDrop` (board-view.tsx:525) parses the drop payload and, for a same-board drop, calls `persistMove(descriptor, entity.id)`.
- `usePersistTaskMove` (board-view.tsx:468) dispatches `task.move` with `{ id, column, before_id?, after_id? }` and `target: task:<id>` (board-view.tsx:476–482).
- Drop descriptors / neighbor ids come from `apps/kanban-app/ui/src/lib/drop-zones.ts` and `apps/kanban-app/ui/src/lib/neighbor-ids.ts`.

Because BOTH reorder and cross-column fail, the break is likely shared and upstream of the column/ordinal distinction. Candidate root causes to check, in order:
1. **Drop never fires** — `handleZoneDrop` not invoked (drop zones not registering, dragover not preventing default, HTML5 drag payload missing). Add logging / check whether `persistMove` is reached.
2. **Dispatch fails silently** — `task.move` rejects and the error is swallowed by the `catch (e) { console.error }` in `usePersistTaskMove` (board-view.tsx:483). Check the backend `task.move` command handler and whether `before_id`/`after_id`/`column` args are accepted and applied.
3. **Move applied but not reflected** — `task.move` succeeds on the backend but the board-data sync (`apps/kanban-app/ui/src/lib/board-data-sync.ts`) does not re-render the new order/column.

Reproduce: open a board with ≥2 tasks; drag one onto another position (reorder) and onto another column. Capture console + backend logs to see how far the drop gets. (Per project convention, check the macOS unified log: `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"'`, and console.warn instrumentation — do not rely on stderr.)

## Acceptance Criteria
- [ ] Dragging a task to a new position within a column persists the reorder and the new order renders.
- [ ] Dragging a task to another column persists the column change and the card appears in the target column.
- [ ] Root cause identified and documented (drop not firing vs. `task.move` rejecting vs. sync not reflecting).

## Tests
- [ ] Extend `apps/kanban-app/ui/src/components/board-drag-drop.test.tsx` (and/or `column-reorder.browser.test.tsx`) to drive a same-column reorder drop and assert `task.move` is dispatched with the correct `before_id`/`after_id` AND the resulting order updates.
- [ ] Add a cross-column drop assertion: `task.move` dispatched with the target `column` and the card renders in the new column.
- [ ] If the break is backend: a `task.move` integration test asserting column + ordinal placement is applied.
- [ ] Regression tests failing before the fix, passing after.

## Workflow
- Use `/tdd` — failing test first, then fix. #bug