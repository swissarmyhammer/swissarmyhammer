---
name: implement
profiles:
  - kanban
description: Kanban task executor. Use this skill when the user says "/implement", "implement task", "implement the next task", "work the next task", "pick up a task", or "implement" followed by a task id. Picks up one kanban task and drives it from ready through doing, leaving it green and ready for review. Do NOT use this skill for free-form edits, typo fixes, refactors, or any coding work that is not tied to a specific kanban task — those are not "implementation" in this skill sense. If there is no kanban task yet, use the `task` or `plan` skill to create one first.
agent: implementer
license: MIT OR Apache-2.0
compatibility: Requires the `kanban` MCP tool (to read, move, and complete tasks) and the `code_context` MCP tool (to research symbols and blast-radius before coding). 
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---


# Implement

Pick up a kanban task and get it done.

**Do NOT deviate from the plan.** A problem you can't resolve within the plan → stop and ask the user.

Here is what the user provided: 
$ARGUMENTS


## Guidelines

{% include "_partials/coding-standards" %}
{% include "_partials/architecture-awareness" %}
- One task at a time.
- Do the work. No "too complex". Find a way.
- Follow the coding standards — correct, robust, prevailing patterns.
- No unrelated refactors while implementing.
- All tests pass before reporting success. Zero failures, zero warnings.
- New work discovered? Add as a new kanban task.
- Stuck? Report what you tried and where you're blocked — don't silently give up.
- Do not create a worktree, just work in the current branch

## Invocation

`/implement` takes an optional argument — a task id, the sentinel `<next>`, or a filter DSL expression scoping `next task`.

| Invocation | Meaning |
|------------|---------|
| `/implement` | Same as `/implement <next>` — `next task` with no filter |
| `/implement <next>` | Explicit default |
| `/implement <task-id>` (26-char ULID) | That specific task. Never call `next task`. |
| `/implement #<tag>` | `next task` with `filter: "#<tag>"` |
| `/implement @<user>` | `next task` with `filter: "@<user>"` |
| `/implement $<project-slug>` | `next task` with `filter: "$<project-slug>"` |
| `/implement <filter-expr>` (e.g. `"#bug && @alice"`, `"$auth-migration && #bug"`) | Filter passed verbatim to `next task` |

Detection:
1. No arg or `<next>` → default
2. ULID (26 chars, `[0-9A-Z]`) → task-id
3. Otherwise → filter-expression (passes to `next task` verbatim)


### Filter DSL recap

Atoms: `#<tag>` (incl. virtual `#READY`, `#BLOCKED`, `#BLOCKING`), `@<user>`, `$<project-slug>`, `^<task-id>`. Operators: `&&` / `and`, `||` / `or`, `!` / `not`, `()`. Adjacent atoms = implicit AND.

The `^<task-id>` atom — like every id argument — accepts a full ULID, a 7-char short id, `^<short>`, or a unique ULID prefix. When you name a task in prose or commits, quote its `short_id` field (`^<short>`); never hand-abbreviate the ULID by prefix.

Parallel orchestrators (`finish`) always pass an explicit `<task-id>` to avoid racing on `next task`. Interactive `/implement` usually runs with no argument.

## Process

### Select the task

- **Task-id**: use directly. Don't call `next task`. Verify with `{"op": "get task", "id": "<id>"}`; missing → report and stop.
- **Default / `<next>`**: `op: "next task"`. Null → "board is clear", stop.
- **Filter-expression**: `op: "next task", filter: "<expr>"`. Null → "no ready tasks match", stop.

  ```json
  {"op": "next task", "filter": "#bug"}
  {"op": "next task", "filter": "#bug && @alice"}
  {"op": "next task", "filter": "$auth-migration"}
  {"op": "next task", "filter": "$auth-migration && #bug"}
  {"op": "next task", "filter": "#READY && !#docs"}
  ```

### Move to doing

Using the `id` of the selected task

```json
{"op": "move task", "id": "<id>", "column": "doing"}
```

### Read the task

```json
{"op": "get task", "id": "<id>"}
```

Full description + subtasks. Understand before writing code.


### Research before writing

**Don't guess.** Run the Code-Context Checkpoints (above) before changing any code:

- **Find symbols** — `search symbol` for functions/types/modules in the task
- **Read implementations** — `get symbol` for actual source, not just names
- **Map dependents** — `get callgraph` (inbound) on every symbol whose signature or behavior you change, to find its callers. When the symbol is shared or public, `get blastradius` on the file surfaces the wider set of callers, tests, and downstream consumers. It is not a mandatory gate — skip or disregard it when LSP call edges aren't available (empty `edges: []` is common on compiling code), and fall back to inbound `get callgraph` and targeted reads.
- **Trace call chains** — `get callgraph` (inbound) on every symbol whose signature or behavior changes
- **Check architecture** — read `ARCHITECTURE.md` (if present) per the Architecture Awareness guidance, to confirm where the change belongs
- **Fallback** — Glob/Grep/Read for string literals, config, patterns not in the index

If the task references a path, function, or type — **verify it still exists.** Tasks go stale; investigate mismatches before proceeding.

When using a library API, framework feature, or CLI flag — **look it up.** WebSearch/WebFetch the current docs. Every time. APIs change, flags get deprecated, versions ship breaking changes.

Never modify code you haven't read. Never assume what a function does — read it. Never assume a pattern exists — search. Never assume an API signature — look it up.

### Implement

Do the work in the task and subtasks. After changing any symbol's signature or behavior, re-run `get callgraph` (inbound) and confirm every blast-radius caller still holds.

When you think you are done `/double-check` your work and implement the feedback.

### Leave the task in `doing` for review

**Do not** move it to `review` yourself. 

**Do not** use `complete task` — it jumps to the terminal column, skipping the review gate entirely.

Cannot finish the work? Do NOT pretend it's done. Record what happened on the task — `{"op": "add comment", "task_id": "<id>", "text": "<what blocked you>"}` — and report back.

Summarize what was done and what tests pass, and tell the user it's ready for `/review` (which moves it into `review`). User decides next — no auto-continue.


### Record progress

{% include "_partials/record-progress" %}
