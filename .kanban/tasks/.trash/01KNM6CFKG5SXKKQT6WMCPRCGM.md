---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '9780'
title: 'MCP/CLI: expose date fields in task add, move, and list operations'
---
## What

Update the MCP kanban tool and CLI commands to support date fields:

1. **`add task`** — accept optional `due` and `scheduled` parameters (ISO 8601 date strings). Set them as entity fields before write.

2. **`update task`** — accept optional `due` and `scheduled` parameters. Allow clearing with null/empty.

3. **`get task`** / **`list tasks`** — include all date fields (due, scheduled, created, updated, started, completed) in task output. System dates are read-only but visible.

4. **`list tasks`** — support sorting by date fields (e.g., sort by due date).

**Files to modify:**
- `swissarmyhammer-kanban/src/commands/task_commands.rs` — add date params to AddTask, UpdateTask structs; include dates in task output
- Possibly `swissarmyhammer-kanban/src/task/add.rs` and update.rs — wire date params through

## Acceptance Criteria
- [ ] `add task` accepts `due` and `scheduled` date strings
- [ ] `update task` accepts `due` and `scheduled`, including clearing them
- [ ] `get task` returns all 5 date fields when present
- [ ] `list tasks` returns date fields on each task
- [ ] System dates (created, updated, started, completed) cannot be set via MCP/CLI
- [ ] Invalid date strings are rejected with a clear error

## Tests
- [ ] Integration test: add task with due date → get task → verify due date returned
- [ ] Integration test: update task to set/clear scheduled date
- [ ] Integration test: verify system dates appear in get task output after create/move
- [ ] `cargo test -p swissarmyhammer-kanban` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement.

#task-dates