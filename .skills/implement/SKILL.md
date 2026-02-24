---
name: implement
description: Implementation workflow. Use this skill whenever you are implementing, coding, or building. The kanban board is your todo list — pick up the next card and do the work.
metadata:
  author: "swissarmyhammer"
  version: "1.1"
---

# Implement

Work through all remaining kanban cards until the board is clear.

## Process

Repeat this loop until there are no more cards to pick up:

### 1. Get the next card

Use `kanban` with `op: "next task"` to find the next actionable card. If there are no remaining cards, you're done — stop and tell the user.

### 2. Move the card to doing

Use `kanban` with `op: "move task"`, `id: "<task-id>"`, `column: "doing"`

### 3. Read the task details

Use `kanban` with `op: "get task"`, `id: "<task-id>"` to see the description and subtasks.

### 4. Work through each subtask

For each subtask:
- Implement what the subtask describes
- Mark it complete: use `kanban` with `op: "complete subtask"`, `task_id: "<task-id>"`, `id: "<subtask-id>"`

### 5. Complete the card

**You MUST complete the card before moving on.** When ALL subtasks are done, use `kanban` with `op: "complete task"`, `id: "<task-id>"`. Do NOT skip this step. A card left in "doing" is not finished — it must be explicitly completed.

### 6. Loop back

After completing the card, go back to step 1. Use `op: "next task"` again — if it returns a card, keep going. If there are no more actionable cards, stop and tell the user.

Use the `js` tool to record the overall result
- If NO kanban tasks remain: `js` with `op: "set expression"`, `name: "kanban_empty"`, `expression: "true"`
- If ANY kanban tasks remain: `js` with `op: "set expression"`, `name: "kanban_empty"`, `expression: "false"`

## Guidelines

- Do not skip subtasks or mark them complete without doing the work
- Run tests after completing each subtask to catch problems early
- If a subtask is blocked or unclear, add a comment to the task and move on to the next card
- If you discover new work while executing a task, add it as a new kanban card
- Only mark a card complete when every subtask is done and tests pass
- Do NOT use TodoWrite, TaskCreate, or any other task tracking — the kanban board is the single source of truth
