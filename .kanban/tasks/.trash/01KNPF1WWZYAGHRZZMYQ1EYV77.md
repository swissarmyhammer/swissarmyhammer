---
assignees:
- claude-code
position_column: todo
position_ordinal: a980
title: Make task.add default to lowest-order column when no column specified
---
## What

`AddTaskCmd` in `swissarmyhammer-kanban/src/commands/task_commands.rs` currently requires a column in scope or args:

```rust
fn available(&self, ctx: &CommandContext) -> bool {
    ctx.has_in_scope("column") || ctx.arg("column").and_then(|v| v.as_str()).is_some()
}
```

And `execute()` errors with `MissingScope("column")` if neither is provided. But the underlying `AddTask` operation in `swissarmyhammer-kanban/src/task/add.rs` already has `resolve_column()` which finds the lowest-order column as a fallback:

```rust
None => {
    let columns = ectx.list("column").await?;
    columns.iter().min_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0))
}
```

This fallback never fires because `AddTaskCmd` always sets `op.column = Some(...)`. The fix: make column optional in the command layer so the operation's default-to-lowest-order-column logic handles it.

**Files to modify:**
- `swissarmyhammer-kanban/src/commands/task_commands.rs` — `AddTaskCmd`
  1. Change `available()` to always return `true` (column is no longer required — the operation resolves a default)
  2. Change `execute()` to set `op.column` only when a column IS available (from scope or args), otherwise leave it `None` so `resolve_column()` picks the first column

**Also update:**
- `swissarmyhammer-kanban/builtin/entities/task.yaml` — change `task.add` scope from `scope: "entity:column"` to no scope (or a broader scope), so the command appears in contexts without a column in scope

## Acceptance Criteria
- [ ] `task.add` dispatched without a column arg or column in scope creates a task in the lowest-order column (order=0, typically "todo")
- [ ] `task.add` with an explicit column arg still places the task in that column
- [ ] `task.add` with a column in scope (board view) still places the task in that column
- [ ] Grid view `grid.newBelow` on a tasks grid successfully creates a task (no longer fails due to missing column)

## Tests
- [ ] Add test in `task_commands.rs` or `task/add.rs` — `task.add` without column defaults to lowest-order column
- [ ] Existing `task.add` tests still pass (column from scope and args paths)
- [ ] Run: `cargo test -p swissarmyhammer-kanban` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.