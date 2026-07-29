---
name: implement
profiles:
  - kanban
description: Kanban task executor. Use this skill when the user says "/implement", "implement task", "implement the next task", "work the next task", "pick up a task", or "implement" followed by a task id. The skill picks up one kanban task. It drives the task from ready through doing. It leaves the task green and ready for review. Do NOT use this skill for free-form edits, typo fixes, refactors, or any coding work not tied to a specific kanban task — work of that kind is not "implementation" in this skill's sense. If there is no kanban task yet, use the `task` or `plan` skill to create one first.
agent: implementer
license: MIT OR Apache-2.0
compatibility: This skill requires the `kanban` MCP tool to read, move, and complete tasks. It also requires the `code_context` MCP tool to research symbols and blast radius before coding.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---


# Implement

Pick up a kanban task and get it done.

**Do NOT deviate from the plan.** If you cannot resolve a problem within the plan, stop and ask the user.

Here is what the user provided:
$ARGUMENTS


## Guidelines

{% include "_partials/coding-standards" %}
{% include "_partials/architecture-awareness" %}

## Invocation

`/implement` takes an optional argument. The argument is a task id, the sentinel `<next>`, or a filter DSL expression that scopes `next task`.

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

Atoms: `#<tag>` (including virtual `#READY`, `#BLOCKED`, `#BLOCKING`), `@<user>`, `$<project-slug>`, `^<task-id>`. Operators: `&&` / `and`, `||` / `or`, `!` / `not`, `()`. Adjacent atoms mean an implicit AND.

The `^<task-id>` atom, like every id argument, accepts a full ULID, a 7-char short id, `^<short>`, or a unique ULID prefix. When you name a task in prose or in commits, quote its `short_id` field (`^<short>`). Never hand-abbreviate the ULID by prefix.

Parallel orchestrators, such as `finish`, always pass an explicit `<task-id>`. This avoids racing on `next task`. Interactive `/implement` usually runs with no argument.

## Process

### 1. Select the task

- **Task-id**: use it directly. Do not call `next task`. Verify with `{"op": "get task", "id": "<id>"}`. If the task is missing, report this and stop.
- **Default / `<next>`**: call `op: "next task"`. If the result is null, report "board is clear" and stop.
- **Filter-expression**: call `op: "next task", filter: "<expr>"`. If the result is null, report "no ready tasks match" and stop.

  ```json
  {"op": "next task", "filter": "#bug"}
  {"op": "next task", "filter": "#bug && @alice"}
  {"op": "next task", "filter": "$auth-migration"}
  {"op": "next task", "filter": "$auth-migration && #bug"}
  {"op": "next task", "filter": "#READY && !#docs"}
  ```

### 2. Move to doing

```json
{"op": "move task", "id": "<id>", "column": "doing"}
```

### 3. Read the task

```json
{"op": "get task", "id": "<id>"}
```

Read the full description and the subtasks. Understand the task before you write code.

### Record progress

{% include "_partials/record-progress" %}

### 4. Research before writing

**Do not guess.** Run the Code-Context Checkpoints (above) before you change any code:

- **Find symbols** — use `search symbol` for functions, types, or modules in the task
- **Read implementations** — use `get symbol` for the actual source, not just names
- **Map dependents** — run `get callgraph` (inbound) on every symbol whose signature or behavior you change, to find its callers. When the symbol is shared or public, run `get blastradius` on the file to surface the wider set of callers, tests, and downstream consumers. This is not a mandatory gate. Skip it or disregard it when LSP call edges are not available — an empty `edges: []` is common on compiling code. In that case, fall back to inbound `get callgraph` and targeted reads.
- **Trace call chains** — run `get callgraph` (inbound) on every symbol whose signature or behavior changes
- **Check architecture** — read `ARCHITECTURE.md`, if present, per the Architecture Awareness guidance. Confirm where the change belongs.
- **Fallback** — use Glob, Grep, or Read for string literals, config, or patterns not in the index

If the task references a path, function, or type, **verify it still exists.** Tasks go stale. Investigate any mismatch before you proceed.

When you use a library API, a framework feature, or a CLI flag, **look it up.** Use WebSearch or WebFetch to check the current docs, every time. APIs change. Flags become deprecated. New versions ship breaking changes.

Never modify code you have not read. Never assume what a function does; read it. Never assume a pattern exists; search for it. Never assume an API signature; look it up.

### 5. Implement

Do the work in the task and its subtasks. After you change any symbol's signature or behavior, re-run `get callgraph` (inbound). Confirm that every blast-radius caller still holds.

### 5.5 Verify with really-done

When the work is done, invoke the `really-done` skill to verify it.

- The verification-command pass is really-done's **hard requirement.** Verification commands must be green before you hand off the task. This gates handoff.
- really-done now runs the advisory adversarial double-check internally, so its sign-off is reached **transitively** through really-done. **Do NOT spawn the double-check agent directly from implement** — reach it through really-done.
- Double-check findings are advisory: fix them, or proceed with a logged justification per really-done's contract.

If the result is not green, do NOT hand off. Fix the work and re-run really-done. Or record what blocked you on the task and report back.

### 6. Leave the task in `doing` for review

When the work is done, really-done is green, and every subtask checkbox is `- [x]`, **leave the task in `doing`**. Do **not** move it to `review` yourself.

Moving a task into `review` is the review step's job, not implement's. `/review` pulls the task from `doing` into `review` when it runs — and under `/finish`, only after the green state has been committed as a checkpoint. Implement establishes that the work is done and green; it does not declare the work ready to review by moving columns. A single owner for the `doing → review` transition is the whole point — implement no longer touches the `review` column.

**Do NOT use `complete task`** — it jumps to the terminal column and skips the review gate entirely.

If you cannot finish the work, do NOT pretend it is done. Record what happened on the task — `{"op": "add comment", "task_id": "<id>", "text": "<what blocked you>"}` — and report back.

### 7. Stop for review

**Always stop once the work is done and green.** The task stays in `doing`. Summarize what you did and which tests pass. Tell the user the task is ready for `/review`, which moves it into `review`. The user decides what happens next; do not auto-continue.

Exception: if the task description explicitly says **auto-continue** or **chain to next**, proceed.

## Rules

- One task at a time.
- Do the work. Never say a task is "too complex". Find a way.
- Follow the coding standards: write correct, robust code that follows the prevailing patterns.
- No unrelated refactors while implementing.
- Stay focused. Validator feedback is part of the task — fixing validator issues is never a deviation.
- All tests must pass before you report success. Allow zero failures and zero warnings.
- Kanban is the single source of truth — no TodoWrite/TaskCreate.
- If you discover new work, add it as a new kanban task.
- Do not hand off a task as done until you have run really-done and the verification commands are green.
- Implement never moves a task into `review` — it leaves the green task in `doing` for `/review` to pick up. (It may still pull a returning task from `review` back to `doing` when it reworks findings.)
- If you get stuck, report what you tried and where you are blocked — do not silently give up.
- **No worktrees.** `isolation: "worktree"` loses changes, because agents write to isolated copies that are never merged back. Work directly in the current tree.
