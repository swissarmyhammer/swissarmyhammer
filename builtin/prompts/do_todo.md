---
title: do_todo
description: Complete the next pending task from the kanban board.
---

## Goals

The goal is to:

- Complete the next task from the todo column
- Follow the instructions in the task title and description
- Mark the task as complete when done

Use the kanban MCP tool with `op: "next task"`. This will get the next pending task from the todo column.

If there are any pending tasks
- use the `js` tool with `op: "set expression"`, `name: "are_todos_done"`, `expression: "false"`
- execute the Task Process below

If there are no pending tasks
- use the `js` tool with `op: "set expression"`, `name: "are_todos_done"`, `expression: "true"`
- you are done, report "No pending tasks, all work complete!"

## Rules

- NEVER skip tasks
- DO NOT commit to git
- DO NOT run a `rules_check` except on individual files you have modified as part of the task

## Task Process

- Use `kanban` with `op: "next task"` to get the next pending task
- Read the task title and description to understand what needs to be done
- Perform the work described in the task
- Verify the work is complete and correct
- Use `kanban` with `op: "complete task"` and the task's `id` to mark it as done
  - DO NOT set `are_todos_done` via `js` tool -- there might be more tasks remaining that have been created dynamically
- Report your progress
