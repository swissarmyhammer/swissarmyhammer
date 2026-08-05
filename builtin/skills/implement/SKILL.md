---
name: implement
description: Use this skill when the user says "/implement", "implement task", "implement the next task", "work the next task", "pick up a task", or "implement" followed by a task id. Picks up one kanban task and drives it from ready through doing, leaving it green and ready for review. Do NOT use this skill for free-form edits, typo fixes, refactors, or any coding work that is not tied to a specific kanban task — those are not "implementation" in this skill sense. If there is no kanban task yet, use the `task` or `plan` skill to create one first.
agent: implementer
license: MIT OR Apache-2.0
compatibility: Requires the `kanban` MCP tool (to read, move, and complete tasks), the `code_context` MCP tool (to research symbols and blast-radius before coding), and the `review` MCP tool (to fetch the validator rules before editing, and to self-review before handoff). 
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


- One task at a time.
- Do the work. No "too complex". Find a way.
- Follow the coding standards — correct, robust, prevailing patterns.
- No unrelated refactors while implementing.
- All tests pass before reporting success. Zero failures, zero warnings.
- New work discovered? Add as a new kanban task.
- Stuck? Report what you tried and where you're blocked — don't silently give up.
- Do not create a worktree, just work in the current branch

{% include "_partials/findings-are-requirements" %}

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

### Know the rules

Get the rules that review will enforce, before you edit a file. Rules match by file pattern, so one example file for each extension gives the full rule set.

Collect the distinct extensions of the files you plan to edit. Pick one example file for each extension. Then make one call on the `review` tool with those example paths:

```json
{"op": "dump validators", "paths": ["<one example file per extension>"]}
```

The call writes one markdown file and returns its path. Read that file whole, one time. The file carries every applicable rule body word for word.

Do not call again for more files with the same extensions. Call again only when a later edit targets a file with a new extension.

Obey each rule as you write the code, not after. Document each public item. Name each numeric constant. Do not copy blocks. Keep functions small and flat. Follow the project naming. Delete dead code.

### Implement

Do the work in the task and subtasks. After changing any symbol's signature or behavior, re-run `get callgraph` (inbound) and confirm every blast-radius caller still works.

### Self-review

Review your own work before you hand it off:

```json
{"op": "review working"}
```

Fix every finding. A finding is a requirement. Do not rank findings. Do not defer findings. Do not label findings.

Run the review again. Repeat until the review is clean.

One self-review run costs about 15 minutes. One full implement→test→review pass costs about 50 minutes. Each finding you fix here removes a pass.

When the review is clean `/double-check` your work and implement the feedback. Only then hand off for the formal `/review`.

### Leave the task in `doing` for review

**Do not** move it to `review` yourself. 

**Do not** use `complete task` — it jumps to the terminal column, skipping the review gate entirely.

Cannot finish the work? Do NOT pretend it's done. Record what happened on the task — `{"op": "add comment", "task_id": "<id>", "text": "<what blocked you>"}` — and report the `stuck` outcome.


### Record progress

{% include "_partials/record-progress" %}

{% include "_partials/step-record" %}

Implement reports `changed`, `no-change`, or `stuck`. The evidence is the list of files it touched.

```
step: implement
outcome: changed
evidence: 3 files — src/auth/login.rs, src/auth/mod.rs, tests/auth.rs
task: ^rc9rb4g
```

`no-change` is a real result, not a failure to report. It tells the orchestrator that this pass made no progress.


### Report to the user

{% include "_partials/card-report" %}

After the block, tell the user the task is ready for `/review` (which moves it into `review`). The user decides next — no auto-continue.
