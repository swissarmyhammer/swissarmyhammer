---
title: are_todos_done
description: "Check if all tasks on the kanban board have been completed."
---

## Goal

We want to know if all tasks are complete (in the done column).

Use the kanban MCP tool with `op: "list tasks"` to get all tasks.

Check if any tasks are NOT in the "done" column.

If there are tasks not in the done column, use `js` with `op: "set expression"`, `name: "are_todos_done"`, `expression: "false"`
If all tasks are in the done column (or there are no tasks), use `js` with `op: "set expression"`, `name: "are_todos_done"`, `expression: "true"`
