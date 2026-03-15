---
position_column: todo
position_ordinal: a0
title: Add query field to ListTasks operation
---
## What
Add `query: Option<String>` to the `ListTasks` struct in `swissarmyhammer-kanban/src/task/list.rs`. Add a `with_query()` builder method. In the `execute()` filter chain, add a query predicate that does case-insensitive substring matching against title, body, description, tag names (via `task_tags()`), and entity ID.

## Acceptance Criteria
- [ ] `ListTasks` has an optional `query` field
- [ ] `with_query()` builder method exists
- [ ] When query is set, only tasks matching the substring (case-insensitive) in title, body, description, tags, or ID are returned
- [ ] When query is None, behavior is unchanged (all existing tests pass)
- [ ] Query filter composes with all existing filters (column, tag, swimlane, assignee, ready)

## Tests
- [ ] `test_list_tasks_by_query` — add tasks with different titles, search by substring, verify only matches returned
- [ ] `test_list_tasks_query_matches_body` — task with matching markdown body is found
- [ ] `test_list_tasks_query_case_insensitive` — "AUTH" matches "authentication"
- [ ] `test_list_tasks_query_with_column_filter` — query + column filter compose correctly
- [ ] All existing ListTasks tests still pass: `cargo nextest run -p swissarmyhammer-kanban list`