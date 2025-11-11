---
title: are_todos_done
description: "Check if all todo items are complete."
---

## Goal

We want to know if there are any pending todo items.

## Rules

If there are any pending todos, respond only with NO
If there are no pending todos, respond only with YES

## Process

- Use the todo_show MCP tool with `item: "next"` to check for pending todos
- If the tool returns a todo item, there are pending todos - respond with NO
- If the tool returns no todos or an error indicating no pending items, respond with YES
- Respond with only YES or NO
