---
title: are_todos_done
description: "Check if all todo items are complete."
---

## Goal

We want to know if there are any pending todo items.

Use the todo_show MCP tool with `item: "next"` to check for pending todos

If there are any pending todos, use the `cel_set` tool to set are_todos_done to `false`
If there are no pending todos, use the `cel_set` tool to set are_todos_done to `true`
