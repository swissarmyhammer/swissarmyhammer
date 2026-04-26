---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffba80
title: Add before/after placement to MCP kanban move task operation
---
## What
The MCP kanban tool's `move task` operation only exposes `ordinal` for positioning. The underlying `task.move` command already supports `before_id` and `after_id` args for relative placement (computed ordinal from neighbors), but the MCP layer doesn't pass them through.

This makes it impossible to say "move this card before that card" — you can only set an explicit ordinal, which requires knowing the ordinal space.

**Files:**
- The MCP kanban tool's move task handler — needs to accept `before_id` and `after_id` parameters and pass them through as args to `task.move`

**The backend already handles it** — `swissarmyhammer-kanban/src/commands/task_commands.rs` lines 91-99:
```rust
} else if ctx.arg("before_id").is_some() || ctx.arg("after_id").is_some() {
    let before_id = ctx.arg("before_id").and_then(|v| v.as_str());
    let after_id = ctx.arg("after_id").and_then(|v| v.as_str());
```

Just needs the MCP layer to expose and forward these params.

## Acceptance Criteria
- [ ] `move task` accepts optional `before_id` parameter — places task before the referenced task
- [ ] `move task` accepts optional `after_id` parameter — places task after the referenced task
- [ ] `before_id`/`after_id` take precedence over `ordinal` when both are provided
- [ ] Existing `ordinal` parameter still works for explicit positioning

## Tests
- [ ] Move a task with `before_id` set to another task's ID — verify it lands directly before that task
- [ ] Move a task with `after_id` set to another task's ID — verify it lands directly after that task
- [ ] Move a task with only `column` (no placement) — still appends at end as before