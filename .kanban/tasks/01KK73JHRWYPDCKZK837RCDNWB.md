---
position_column: done
position_ordinal: ffffed80
title: 'Bug: New task from + button goes to done column instead of clicked column'
---
When clicking the + button on a column to add a new task, the task is created in the done column instead of the column where + was clicked. Additionally, only the 'todo' column should have the + button — it doesn't make sense to start cards in other columns.

Two fixes needed:
1. Fix task creation to respect the target column
2. Only show the + button on the first (todo) column

Key files: board-view.tsx (handleAddTask), column-view.tsx (+ button rendering), Rust dispatch for task.add