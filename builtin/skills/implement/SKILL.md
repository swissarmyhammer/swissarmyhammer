---
name: implement
description: Implementation workflow. Use this skill whenever you are implementing, coding, or building. Picks up one kanban task and does the work. Produces verbose output — automatically delegates to an implementer subagent.
context: fork
agent: implementer
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

{% include "_partials/coding-standards" %}
{% include "_partials/review-column" %}

# Implement

Pick up a kanban task and get it done.

DO NOT deviate from the plan -- if you run into a problem, you need to stop and ask the user for guidance -- DO NOT deviate from the plan without permission from the user.

## Invocation

`/implement` accepts an optional argument. It can be a literal task id, the sentinel `<next>`, or a filter DSL expression that scopes which task `next task` returns.

| Invocation | Meaning |
|------------|---------|
| `/implement` | Default — same as `/implement <next>`. Picks up the next actionable task via `next task` with no filter. |
| `/implement <next>` | Explicit form of the default. |
| `/implement <task-id>` (e.g. `/implement 01KN...`) | Work on the specific task with that ULID. Do NOT call `next task`. |
| `/implement #<tag>` (e.g. `/implement #bug`) | Pick the next actionable task with that tag. Passes `filter: "#<tag>"` to `next task`. |
| `/implement @<user>` (e.g. `/implement @alice`) | Pick the next actionable task assigned to that user. |
| `/implement $<project-slug>` (e.g. `/implement $auth-migration`) | Pick the next actionable task in the given project. Passes `filter: "$<project-slug>"` to `next task`. |
| `/implement <filter-expression>` (e.g. `/implement "#bug && @alice"`, `/implement "$auth-migration && #bug"`) | Any valid filter DSL expression — passes straight through to `next task`'s `filter` parameter. |

Argument detection rules (for the skill to apply):

1. No argument or the literal string `<next>` → default mode (no filter).
2. Argument matches a ULID pattern (26 chars, `[0-9A-Z]`) → task-id mode.
3. Otherwise → filter-expression mode (pass to `next task` verbatim). This covers `#tag`, `@user`, `$project-slug`, `^ref`, and any compound expression.

### Filter DSL recap

The DSL atoms that `next task` understands:

- `#<tag>` — tag match (including virtual tags `#READY`, `#BLOCKED`, `#BLOCKING`)
- `@<user>` — assignee match
- `$<project-slug>` — project match
- `^<task-id>` — reference match
- `&&` / `and`, `||` / `or`, `!` / `not`, `()` — boolean composition
- Adjacent atoms → implicit AND: `#bug @alice` = `#bug && @alice`, `$auth-migration #bug` = `$auth-migration && #bug`

Parallel orchestrators (like `finish`) always pass an explicit `<task-id>` to avoid racing on `next task`. Interactive `/implement` usually runs with no argument and falls back to `<next>`.

## Process

### 1. Select the task

Apply the detection rules above to decide which sub-flow to run:

- **Task-id mode** (`/implement <task-id>`): use that id directly. Do NOT call `next task`. Verify the task exists with `{"op": "get task", "id": "<task-id>"}` before proceeding; if it doesn't exist, report the error and stop.

- **Default / `<next>` mode**: call `kanban` with `op: "next task"`. If it returns null, tell the user the board is clear and stop.

- **Filter-expression mode** (`#tag`, `@user`, `$project-slug`, `^ref`, or any compound): call `kanban` with `op: "next task"` and `filter: "<expression>"`. If it returns null, tell the user no ready tasks match that filter and stop.

  ```json
  {"op": "next task", "filter": "#bug"}
  {"op": "next task", "filter": "#bug && @alice"}
  {"op": "next task", "filter": "$auth-migration"}
  {"op": "next task", "filter": "$auth-migration && #bug"}
  {"op": "next task", "filter": "#READY && !#docs"}
  ```

### 2. Move the task to doing

```json
{"op": "move task", "id": "<task-id>", "column": "doing"}
```

### 3. Read the task

```json
{"op": "get task", "id": "<task-id>"}
```

Get the full description and subtasks. Understand the task before writing code.

### 4. Research before writing

**Do not guess.** Use `code_context` to understand the code before changing it:

- **Find symbols** — `op: "search symbol"` to locate functions, types, and modules mentioned in the task
- **Read implementations** — `op: "get symbol"` to see actual source code, not just names
- **Map blast radius** — `op: "get blastradius"` on files you plan to change, to find callers, tests, and downstream consumers you might break
- **Trace call chains** — `op: "get callgraph"` to understand how code flows before inserting yourself into it
- **Fall back to text search** — Glob, Grep, Read for string literals, config values, or patterns not in the index

If the task references a file path, function name, or type — **verify it still exists before acting on it.** Tasks can go stale. A function may have been renamed, moved, or deleted since the task was written. If something doesn't match, investigate before proceeding.

When using a library API, framework feature, or CLI flag — **look it up.** Use `WebSearch` or `WebFetch` to check current docs before writing the code. Every time. No exceptions. APIs change, flags get deprecated, new versions ship breaking changes. Verify against the actual docs.

Never modify code you haven't read. Never assume you know what a function does — read it. Never assume a pattern exists — search for it. Never assume an API signature — look it up. The cost of looking is low; the cost of guessing wrong is a broken build and wasted time.

### 5. Implement the work

Do the work described in the task and its subtasks.

### 6. Move the task to review

When the work is done and every subtask checkbox in the task description is flipped to `- [x]`:

1. First, ensure the `review` column exists by following the **Ensure the Review Column Exists** partial above. It is idempotent — run it every time.

2. Then move the task to `review`:

   ```json
   {"op": "move task", "id": "<task-id>", "column": "review"}
   ```

A task left in "doing" is not finished.

**Do NOT use `complete task`.** `complete task` always moves the task to the terminal column, which would skip the review gate. Use `move task` with `column: "review"` explicitly.

If you cannot complete the task, do NOT move it forward. Add a comment describing what happened and report back.

### 7. Stop for review

**Always stop after moving a task to review.** Present a summary of what was done, what tests pass, and tell the user the task is ready for `/review`. The user decides when to move on — you do not auto-continue.

Only exception: if the task description explicitly says **auto-continue** or **chain to next**, proceed to the next task without stopping.

## Rules

- One task at a time. Don't try to do multiple tasks in one pass.
- Do the work. No excuses, no "too complex". Find a way.
- Follow the coding standards — correct, robust, well-designed code that follows prevailing patterns.
- Don't refactor unrelated code while implementing.
- Stay focused on the task you were given.
- ALL tests must pass before you report success. Zero failures, zero warnings.
- Do NOT use TodoWrite, TaskCreate, or any other task tracking — the kanban board is the single source of truth.
- If you discover new work, add it as a new kanban task.
- If you get stuck, report what you tried and where you're blocked — don't silently give up.
- **Do NOT create additional worktrees.** Spawning agents with `isolation: "worktree"` causes changes to be lost — agents write to isolated copies that are never merged back. All agents must work directly in the current working tree.
