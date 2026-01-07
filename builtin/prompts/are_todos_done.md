---
title: are_todos_done
description: "Check if all todo items have been completed."
---

## Goal

We want to know if all todo items are complete.

Use the todo_list MCP tool with `completed: false` to check for incomplete todos.

If there are any incomplete todos, `cel_set` are_todos_done to `false`
If all todos are complete (no incomplete todos returned), `cel_set` are_todos_done to `true`
