---
assignees:
- claude-code
depends_on:
- 01KNJD76746DTJ540A5SHGRCF0
position_column: todo
position_ordinal: 8f80
title: 'FILTER-7: Replace individual filter fields with DSL param on ListTasks + NextTask'
---
## What

Replace the individual `tag`, `assignee`, and `ready` filter fields on `ListTasks` and `NextTask` with a single `filter: Option<String>` DSL field. Keep the `column` field on `ListTasks` (it's structural — controls done-exclusion, not a predicate). Keep `NextTask` as an operation but give it the DSL `filter` param instead of `tag`/`assignee`.

### ListTasks changes
**Remove**: `tag`, `assignee`, `ready` fields and their builder methods
**Keep**: `column` (structural)
**Add**: `filter: Option<String>`

Old: `{"op": "list tasks", "tag": "bug", "assignee": "alice", "ready": true}`
New: `{"op": "list tasks", "filter": "#bug && @alice && #READY"}`

### NextTask changes
**Remove**: `tag`, `assignee` fields and their builder methods
**Add**: `filter: Option<String>`

Old: `{"op": "next task", "tag": "bug"}`
New: `{"op": "next task", "filter": "#bug"}`

`NextTask` still returns the oldest ready task matching the filter. The `#READY` virtual tag is implicit in `next task` (it already only returns ready tasks), but users can combine it with other predicates.

### Files to modify
- `swissarmyhammer-kanban/Cargo.toml` — add dep on `swissarmyhammer-filter-expr` (may already exist from FILTER-1)
- `swissarmyhammer-kanban/src/task/list.rs` — remove `tag`, `assignee`, `ready` fields; add `filter: Option<String>`; update `execute()` to parse + evaluate DSL
- `swissarmyhammer-kanban/src/task/next.rs` — remove `tag`, `assignee` fields; add `filter: Option<String>`; update `execute()`
- Update all callers of `ListTasks::with_tag()`, `with_assignee()`, `with_ready()`, `NextTask::with_tag()`, `with_assignee()` — these builder methods go away

### What happens downstream
- MCP tool schemas update automatically (operation macro derives them)
- CLI flags update automatically (derived from operation metadata)
- No changes needed in `swissarmyhammer-tools/` or `swissarmyhammer-cli/`

## Acceptance Criteria
- [ ] `ListTasks { filter: Some("#bug"), ..Default::default() }` returns only bug-tagged tasks
- [ ] `ListTasks { filter: Some("#bug && @alice"), ..Default::default() }` applies boolean logic
- [ ] `ListTasks { filter: None, ..Default::default() }` returns all non-done tasks (unchanged)
- [ ] `ListTasks { column: Some("done"), filter: Some("#bug"), ..Default::default() }` filters within done column
- [ ] `NextTask { filter: Some("#bug") }` returns oldest ready bug task
- [ ] `NextTask { filter: None }` returns oldest ready task (unchanged)
- [ ] `tag`, `assignee`, `ready` fields no longer exist on `ListTasks`
- [ ] `tag`, `assignee` fields no longer exist on `NextTask`
- [ ] MCP tool schema shows `filter` param, not the old individual fields
- [ ] `cargo test -p swissarmyhammer-kanban` passes
- [ ] No compilation errors from removed builder methods

## Tests
- [ ] `swissarmyhammer-kanban/src/task/list.rs` — update all existing tests to use DSL filter
- [ ] `swissarmyhammer-kanban/src/task/next.rs` — update existing tests to use DSL filter
- [ ] New test: DSL filter with virtual tags on ListTasks
- [ ] New test: DSL filter on NextTask
- [ ] Verify old builder methods (`with_tag`, `with_assignee`, `with_ready`) are removed

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.