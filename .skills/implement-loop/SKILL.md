---
name: implement-loop
description: Implement all ready kanban cards autonomously until the board is clear. Uses ralph to prevent stopping between cards.
metadata:
  author: "swissarmyhammer"
  version: "0.9.2"
---

# Implement Loop

Autonomously implement every kanban card until the board is clear.

This skill is an **orchestrator**. It does not pick cards, write code, or run tests itself. It delegates to `/implement` and `/test`, and uses `ralph` to stay alive between cards.

Independent cards run in parallel. Dependent cards wait for their dependencies to complete.

## Process

1. **Set ralph**: call `ralph` with `op: "set ralph"` and instruction "Implement all kanban cards until the board is clear".
2. **Query ready cards**: call `kanban` with `op: "list tasks"` and `ready: true` to get all cards with no incomplete dependencies.
3. **Implement the batch**: Spawn parallel `Agent` subagents, one per card. Each agent runs `/implement` for a specific card.  Send all Agent tool calls in a **single message** so they run concurrently.
4. **Run `/test`** — after each batch completes, verify all tests pass.
5. **Check for remaining cards**: query `kanban` with `op: "list tasks"` and `ready: true`.
6. **If cards remain**: go back to step 3.
7. **Stop condition**: only when no ready cards remain may you call `ralph` with `op: "clear ralph"` and report.

### Parallel Agent Prompt Template

When spawning parallel agents, use this prompt pattern:

```
Implement kanban card [CARD-ID]: [CARD-TITLE]

Move it to doing, implement the work described in the card, run tests, and complete it.
Use `kanban` to move and complete the card.
Use the card ID directly — do not call `next task`.

Card ID: [CARD-ID]
```

Each agent must target a specific card by ID. Do NOT let parallel agents call `next task` — they will race and pick up the same card.

## Constraints

### Ralph

- **First action**: call `ralph` with `op: "set ralph"` and an instruction describing the goal.
- The Stop hook blocks you from stopping while ralph is active. This is intentional — do not work around it.
- Only call `ralph` with `op: "clear ralph"` when no ready cards remain.

### Delegation

- Use `/implement` for each card (sequential) or `Agent` tool (parallel). Each owns card selection, implementation, and completion.
- Use `/test` after each batch to verify all tests pass.
- Do not pick cards, write code, or run tests yourself.
- If an agent reports it is stuck on a card, move to the next — do not try to fix it yourself.

### Parallel Safety

- **Max 4 concurrent agents.** More than this risks resource exhaustion and merge conflicts.
- **After parallel agents complete**, check for merge conflicts in their worktrees before proceeding.
- **If a parallel agent fails**, continue with the others. Report the failure at the end.

### Scope

- Do only what the cards say. No bonus refactoring, no unrelated changes.
- The kanban board is the single source of truth. Do not use TodoWrite, TaskCreate, or other task tracking.

### When done

- Present a summary of all cards implemented and their test results.
- Note which cards ran in parallel vs sequential.
- Report any cards that failed or were skipped.
