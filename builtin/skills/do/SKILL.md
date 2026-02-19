---
name: do
description: Execute the next task from the kanban board. Use when the user wants to make progress on planned work by implementing the next available todo item.
allowed-tools: "*"
metadata:
  author: swissarmyhammer
  version: "1.0"
---

# Do

Pick up and execute the next task from the kanban board.

## How to Execute

Use the `flow` tool to run the do workflow:

    flow_name: "do"

## What Happens

1. Queries the kanban board for the next unassigned task
2. Assigns the task and moves it to "doing"
3. Implements the task following the plan
4. Runs tests to verify the implementation
5. Marks the task as complete when done
