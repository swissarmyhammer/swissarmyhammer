---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9180
title: 'WARNING: Cascade events can produce O(N*M) event flood'
---
kanban-app/src/commands.rs:1539-1597\n\ncascade_aggregate_events() re-reads ALL entities of every dependent type and emits an EntityFieldChanged for each one. If a single task is modified and columns depend on tasks (for task_count), this emits one EntityFieldChanged per column. For a board with 10 columns, that is 10 extra events per task mutation.\n\nMore concerning: if the cascade produces events for entity type A, and type B depends on type A, the current code does NOT cascade again (it only runs once). This is probably intentional, but there is no comment explaining that cascades are intentionally single-level.\n\nThe real performance issue is that this reads ALL entities of each dependent type from disk via ectx.list(), even when only one entity in the triggering type changed. For large boards (hundreds of tasks), a single column reorder could trigger re-reading all columns + all tasks from disk just to produce cascade events.\n\nSuggestion: Consider batching cascade events per entity type (one event with all column IDs) rather than per entity. Add a comment documenting the intentional single-level cascade. Long-term, consider only re-reading the specific entities whose aggregate fields actually changed.",
<parameter name="tags">["review-finding"] #review-finding