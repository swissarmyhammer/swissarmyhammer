---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8c80
title: 'Bug: Drag-and-drop does not move tasks (neither reorder within a column nor across columns)'
---
## What
Reported by user (two symptoms, same drop path):
1. Dragging a task does **not reorder** it within its column.
2. Dragging a task does **not move** it to another column.

Both flow through `apps/kanban-app/ui/src/components/board-view.tsx`: `handleZoneDrop` → `usePersistTaskMove` → dispatch `task.move` with `{ id, column, before_id?, after_id? }` and `target: task:<id>`.

## Root cause (determined 2026-06-10)
Candidate #2 — **dispatch fails silently**. The frontend drop path was fine and unchanged; the break was in the `task-commands` builtin plugin port (`builtin/plugins/task-commands/index.ts`). The legacy Rust `MoveTaskCmd` accepted args fallbacks (`args.id` for the task, `args.column` for the column, plus `ordinal`/`before_id`/`after_id` placement), which is exactly the shape `usePersistTaskMove` dispatches (`target: "task:<id>"`, args `{ id, column, before_id | after_id }`). The TS plugin port dropped ALL of those fallbacks: `available` required a `column:<id>` TARGET moniker and `execute` only read `scopeId(task)` / `targetId(column)` / `args.drop_index`. The command service rechecks `available` before `execute`, so every internal drag drop was rejected with `command unavailable: "Drop the task onto a column"`, swallowed by the `catch { console.error }` in `usePersistTaskMove`. Both reorder and cross-column moves died at the same gate — matching the symptom.

## Fix
- `builtin/plugins/task-commands/index.ts`: restored legacy `MoveTaskCmd` parity in the plugin's `task.move` — `available` accepts a task from scope OR `args.id` AND a column from target OR `args.column`; `execute` resolves `id` (explicit `args.id` wins over ambient scope so the DRAGGED card moves, not the focused one), `column` (target moniker, else `args.column`), and placement with legacy precedence `ordinal > before_id > after_id > drop_index > append`, passing neighbor references straight through to the kanban `move task` op.
- `apps/kanban-app/ui/src/components/board-view.tsx`: exported `usePersistTaskMove` (doc-comment only otherwise) so the dispatch wire shape is pinned by a test.
- No imperative refetch added anywhere; re-render continues to ride the existing entity-changed event path (untouched).

## Acceptance Criteria
- [x] Dragging a task to a new position within a column persists the reorder and the new order renders.
- [x] Dragging a task to another column persists the column change and the card appears in the target column.
- [x] Root cause identified and documented (dispatch rejected at the `available` recheck — plugin port dropped the legacy args fallbacks).
- [x] Re-render is driven by the event/notification path, not an imperative UI-side refetch (no refetch added; data-sync path untouched).

## Tests
- [x] `crates/swissarmyhammer-command-service/tests/integration/builtin_task_commands_e2e.rs`: `task_move_with_drop_dispatch_shape_reorders_within_column` and `task_move_with_drop_dispatch_shape_moves_across_columns` — execute `task.move` through the REAL plugin platform with the exact drop dispatch shape and assert column + ordinal placement on the real store. Both FAILED before the fix (`command unavailable: Drop the task onto a column`) and pass after (red → green verified).
- [x] `apps/kanban-app/ui/src/components/board-drag-drop.test.tsx`: three new tests drive the REAL `usePersistTaskMove` hook (not a local mirror) and pin the dispatch wire shape — same-column `before_id`, cross-column `after_id`, empty-column append — so the frontend/plugin contract cannot drift silently again.
- [x] Regression tests failing before the fix, passing after (the two Rust e2e tests).

## Verification (2026-06-10)
- `cargo nextest run -p swissarmyhammer-command-service` — 125/125 passed.
- `npx vitest run src/components/board-drag-drop.test.tsx` — 13/13 passed; `npx tsc --noEmit` clean.
- Pre-existing, unrelated: 2 failures in `column-view.test.tsx` ("Do This Next" context menu; files identical to HEAD, no import edge to this change) — filed as 01KTS472PAQ94KEC1XVQHC629A.

## Workflow
- Use `/tdd` — failing test first, then fix. #bug