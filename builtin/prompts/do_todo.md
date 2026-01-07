---
title: do_todo
description: Complete the next pending todo item.
---

## Goals

The goal is to:

- Complete the next todo item in the list
- Follow the instructions in the task and context
- Mark the todo as complete when done

Use the todo_show MCP tool with `item: "next"`. This will get the next pending todo item.

If there are any pending todos
- use the `cel_set` tool to set name `are_todos_done` to value `false`
- exeute the TODO Process below

If there are no pending todos
- use the `cel_set` tool to set name `are_todos_done` to value `true`
- you are done, report "No pending todos, all work complete!"

## Rules

- NEVER skip todos
- DO NOT commit to git
- DO NOT run a `rules_check` except on individual files you have modified as part of the todo

## TODO Process

- Use `todo_show` with `item: "next"` to get the next pending todo
- Read the task description to understand what needs to be done
- Perform the work described in the todo
- Verify the work is complete and correct
- Use `todo_mark_complete` with the todo's id to mark it as done
- Report what was accomplished
