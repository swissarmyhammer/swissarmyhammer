---
name: implement
description: Implementation workflow. Use this skill whenever you are implementing, coding, or building. Picks up one kanban card and delegates the work to an implementer subagent. Consider delegating to an implementation-focused agent to keep the main context clean.
metadata:
  author: swissarmyhammer
  version: "2.0"
---

# Implement

Pick up the next kanban card and get it done.

## Process

### 1. Get the next card

Use `kanban` with `op: "next task"` to find the next actionable card. If there are no remaining cards, tell the user the board is clear.

### 2. Move the card to doing

Use `kanban` with `op: "move task"`, `id: "<task-id>"`, `column: "doing"`

### 3. Read the card

Use `kanban` with `op: "get task"`, `id: "<task-id>"` to get the full description and subtasks.

### 4. Delegate to a subagent

Spawn an **implementer** subagent to do the actual work. Pass it the card details — title, description, subtasks, and any relevant context about the codebase. The subagent does the implementation, runs tests, and reports back.

This keeps verbose implementation output (compiler errors, test output, file reads) in the subagent's context instead of cluttering yours.

### 5. Complete the card

When the subagent reports success, complete the card: `kanban` with `op: "complete task"`, `id: "<task-id>"`. A card left in "doing" is not finished.

If the subagent reports failure or partial progress, do NOT complete the card. Add a comment describing what happened and tell the user.

### 6. Stop for review

**Always stop after completing a card.** Present the user with a summary of what was done and what the subagent reported. The user decides when to move to the next card — you do not auto-continue.

Only exception: if the card description explicitly says **auto-continue** or **chain to next**, proceed to the next card without stopping.

## Guidelines

- One card, one subagent. Don't try to do multiple cards in a single agent.
- The subagent does the work. You are the dispatcher — get the card, delegate, complete, report.
- Do NOT use TodoWrite, TaskCreate, or any other task tracking — the kanban board is the single source of truth.
- If you discover new work while reviewing the subagent's output, add it as a new kanban card.
