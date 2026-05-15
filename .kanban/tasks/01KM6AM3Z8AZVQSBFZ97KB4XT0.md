---
assignees:
- claude-code
depends_on:
- 01KM6AKQV2WB1X0H8H27Z6ZE0W
position_column: done
position_ordinal: ffffffffffff9a80
title: Wire archive/unarchive/list-archived dispatch in MCP kanban tool
---
## What

Add dispatch match arms so that `\"archive task\"`, `\"unarchive task\"`, and `\"list archived\"` route to the correct operations through the MCP kanban tool.

### Changes

In `swissarmyhammer-kanban/src/dispatch.rs`:
- `(Verb::Archive, Noun::Task)` → `ArchiveTask::new(id)`
- `(Verb::Unarchive, Noun::Task)` → `UnarchiveTask::new(id)`
- `(Verb::List, Noun::Archived)` → `ListArchived` (no params, or optional entity_type filter)

### Files
- `swissarmyhammer-kanban/src/dispatch.rs` — add three match arms

## Acceptance Criteria
- [ ] `parse_input({\"op\": \"archive task\", \"id\": \"...\"})` + `execute_operation` archives the task
- [ ] `parse_input({\"op\": \"unarchive task\", \"id\": \"...\"})` + `execute_operation` unarchives the task
- [ ] `parse_input({\"op\": \"list archived\"})` + `execute_operation` returns archived tasks
- [ ] After MCP restart, the tool description includes archive operations

## Tests
- [ ] `dispatch_archive_task` — end-to-end parse + dispatch
- [ ] `dispatch_unarchive_task` — end-to-end parse + dispatch
- [ ] `dispatch_list_archived` — end-to-end parse + dispatch
- [ ] `cargo test -p swissarmyhammer-kanban`