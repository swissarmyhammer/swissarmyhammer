---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '9680'
title: Inject Started and Completed dates on column transitions
---
## What

When a task's `position_column` changes, detect transitions to/from terminal columns and set Started/Completed dates accordingly.

**Started** — set the *first* time `position_column` transitions to `doing` (or any non-first, non-terminal column). Once set, never overwritten, even if the task leaves and re-enters doing. This is the \"work began\" timestamp.

**Completed** — set the *last* time `position_column` transitions to the terminal column (highest-order column, currently `done`). If a task is reopened (moved out of done) and completed again, Completed updates to the new timestamp. If moved out of done, Completed clears.

**State machine:**
```
todo → doing         Started = now (first time only)
doing → done         Completed = now
done → doing         Completed clears, Started unchanged
doing → done         Completed = now (re-set)
```

**Implementation approach:**

In `EntityContext::write()`, after reading `previous` but before the store handle write:

1. If entity type has `started`/`completed` fields AND `position_column` changed:
2. Compare `previous.fields[\"position_column\"]` vs new `entity.fields[\"position_column\"]`
3. Determine column ordering from the entity definitions (need to resolve which column is terminal)
4. Apply the state machine rules above

**Challenge:** EntityContext doesn't currently know column ordering. Options:
- Pass column metadata to write() — too invasive
- Use a convention: terminal column = `done` (matches current setup)
- Better: add a method to check if a column is terminal by reading column entities

The simplest correct approach: in the kanban crate (which knows about columns), hook into task move/complete operations to set Started/Completed *before* calling `ectx.write()`. This keeps EntityContext generic and puts domain logic where it belongs.

**Files to modify:**
- `swissarmyhammer-kanban/src/task/mv.rs` — set Started on move to non-todo/non-done column; clear Completed on move out of done
- `swissarmyhammer-kanban/src/task/complete.rs` — set Completed on complete
- `swissarmyhammer-kanban/src/task/add.rs` — ensure Started/Completed are NOT set on create

## Acceptance Criteria
- [ ] First move to `doing` sets `started` to current UTC timestamp
- [ ] Subsequent moves to `doing` do NOT overwrite `started`
- [ ] Move to terminal column sets `completed` to current UTC timestamp
- [ ] Move out of terminal column clears `completed`
- [ ] Re-completing a task updates `completed` to new timestamp
- [ ] Creating a task does NOT set `started` or `completed`

## Tests
- [ ] Integration test: create task → move to doing → verify started set
- [ ] Integration test: move to doing → move to done → verify completed set, started unchanged
- [ ] Integration test: move to done → move back to doing → verify completed cleared, started unchanged
- [ ] Integration test: move to doing → done → doing → done → verify started is first timestamp, completed is last
- [ ] `cargo test -p swissarmyhammer-kanban` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement.

#task-dates