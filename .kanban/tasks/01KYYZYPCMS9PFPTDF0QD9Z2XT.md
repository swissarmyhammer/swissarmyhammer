---
assignees:
- claude-code
position_column: todo
position_ordinal: dc80
title: list tasks silently ignores the documented `assignee` and `exclude_done` params
---
Found while auditing `dispatch.rs` for silent-drop params on ^n36mc1q.

`crates/swissarmyhammer-kanban/src/schema.rs` advertises this example:

```
{"op": "list tasks", "assignee": "alice", "exclude_done": true}
```

The `list tasks` arm of `execute_task_query_operation`
(crates/swissarmyhammer-kanban/src/dispatch.rs) reads only `column`, `filter`,
`project`, `page`, `page_size`, and `detail`. It never reads `assignee`, and
`exclude_done` appears nowhere else in the workspace. Both params are dropped
and the caller gets an `ok` with an unfiltered page.

## Evidence

- `exclude_done` matches exactly one line in `crates/` — the schema example
  itself. No handler exists.
- The existing test `dispatch_list_tasks_with_assignee_filter`
  (dispatch.rs) does NOT prove the filter works: the board holds one task, so
  `count == 1` passes whether or not `assignee` filters anything. Add a second,
  unassigned task and the test would still pass while the filter does nothing.

## Options

1. Fold `assignee` into the filter DSL the way `project` is folded
   (`@<assignee>`), and add `exclude_done` as a filter atom or drop it from the
   example.
2. Drop both keys from the schema example so the docs match the code.

Option 1 matches what the example promises. Option 2 is honest but removes a
capability an agent reading the schema believes it has.

## Acceptance

- A `list tasks` call with `assignee` filters the page. Test must fail first,
  with at least two tasks on the board and only one assigned.
- `exclude_done` either filters or is removed from `schema.rs`.
- `dispatch_list_tasks_with_assignee_filter` is strengthened so it can fail: a
  second, unassigned task on the board. #bug #kanban