---
assignees:
- claude-code
due: 2026-04-17
position_column: review
position_ordinal: '80'
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
- Use `/tdd` — write failing tests first, then implement to make them pass.</description>
</invoke>