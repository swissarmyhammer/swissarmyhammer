---
name: implement
description: Implementation workflow. Use this skill whenever you are implementing, coding, or building. Picks up one kanban card and does the work. Produces verbose output — automatically delegates to an implementer subagent.
metadata:
  author: "swissarmyhammer"
  version: "0.12.11"
---

## Code Quality

- Write clean, readable code that follows existing patterns in the codebase
- Prefer simple, obvious solutions over clever ones
- Make minimal changes to achieve the goal - avoid unnecessary refactoring
- Don't add features, abstractions, or "improvements" beyond what was asked

## Style

- Follow the project's existing conventions for naming, formatting, and structure
- Match the indentation, quotes, and spacing style already in use
- If the project has a formatter config (prettier, rustfmt, black), respect it

## Documentation

- Every function needs a docstring explaining what it does
- Document parameters, return values, and errors
- Update existing documentation if your changes make it stale
- Inline comments explain "why", not "what"

## Error Handling

- Handle errors at appropriate boundaries
- Don't add defensive code for scenarios that can't happen
- Trust internal code and framework guarantees


# Implement

Pick up the next kanban card and get it done.

DO NOT deviate from the plan -- if you run into a problem, you need to stop and ask the user for guidance -- DO NOT deviate from the plan without permission from the user.

## Process

### 1. Get the next card

Use `kanban` with `op: "next task"` to find the next actionable card. If there are no remaining cards, tell the user the board is clear.

### 2. Move the card to doing

```json
{"op": "move task", "id": "<task-id>", "column": "doing"}
```

### 3. Read the card

```json
{"op": "get task", "id": "<task-id>"}
```

Get the full description and subtasks. Understand the task before writing code.

### 4. Research before writing

**Do not guess.** Use `code_context` to understand the code before changing it:

- **Find symbols** — `op: "search symbol"` to locate functions, types, and modules mentioned in the card
- **Read implementations** — `op: "get symbol"` to see actual source code, not just names
- **Map blast radius** — `op: "get blastradius"` on files you plan to change, to find callers, tests, and downstream consumers you might break
- **Trace call chains** — `op: "get callgraph"` to understand how code flows before inserting yourself into it
- **Fall back to text search** — Glob, Grep, Read for string literals, config values, or patterns not in the index

If the card references a file path, function name, or type — **verify it still exists before acting on it.** Cards can go stale. A function may have been renamed, moved, or deleted since the card was written. If something doesn't match, investigate before proceeding.

When using a library API, framework feature, or CLI flag — **look it up.** Use `WebSearch` or `WebFetch` to check current docs before writing the code. Every time. No exceptions. APIs change, flags get deprecated, new versions ship breaking changes. Verify against the actual docs.

Never modify code you haven't read. Never assume you know what a function does — read it. Never assume a pattern exists — search for it. Never assume an API signature — look it up. The cost of looking is low; the cost of guessing wrong is a broken build and wasted time.

### 5. Implement the work

Do the work described in the card and its subtasks.

### 6. Complete the card

When all subtasks pass:

```json
{"op": "complete task", "id": "<task-id>"}
```

A card left in "doing" is not finished.

If you cannot complete the task, do NOT complete the card. Add a comment describing what happened and report back.

### 7. Stop for review

**Always stop after completing a card.** Present a summary of what was done and what tests pass. The user decides when to move to the next card — you do not auto-continue.

Only exception: if the card description explicitly says **auto-continue** or **chain to next**, proceed to the next card without stopping.

## Rules

- One card at a time. Don't try to do multiple cards in one pass.
- Do the work. No excuses, no "too complex". Find a way.
- Don't over-engineer — write the simplest code that works.
- Don't refactor unrelated code while implementing.
- Stay focused on the task you were given.
- ALL tests must pass before you report success. Zero failures, zero warnings.
- Do NOT use TodoWrite, TaskCreate, or any other task tracking — the kanban board is the single source of truth.
- If you discover new work, add it as a new kanban card.
- If you get stuck, report what you tried and where you're blocked — don't silently give up.
- **Do NOT create additional worktrees.** Spawning agents with `isolation: "worktree"` causes changes to be lost — agents write to isolated copies that are never merged back. All agents must work directly in the current working tree.
