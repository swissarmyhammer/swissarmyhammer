---
assignees:
- claude-code
position_column: todo
position_ordinal: d480
title: Unify Rust ordinal placement — DoThisNextCmd uses MoveTask.with_before(), delete duplicated drag command logic
---
## What

The "Do This Next" command is flaky — sometimes it places the task first, sometimes just one position up, sometimes it moves one position then "jumps" to first later. Manually dragging a card to the first slot of the first column is always consistent. The user's hunch: the Rust `DoThisNextCmd` duplicates logic that already lives in `MoveTask` and the drag path, and the duplicates have drifted.

There is a separate task (`01KP63NNA6ME0GWXQZ3GHV4RZ7`) that handles the frontend — deleting the `buildDoThisNextCommand` workaround in `column-view.tsx` that dispatches `task.move` with a stale `before_id` and a hardcoded `"todo"` column. **That frontend fix alone does not remove the Rust-side duplication the user is asking about.** This task covers the backend half.

### The Rust-side duplication

There are currently three near-identical implementations of "compute an ordinal for before/after placement":

1. **`MoveTask::execute`** — `swissarmyhammer-kanban/src/task/mv.rs:108-~215`. The canonical operation. When `before_id` / `after_id` is set on the op, it loads and sorts the column tasks and calls `compute_ordinal_for_neighbors` with the correct neighbor pair.
2. **`MoveTaskCmd`** — `swissarmyhammer-kanban/src/commands/drag_commands.rs:220-329`. The drag command pre-computes the ordinal inline (copy-pasted copy of `MoveTask::execute`'s logic) and then calls `MoveTask::to_column(...).with_ordinal(ord)`. The explicit ordinal wins over `before_id/after_id` in `MoveTask::execute`, so the op's own placement logic is bypassed.
3. **`DoThisNextCmd`** — `swissarmyhammer-kanban/src/commands/task_commands.rs:323-349`. Uses extracted helpers `load_sorted_column_tasks`, `task_ordinal`, and `compute_ordinal_for_neighbors(None, Some(&first_ord))` to pre-compute the ordinal, then calls `MoveTask::to_column(...).with_ordinal(ord)` — same bypass pattern as

All three call `compute_ordinal_for_neighbors(None, Some(&first_ord))` for "place before first task", but through three different call sites with three different excluding/sorting/loading behaviors. Any divergence (a caching choice, an entity filter, a sort tiebreaker) between these paths produces exactly the symptoms the user reports: flaky, almost-correct placement.

### The fix

Funnel everything through `MoveTask::execute`. The op already knows how to compute ordinals from `before_id` / `after_id`. Callers should set those fields — not pre-compute the ordinal.

**Files to change (all under `swissarmyhammer-kanban/src/`):**

1. **`commands/task_commands.rs`**
   - Rewrite `DoThisNextCmd::execute` to:
     ```rust
     let todo_col_id = first_column_id(&kanban).await?;
     let first_task_id = first_task_id_in_column(&kanban, &todo_col_id, task_id).await?;
     let mut op = crate::task::MoveTask::to_column(task_id, todo_col_id);
     if let Some(id) = first_task_id {
         op = op.with_before(id);
     }
     run_op(&op, &kanban).await
     ```
     Extract a small `first_task_id_in_column(kanban, col, exclude) -> Option<String>` helper (or inline: load tasks in column sorted by ordinal excluding `task_id`, return `.first().map(|t| t.id.as_str().to_owned())`).
   - Remove the now-unused `load_sorted_column_tasks`, `task_ordinal`, `ordinal_for_before`, `ordinal_for_after`, `compute_placement_ordinal`, `compute_drop_ordinal` helpers from this file IF nothing else uses them. Verify via grep before deleting. If `MoveTaskCmd` also gets simplified below, the drag command's usage of these helpers goes away too.

2. **`commands/drag_commands.rs`**
   - Rewrite the `before_id` / `after_id` / `drop_index` branches in `MoveTaskCmd::execute` (lines ~220-329) to set the fields on the `MoveTask` op rather than pre-computing the ordinal. For `drop_index`, `MoveTask` does not currently handle that directly — either extend `MoveTask` with a `drop_index` field and move the logic there, or keep a single minimal `drop_index` → `ordinal` conversion in the drag command (but stop duplicating the `before_id`/`after_id` math).
   - If `drop_index` is only used from the drag path, consider folding it into `MoveTask` as an additional placement mode. That keeps one canonical placement API. Judgment call — if it balloons the op, leave the drop-index conversion in the drag command but share the column-loading helper with `MoveTask` (pull it into `task_helpers`).

3. **`task/mv.rs`** — no behavior change required. `MoveTask::execute` already correctly handles `before_id` / `after_id`. Just make sure the neighbor-pair logic matches what the tests below will assert (it does as of today; just keep it that way).

### Non-goals

- Do NOT change the frontend. Task `01KP63NNA6ME0GWXQZ3GHV4RZ7` owns that.
- Do NOT change the command registration, YAML definitions, or `DoThisNextCmd::available` predicate.
- Do NOT invent a new placement primitive — prefer setting `before_id` / `after_id` on the existing `MoveTask` op.

## Subtasks

- [ ] Grep for callers of `load_sorted_column_tasks`, `task_ordinal`, `ordinal_for_before`, `ordinal_for_after`, `compute_placement_ordinal`, and `compute_drop_ordinal` in `commands/task_commands.rs`. Note which survive after the refactor.
- [ ] Rewrite `DoThisNextCmd::execute` to set `before_id` on `MoveTask` and let the op compute the ordinal. Drop the pre-computation.
- [ ] Rewrite `MoveTaskCmd::execute`'s `before_id`/`after_id` branches in `drag_commands.rs` to set `before_id`/`after_id` on `MoveTask` instead of pre-computing. Decide whether `drop_index` stays in the command or migrates into `MoveTask` (document the decision in the commit message).
- [ ] Delete any helpers in `task_commands.rs` that become unused after the above two changes.
- [ ] Add the tests listed below; existing Rust tests for `DoThisNextCmd` and the drag flow must still pass unchanged.
- [ ] Manual check with the frontend task (`01KP63NNA6ME0GWXQZ3GHV4RZ7`) applied — rapidly invoke Do-This-Next on 3 tasks in different columns; all 3 end up at the top of column 0, last-clicked at position 0. No flicker.

## Acceptance Criteria

- [ ] `DoThisNextCmd::execute` contains no direct call to `compute_ordinal_for_neighbors` — the op computes the ordinal.
- [ ] `MoveTaskCmd::execute` (drag path) contains no direct call to `compute_ordinal_for_neighbors` for the `before_id` / `after_id` branches — the op computes those. Any remaining call is only for `drop_index` if that stays in the command, and is exercised by a test.
- [ ] Both "Do This Next on a middle-column task" and "drag a task to the first slot of the first column" produce *identical* resulting `position_ordinal` strings given the same starting board state. This is testable: run both paths on an identical seed board and assert the moved task ends up at the same `position_ordinal`.
- [ ] Rapidly invoking `DoThisNextCmd` N times in sequence (one per call, no stale client state) produces the N moved tasks at positions 0..N-1 of the first column, with the most-recent call at position 0 (each call recomputes "first" after the previous completed).
- [ ] The existing backend tests — `do_this_next_moves_to_first_column`, the drag-move tests, any tests under `-- do_this_next` — all still pass without modification.
- [ ] `cargo nextest run -p swissarmyhammer-kanban` green.
- [ ] `cargo clippy --all-targets -- -D warnings` clean.
- [ ] `cargo fmt --check` clean.

## Tests

- [ ] `swissarmyhammer-kanban/src/commands/task_commands.rs` (in the existing `mod tests`) — add `do_this_next_matches_drag_to_first_slot`: seed a board with 3 columns (order 0, 1, 2) and 3 tasks in column 1. On one copy of the state, run `DoThisNextCmd` on `tasks[1]`. On an identical copy, run the drag path (construct `MoveTask::to_column(tasks[1].id, col0).with_before(first_task_id_in_col0)`) with the same target. Assert the moved task's `position_ordinal` is identical in both copies.
- [ ] `swissarmyhammer-kanban/src/commands/task_commands.rs` — add `do_this_next_sequential_calls_keep_last_first`: seed a board with column 0 holding task `A` and column 1 holding tasks `B, C, D`. Call `DoThisNextCmd` on B, then C, then D. Assert column 0's sorted order is `D, C, B, A` — last call wins position 0.
- [ ] `swissarmyhammer-kanban/src/commands/drag_commands.rs` — if `MoveTaskCmd` logic changes, add (or update) a test `drag_before_first_matches_do_this_next` that runs the drag command with `before_id = first_task` and verifies parity with the DoThisNext result on the same input.
- [ ] Test command: `cargo nextest run -p swissarmyhammer-kanban -- do_this_next drag_move` — all green.
- [ ] Full crate check: `cargo nextest run -p swissarmyhammer-kanban && cargo clippy -p swissarmyhammer-kanban --all-targets -- -D warnings` — clean.

## Workflow

- Use `/tdd` — write the `do_this_next_matches_drag_to_first_slot` test first (it will fail against today's pre-computed-ordinal path if the two paths diverge at all, and gives you the canary for the refactor), then collapse the duplication until it passes. Run the whole `-p swissarmyhammer-kanban` suite after each change.

## Depends on

- None. This task and `01KP63NNA6ME0GWXQZ3GHV4RZ7` (frontend cleanup) are independent — each is a complete improvement on its own. Running both closes the loop on the flakiness the user is seeing. #drag-and-drop