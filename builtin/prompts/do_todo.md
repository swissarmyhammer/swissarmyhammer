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
- DO NOT commit to git

## Process

- Use `todo_show` with `item: "next"` to get the next pending todo
- Read the task description to understand what needs to be done
- Use `git_changes` to see what has changed in the codebase
- Perform the work described in the todo
- Verify the work is complete and correct
- Use todo_mark_complete with the todo's id to mark it as done
- Report what was accomplished
