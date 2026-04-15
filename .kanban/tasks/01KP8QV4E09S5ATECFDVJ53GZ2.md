---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffd880
title: Unify Rust ordinal placement — DoThisNextCmd uses MoveTask.with_before(), delete duplicated drag command logic
---
## What

The "Do This Next" command is flaky — sometimes it places the task first, sometimes just one position up, sometimes it moves one position then "jumps" to first later. Manually dragging a card to the first slot of the first column is always consistent. The user's hunch: the Rust `DoThisNextCmd` duplicates logic that already lives in `MoveTask` and the drag path, and the duplicates have drifted.

There is a separate task (`01KP63NNA6ME0GWXQZ3GHV4RZ7`) that handles the frontend — deleting the `buildDoThisNextCommand` workaround in `column-view.tsx` that dispatches `task.move` with a stale `before_id` and a hardcoded `"todo"` column. **That frontend fix alone does not remove the Rust-side duplication the user is asking about.** This task covers the backend half.

### The Rust-side duplication

There are currently three near-identical implementations of "compute an ordinal for before/after placement":

1. **`MoveTask::execute`** — `swissarmyhammer-kanban/src/task/mv.rs:108-~215`. The canonical operation. When `before_id` / `after_id` is set on the op, it loads and sorts the column tasks and calls `compute_ordinal_for_neighbors` with the correct neighbor pair.
2. **`MoveTaskCmd`** — `swissarmyhammer-kanban/src/commands/drag_commands.rs:220-329`. The drag command pre-computes the ordinal inline (copy-pasted copy of `MoveTask::execute`'s logic) and then calls `MoveTask::to_column(...).with_ordinal(ord)`. The explicit ordinal wins over `before_id/after_id` in `MoveTask::execute`, so the op's own placement logic is bypassed.
3. **`DoThisNextCmd`** — `swissarmyhammer-kanban/src/commands/task_commands.rs:323-349`. Uses extracted helpers `load_sorted_column_tasks`, `task_ordinal`, and `compute_ordinal_for_neighbors(None, Some(&first_ord))` to pre-compute the ordinal, then calls `MoveTask::to_column(...).with_ordinal(ord)` — same bypass pattern as #2.

All three call `compute_ordinal_for_neighbors(None, Some(&first_ord))` for "place before first task", but through three different call sites with three different excluding/sorting/loading behaviors. Any divergence (a caching choice, an entity filter, a sort tiebreaker) between these paths produces exactly the symptoms the user reports: flaky, almost-correct placement.

### The fix

Funnel everything through `MoveTask::execute`. The op already knows how to compute ordinals from `before_id` / `after_id`. Callers should set those fields — not pre-compute the ordinal.

### Non-goals

- Do NOT change the frontend. Task `01KP63NNA6ME0GWXQZ3GHV4RZ7` owns that.
- Do NOT change the command registration, YAML definitions, or `DoThisNextCmd::available` predicate.
- Do NOT invent a new placement primitive — prefer setting `before_id` / `after_id` on the existing `MoveTask` op.

## Subtasks

- [x] Grep for callers of `load_sorted_column_tasks`, `task_ordinal`, `ordinal_for_before`, `ordinal_for_after`, `compute_placement_ordinal`, and `compute_drop_ordinal` in `commands/task_commands.rs`. Note which survive after the refactor.
- [x] Rewrite `DoThisNextCmd::execute` to set `before_id` on `MoveTask` and let the op compute the ordinal. Drop the pre-computation.
- [x] Rewrite `MoveTaskCmd::execute`'s `before_id`/`after_id` branches in `drag_commands.rs` to set `before_id`/`after_id` on `MoveTask` instead of pre-computing. Decide whether `drop_index` stays in the command or migrates into `MoveTask` (document the decision in the commit message).
- [x] Delete any helpers in `task_commands.rs` that become unused after the above two changes.
- [x] Add the tests listed below; existing Rust tests for `DoThisNextCmd` and the drag flow must still pass unchanged.
- [ ] Manual check with the frontend task (`01KP63NNA6ME0GWXQZ3GHV4RZ7`) applied — rapidly invoke Do-This-Next on 3 tasks in different columns; all 3 end up at the top of column 0, last-clicked at position 0. No flicker.

## Acceptance Criteria

- [x] `DoThisNextCmd::execute` contains no direct call to `compute_ordinal_for_neighbors` — the op computes the ordinal.
- [x] `MoveTaskCmd::execute` (drag path) contains no direct call to `compute_ordinal_for_neighbors` for the `before_id` / `after_id` branches — the op computes those. Any remaining call is only for `drop_index` if that stays in the command, and is exercised by a test.
- [x] Both "Do This Next on a middle-column task" and "drag a task to the first slot of the first column" produce *identical* resulting `position_ordinal` strings given the same starting board state. This is testable: run both paths on an identical seed board and assert the moved task ends up at the same `position_ordinal`.
- [x] Rapidly invoking `DoThisNextCmd` N times in sequence (one per call, no stale client state) produces the N moved tasks at positions 0..N-1 of the first column, with the most-recent call at position 0 (each call recomputes "first" after the previous completed).
- [x] The existing backend tests — `do_this_next_moves_to_first_column`, the drag-move tests, any tests under `-- do_this_next` — all still pass without modification.
- [x] `cargo nextest run -p swissarmyhammer-kanban` green.
- [x] `cargo clippy --all-targets -- -D warnings` clean.
- [x] `cargo fmt --check` clean.

## Tests

- [x] `do_this_next_matches_drag_to_first_slot`: seed a board with 3 columns and tasks, run DoThisNextCmd on one copy and MoveTask.with_before on another, assert identical ordinals.
- [x] `do_this_next_sequential_calls_keep_last_first`: seed a board, call DoThisNextCmd on B, C, D in sequence, assert todo column order is [D, C, B, A].
- [x] `drag_before_first_matches_do_this_next`: drag-complete with beforeId matches DoThisNextCmd result on identical board.
- [x] Full crate check: `cargo nextest run -p swissarmyhammer-kanban && cargo clippy -p swissarmyhammer-kanban --all-targets -- -D warnings` — clean.

## Design Decision: `drop_index` stays in the command

`drop_index` was NOT migrated into `MoveTask` because it is a UI-specific concept (index-based position from drag-and-drop). `MoveTask` deals with neighbor-based placement (`before_id`/`after_id`) and explicit ordinals. Adding `drop_index` would pollute the domain op with presentation concerns. The `drop_index` -> ordinal conversion remains in the command layer (in both `MoveTaskCmd` and `DragCompleteCmd`), using the shared `compute_ordinal_for_drop` helper from `task_helpers`.

## Depends on

- None. This task and `01KP63NNA6ME0GWXQZ3GHV4RZ7` (frontend cleanup) are independent — each is a complete improvement on its own. Running both closes the loop on the flakiness the user is seeing. #drag-and-drop