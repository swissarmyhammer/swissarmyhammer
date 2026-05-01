---
assignees:
- claude-code
due: 2026-06-13
position_column: done
position_ordinal: ffffffffffffffffffffffffffffff9680
title: Fix DoThisNextCmd column sort — get_str on numeric order field picks wrong column
---
## What

`DoThisNextCmd` in `swissarmyhammer-kanban/src/commands/task_commands.rs:314-319` sorts columns with `get_str(\"order\")`, but the `order` field is a JSON number, not a string. `get_str` returns `None` for numbers, so every column falls back to `\"0\"`, making the sort arbitrary. The command picks whichever column happens to come first from `ectx.list()` instead of the actual first (todo) column.

### The bug (line 315-318)

```rust
sorted_columns.sort_by(|a, b| {
    let oa = a.get_str(\"order\").unwrap_or(\"0\");  // always \"0\" — order is a number!
    let ob = b.get_str(\"order\").unwrap_or(\"0\");
    oa.cmp(ob)
});
```

### The fix — match every other column sort in the codebase

`column_commands.rs:45`, `board/get.rs:55`, `column/list.rs:25`, and `kanban-app/src/commands.rs` all use:

```rust
columns.sort_by_key(|c| c.get(\"order\").and_then(|v| v.as_u64()).unwrap_or(0));
```

### Subtasks

- [x] Change column sort in `DoThisNextCmd::execute` (`task_commands.rs:314-319`) from `get_str(\"order\")` string comparison to `get(\"order\").and_then(|v| v.as_u64()).unwrap_or(0)` numeric sort
- [x] Add test: board with 3 columns (order 0, 1, 2), run DoThisNext on a task in column 1, verify it moves to column 0 at the top position

## Acceptance Criteria

- [x] `task.doThisNext` moves the target task to the first column (lowest `order` value), not an arbitrary column
- [x] Task is placed before all existing tasks in that column (ordinal sorts first)
- [x] Existing DoThisNext behavior is preserved for boards where the first column is correct

## Tests

- [x] `swissarmyhammer-kanban/src/commands/task_commands.rs` — add test `do_this_next_moves_to_first_column`: create board with columns at order 0/1/2, create task in column at order 1, run DoThisNext, assert `position_column` is the order-0 column and `position_ordinal` sorts before any existing task in that column
- [x] `cargo nextest run -p swissarmyhammer-kanban -- do_this_next` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Review Findings (2026-04-12 18:15)

### Warnings
- [x] `swissarmyhammer-kanban/src/commands/task_commands.rs` (test `do_this_next_moves_to_first_column`) — The new test relies on an unstated assumption that `fs::read_dir` returns column entity files in creation order (`todo, doing, done`). `read_entity_dir` in `swissarmyhammer-entity/src/io.rs` does NOT sort its output, so the regression assertion is filesystem-dependent. Empirically it works on macOS APFS, but on a filesystem that happened to return entries alphabetically, the buggy pre-fix code would also pick `doing` (comes before `todo` alphabetically) and the test would silently pass against the bug. Strengthen by also creating a third column `aaa` with order 5 (or similar) — then regardless of iteration order, the bug's stable-sort-on-equal-keys behavior would land somewhere other than the intended order-0 column. Alternatively, add an inline assertion-sanity comment explaining the fs-ordering dependency. This isn't a correctness problem for the committed code — only for the test's ability to catch future regressions of the same class.

### Nits
- [x] `swissarmyhammer-kanban/src/commands/task_commands.rs` (`compute_placement_ordinal`) — The `else` branch is unreachable by construction (`MoveTaskCmd::execute` only calls this when `before_id.is_some() || after_id.is_some()`). The comment "Neither — shouldn't happen, append at end" acknowledges this. Consider either (a) making the function signature take a stricter type that encodes "at least one of before/after is set" (e.g., `enum Placement { Before(&str), After(&str) }`) or (b) replacing the fallback with `unreachable!(\"compute_placement_ordinal called with neither before_id nor after_id\")` to surface an invariant violation rather than silently producing a potentially wrong ordinal. Option (a) is the dtolnay-school approach; option (b) is the lightweight fix.
- [x] `swissarmyhammer-kanban/src/commands/task_commands.rs` (`resolve_move_task_args`) — Error mapping preserves the pre-existing quirk where a missing `id` arg (not scope) still surfaces as `MissingScope(\"task\")` rather than `MissingArg(\"id\")`. Pre-existing, not introduced by this refactor, so not blocking. Worth noting for a future cleanup pass — since `MoveTaskCmd::available` treats either source as valid, the error should probably say "task id required (via `task` scope or `id` arg)".
- [x] `swissarmyhammer-kanban/src/commands/task_commands.rs` (`resolve_move_task_args`) — Returns `(String, String)` where `(&str, &str)` would suffice if the function were generic over the context lifetime. The `.to_string()` allocations are essentially free at human-invocation frequency and the owned-string form sidesteps lifetime plumbing, so the trade is fine — flagging only as a style note.
- [x] `swissarmyhammer-kanban/src/commands/task_commands.rs` (`first_column_id`) — Docstring says "lowest-`order` column" which is correct. However, when two columns share the same `order` (e.g., both 0 after a manual edit), the result depends on sort stability and filesystem iteration order. Consider either documenting the tiebreaker behavior or adding a secondary sort key by column id for determinism. Low priority since the code path assumes well-formed orders.

Counts: 0 blockers, 1 warning, 4 nits.