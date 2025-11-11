---
title: do_todo
description: Complete the next pending todo item.
---

## Goals

The goal is to:

- Complete the next todo item in the list
- Follow the instructions in the task and context
- Mark the todo as complete when done

## Rules

- NEVER skip todos
- Always read the task and context carefully
- Complete only the specific work described in the todo
- Use todo_mark_complete after successfully finishing the work
- If the todo cannot be completed, document why and ask for help
- DO NOT commit to git

## Process

- Use todo_show with `item: "next"` to get the next pending todo
- Read the task description and context to understand what needs to be done
- Perform the work described in the todo
- Verify the work is complete and correct
- Use todo_mark_complete with the todo's id to mark it as done
- Report what was accomplished

## Reporting

Show progress as:

✅ Completed todo: <task description>
