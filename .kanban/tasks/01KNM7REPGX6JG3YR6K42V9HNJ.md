---
assignees:
- claude-code
depends_on:
- 01KNM7QS0FJ0JNRYJD3NG3BWHC
position_column: todo
position_ordinal: '9780'
title: 'MCP/CLI: expose user-set date fields in task operations'
---
## What

Update the MCP kanban tool and CLI commands to support the new date fields:

1. **`add task`** — accept optional `due` and `scheduled` parameters (ISO 8601 date strings). Set them as entity fields before write.

2. **`update task`** — accept optional `due` and `scheduled` parameters. Allow clearing with null/empty string.

3. **`get task`** / **`list tasks`** — include all date fields in task output. User-set dates (due, scheduled) come from stored fields. System dates (created, updated, started, completed) come from computed field derivation (already happens via the enrichment pipeline on read).

4. System dates are read-only — reject attempts to set created/updated/started/completed via add/update.

**Files to modify:**
- `swissarmyhammer-kanban/src/commands/task_commands.rs` — add date params to AddTask/UpdateTask structs; include dates in task JSON output
- `swissarmyhammer-kanban/src/task/add.rs` — wire `due`/`scheduled` params through to entity.set()
- Update task command similarly

## Acceptance Criteria
- [ ] `add task` accepts `due` and `scheduled` ISO 8601 date strings
- [ ] `update task` accepts `due` and `scheduled`, including clearing them
- [ ] `get task` returns all date fields (user-set + system-derived) when present
- [ ] `list tasks` returns date fields on each task
- [ ] Invalid date strings are rejected with clear error
- [ ] System dates cannot be set via MCP/CLI params

## Tests
- [ ] Integration test: add task with due date → get task → verify due date returned
- [ ] Integration test: update task to set/clear scheduled date
- [ ] Integration test: create task → get task → verify created/updated computed dates appear
- [ ] Integration test: move task through columns → verify started/completed appear
- [ ] `cargo test -p swissarmyhammer-kanban` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement.

#task-dates