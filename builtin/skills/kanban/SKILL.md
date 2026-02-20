---
name: kanban
description: Execute the next task from the kanban board. Use when the user wants to make progress on planned work by implementing the next available todo item.
allowed-tools: "*"
metadata:
  author: swissarmyhammer
  version: "1.0"
---

# Do

Pick up and execute the next task from the kanban board.


## Process

1. Get the next task: use `kanban` with `op: "next task"` to find the next actionable card
2. Move it to doing: use `kanban` with `op: "move task"`, `id: "<task-id>"`, `column: "doing"`
3. Read the task details: use `kanban` with `op: "get task"`, `id: "<task-id>"` to see description and subtasks
4. Work through each subtask:
   - Implement what the subtask describes
   - Mark it complete: use `kanban` with `op: "complete subtask"`, `task_id: "<task-id>"`, `id: "<subtask-id>"`
5. When ALL subtasks are done, complete the card: use `kanban` with `op: "complete task"`, `id: "<task-id>"`

## Guidelines

- Each kanban card can have subtasks -- you need to do all of these subtasks to complete the card
- Do not skip subtasks or mark them complete without actually doing the work
- If a subtask is blocked or unclear, add a comment to the task explaining the issue
- Run tests after completing each subtask to catch problems early
- Only mark the card as complete when every subtask is done and tests pass
