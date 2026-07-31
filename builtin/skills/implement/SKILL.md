---
name: implement
profiles:
  - kanban
description: Use this skill when the user says "/implement", "implement task", "implement the next task", "work the next task", "pick up a task", or "implement" followed by a task id. Picks up one kanban task and drives it from ready through doing, leaving it green and ready for review. Do NOT use this skill for free-form edits, typo fixes, refactors, or any coding work that is not tied to a specific kanban task — those are not "implementation" in this skill sense. If there is no kanban task yet, use the `task` or `plan` skill to create one first.
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

`/implement` takes a required task id.

| Invocation | Meaning |
|------------|---------|
| `/implement <task-id>` (26-char ULID) | That specific task. Never call `next task`. |
| `/implement ^<task-id>` | That specific task. Never call `next task`. |

The `^<task-id>` atom — like every id argument — accepts a full ULID, a 7-char short id, `^<short>`, or a unique ULID prefix. When you name a task in prose or commits, quote its `short_id` field (`^<short>`); never hand-abbreviate the ULID by prefix.

## Process

### Read the task

```json
{"op": "get task", "id": "<id>"}
```

Full description + subtasks. Understand before writing code. If you cannot find the task stop and report.

### Move to doing

Using the `id` of the selected task

```json
{"op": "move task", "id": "<id>", "column": "doing"}
```

### Research before writing

Use the `/explore` skill to research the context provided in the task.

Record what you discovered on the task — `{"op": "add comment", "task_id": "<id>", "text": "<discoveries>"}`.

### Implement

Do the work in the task and subtasks. After changing any symbol's signature or behavior, re-run `get callgraph` (inbound) and confirm every blast-radius caller still works.

When you think you are done `/double-check` your work and implement the feedback.

### Leave the task in `doing` for review

**Do not** move it to `review` yourself. 

**Do not** use `complete task` — it jumps to the terminal column, skipping the review gate entirely.

Cannot finish the work? Do NOT pretend it's done. Record what happened on the task — `{"op": "add comment", "task_id": "<id>", "text": "<what blocked you>"}` — and report back.

Summarize what was done and what tests pass, and tell the user it's ready for `/review` (which moves it into `review`). User decides next — no auto-continue.


### Record progress

{% include "_partials/record-progress" %}
