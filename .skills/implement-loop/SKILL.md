---
name: implement-loop
description: Implement all ready kanban cards autonomously until the board is clear. Uses ralph to prevent stopping between cards.
metadata:
  author: swissarmyhammer
  version: 0.12.11
---

# Implement Loop

Autonomously implement kanban cards until the board (or the scoped subset) is clear.

This skill is an **orchestrator**. It does not pick cards, write code, or run tests itself. It delegates to `/implement`, `/review`, and `/test`, and uses `ralph` to stay alive between cards.

Independent cards run in parallel. Dependent cards wait for their dependencies to complete.

The loop drives the full pipeline: `todo → doing → review → done`. `/implement` finishes cards into the `review` column (not `done`), and this orchestrator is responsible for dispatching `/review` on those cards so they either advance to `done` (clean) or come back for another implement pass (fresh findings appended as a checklist).

## Invocation

`/implement-loop` accepts an optional scoping argument that narrows which cards the loop picks up. It uses the same detection rules as `/implement`:

| Invocation | Meaning |
|------------|---------|
| `/implement-loop` | Work on every ready card on the board, regardless of tag/project. |
| `/implement-loop #<tag>` (e.g. `/implement-loop #bug`) | Only cards matching that tag. |
| `/implement-loop @<user>` | Only cards assigned to that user. |
| `/implement-loop $<project-slug>` (e.g. `/implement-loop $auth-migration`) | Only cards in that project. |
| `/implement-loop <filter-expression>` (e.g. `/implement-loop "#bug && @alice"`, `/implement-loop "$auth-migration && #bug"`) | Any filter DSL expression — applied to every `list tasks` call. |

Let `<SCOPE_FILTER>` denote the DSL expression (or absent) derived from the invocation. In every `list tasks` call below, combine `<SCOPE_FILTER>` with `#READY` (and any other structural constraint) using `&&`.

### Filter DSL recap

The DSL atoms: `#<tag>`, `@<user>`, `$<project-slug>`, `^<card-id>`, plus `&&`, `||`, `!`, and parens. Virtual tags `#READY`, `#BLOCKED`, `#BLOCKING` are available. All scoping — including project — flows through the filter DSL directly.

## Process

1. **Set ralph**: call `ralph` with `op: "set ralph"` and instruction "Implement all kanban cards until the scope is clear".

2. **Query ready implement cards**: call `kanban` with `op: "list tasks"`, `column: "todo"`, and a `filter` that combines `#READY` with the scope:
   - No scope → `filter: "#READY"`
   - Scope present → `filter: "#READY && (<SCOPE_FILTER>)"`

   Cards in `doing` are already being worked on; cards in `review` belong to step 5, not here.

3. **Implement the batch**: Spawn parallel `Agent` subagents, one per card. Each agent runs `/implement <card-id>` for a specific card. Send all Agent tool calls in a **single message** so they run concurrently. `/implement` will move each card through `doing` into `review` when it finishes — it will not move anything to `done`.

4. **Run `/test`** — after each implement batch completes, verify all tests pass.

5. **Query the review column (scoped)**: call `kanban` with `op: "list tasks"`, `column: "review"`, and the same `<SCOPE_FILTER>` (or no filter if none).

   Spawn parallel `Agent` subagents, one per card, each running `/review <card-id>`. Send them in a single message. Each `/review` agent either:
   - moves its card to `done` (clean: no new findings and any prior checklist items all checked), or
   - appends a fresh dated `## Review Findings` checklist to the card's description and leaves it in `review`.

6. **Handle review-column cards with unresolved findings**: after step 5, any card still in the scoped `review` set has a fresh `## Review Findings` checklist with unchecked `- [ ]` items. Those items are work to do. Dispatch parallel `/implement <card-id>` agents on each such card — `/implement` will read the description, work through the unchecked checklist items, flip them to `- [x]`, and move the card back to `review` on completion. Run `/test`, then return to step 5 to re-review.

7. **Loop**: return to step 2. Continue until both queries (ready todo in scope AND review cards in scope) return empty.

8. **Stop condition**: when both scoped queries are empty, call `ralph` with `op: "clear ralph"` and report. **Cards outside the scope are deliberately ignored** — the loop does not touch them even if they are ready.

### Parallel Agent Prompt Template

When spawning parallel agents, use this prompt pattern:

```
Run `/implement [CARD-ID]` on kanban card [CARD-ID]: [CARD-TITLE]

The explicit card ID form pins `/implement` to this specific card — it will not call `next task`.
`/implement` will move the card through doing → review. Do NOT let it use `complete task`.

Card ID: [CARD-ID]
```

Each agent must target a specific card by ID. Do NOT let parallel agents call `next task` — they will race and pick up the same card.

## Constraints

### Ralph

- **First action**: call `ralph` with `op: "set ralph"` and an instruction describing the goal.
- The Stop hook blocks you from stopping while ralph is active. This is intentional — do not work around it.
- Only call `ralph` with `op: "clear ralph"` when no ready cards remain.

### Delegation

- Use `/implement` for each card (sequential) or `Agent` tool (parallel). Each owns card selection, implementation, and moving the card into `review`.
- Use `/review` after each implement batch to drive cards from `review` to `done` (or back for another pass with fresh findings).
- Use `/test` after each implement batch to verify all tests pass.
- Do not pick cards, write code, run tests, or review code yourself.
- If an agent reports it is stuck on a card, move to the next — do not try to fix it yourself.

### Parallel Safety

- **Max 4 concurrent agents.**, folks are still using their computers.
- **Do NOT create additional worktrees.** Spawning agents with `isolation: "worktree"` causes changes to be lost — agents write to isolated copies that are never merged back. All agents must work directly in the current working tree.
- **If a parallel agent fails**, continue with the others. Report the failure at the end.

### Scope

- Do only what the cards say. No bonus refactoring, no unrelated changes.
- The kanban board is the single source of truth. Do not use TodoWrite, TaskCreate, or other task tracking.

### When done

- Present a summary of all cards implemented and their test results.
- Note which cards ran in parallel vs sequential.
- Report any cards that failed or were skipped.
