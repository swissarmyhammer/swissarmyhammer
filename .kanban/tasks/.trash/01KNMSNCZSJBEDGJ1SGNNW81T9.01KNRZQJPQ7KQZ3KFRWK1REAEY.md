---
assignees:
- claude-code
position_column: todo
position_ordinal: a680
title: 'Fix failing test: project::delete::tests::test_delete_project_with_tasks_fails'
---
Test at `swissarmyhammer-kanban/src/project/delete.rs:177` fails: `assertion failed: matches!(result, Err(KanbanError::ProjectHasTasks { .. }))`. The delete operation is not returning the expected `ProjectHasTasks` error variant when a project with tasks is deleted. #test-failure