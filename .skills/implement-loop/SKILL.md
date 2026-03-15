---
name: implement-loop
description: Implement all ready kanban cards autonomously until the board is clear. Uses ralph to prevent stopping between cards.
metadata:
  author: "swissarmyhammer"
  version: "1.0"
---

# Implement Loop

Autonomously implement every kanban card until the board is clear.

This skill is an **orchestrator**. It does not pick cards, write code, or run tests itself. It delegates to `/implement` and `/test`, and uses `ralph` to stay alive between cards.

## Process

1. **Set ralph**: call `ralph` with `op: "set ralph"` and instruction "Implement all kanban cards until the board is clear".
2. **Run `/implement`** — it picks the next card, implements it, and marks it complete.
3. **Run `/test`** — verify all tests pass after the implementation.
4. **Check for remaining cards**: query `kanban` with `next task`.
5. **If cards remain**: go back to step 2.
6. **Stop condition**: only when `kanban` `next task` returns no cards may you call `ralph` with `op: "clear ralph"` and report.

## Constraints

### Ralph

- **First action**: call `ralph` with `op: "set ralph"` and an instruction describing the goal.
- The Stop hook blocks you from stopping while ralph is active. This is intentional — do not work around it.
- Only call `ralph` with `op: "clear ralph"` when `kanban` `next task` returns no cards.

### Delegation

- Use `/implement` for each card. It owns card selection, implementation, and completion.
- Use `/test` after each card to verify all tests pass.
- Do not pick cards, write code, or run tests yourself.
- If `/implement` reports it is stuck on a card, move to the next — do not try to fix it yourself.

### Scope

- Do only what the cards say. No bonus refactoring, no unrelated changes.
- The kanban board is the single source of truth. Do not use TodoWrite, TaskCreate, or other task tracking.

### When done

- Present a summary of all cards implemented and their test results.
