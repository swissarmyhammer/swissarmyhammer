---
name: finish
description: Drive kanban tasks from ready to done by looping implement → test → commit → review until each task is clean. Use when the user says "/finish", "drive tasks to done", "work the board", "finish the tasks", "finish the batch", or otherwise wants to orchestrate tasks through the full pipeline to done. Supports single-task mode (one task id) and scoped-batch mode (all ready tasks in a tag, project, or filter).
license: MIT OR Apache-2.0
compatibility: Requires the `kanban` and `ralph` MCP tools plus a Stop-hook-capable harness.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
hooks:
  Stop:
    - hooks:
        - type: command
          command: "sah tool ralph ralph check --"
---

# Finish

Drive kanban tasks all the way to `done` — orchestrating `/implement`, `/test`, `/commit`, and `/review` in a loop until each task lands in `done` or is reported stuck.

**Orchestrator only** — does not write code, run tests, or commit. Delegates to `/implement`, `/review`, `/test`, `/commit`; uses `ralph` to stay alive between iterations.

**IMPORTANT** each of the skill driven steps should be run in an appropriate sub agent to minimize context bloat in this session.

**IMPORTANT — block on every sub agent.** `Agent` starts the sub agent and returns
at once. It does not wait. If you end your turn here, the `ralph` Stop hook starts
you again, the step is still running, and you have nothing to report. That repeats
for as long as the step runs.

So every `Agent` call is followed at once by a blocking read of its result:

```json
{"tool": "TaskOutput", "task_id": "<the agentId Agent returned>", "block": true, "timeout": 600000}
```

`TaskOutput` holds inside the turn until the sub agent finishes, then gives you the
step record block. The turn never ends while a step runs, so the Stop hook never
fires.

Rules:

- One `Agent` call, then one `TaskOutput` call. Never two `Agent` calls in a row.
- Use the largest timeout the tool accepts. A test run on a large workspace takes
  more than ten minutes.
- **Call `TaskOutput` once for each step.** A `TaskOutput` that returns
  `status: running` hit its own ceiling. Do not call it again. On a sub agent that
  still runs, `TaskOutput` returns the raw transcript of the sub agent, not the
  step record — tens of thousands of tokens of tool calls you already delegated
  to keep out of this context. A second call costs that again and still gives no
  result.
- After a `status: running`, end the turn. The harness sends a task notification
  when the sub agent finishes, and that notification carries the step record.
  The `ralph` Stop hook may start you once or twice while you wait. Answer with
  one short line and end the turn again. That is cheap; a transcript dump is not.
- Never `sleep` in a shell, and never poll `ListAgents` in a loop. Neither one
  tells you anything.


## Invocation

| Invocation | Mode | Meaning |
|------------|------|---------|
| `/finish <task-id>` (ULID or short id) | **single-task** | Drive exactly that task. Never call `next task`. |
| `/finish` | **scoped-batch** (no scope) | All ready tasks. |
| `/finish #<tag>` | **scoped-batch** | Matching tag. |
| `/finish @<user>` | **scoped-batch** | Assigned to user. |
| `/finish $<project-slug>` | **scoped-batch** | In project. |
| `/finish <filter-expr>` | **scoped-batch** | Any filter DSL — applied to every `list tasks`. |

Detection:
1. ULID (26 chars, `[0-9A-Z]`) or short ULID → single-task
2. No arg → scoped-batch, no filter
3. Otherwise → scoped-batch, arg passed verbatim as filter

Let `<SCOPE_FILTER>` be the DSL expression (or absent). Combine with `#READY` via `&&` on every scoped `list tasks`.

### Filter DSL recap

Atoms: `#<tag>`, `@<user>`, `$<project-slug>`, `^<task-id>`. Operators: `&&`, `||`, `!`, `()`. Virtual tags: `#READY`, `#BLOCKED`, `#BLOCKING`. All scoping (incl. project) flows through the filter.

The `^<task-id>` atom and every id argument accept a full ULID, a 7-char short id, `^<short>`, or a unique ULID prefix. When reporting on a task in prose, quote its `short_id` field (`^<short>`) rather than hand-abbreviating the ULID by prefix.

## Process

### Set ralph (both modes)

**First action**:

```json
{"op": "set ralph", "instruction": "<mode-specific goal>"}
```

- single-task: `"Finish task <TASK_ID> — loop until it lands in done"`
- scoped-batch: `"Finish all ready kanban tasks in <SCOPE> until the scope is clear"`

The Stop hook blocks stopping while ralph is active. Only `clear ralph` when the stop condition is met.

### Detect Projects

`/detected-projects` so we know what we are working with up front.

### Single-task mode

Pin `<TASK_ID>` for the entire loop — never `next task`, never switch tasks.

1. **Verify exists**: `op: "get task", id: "<TASK_ID>"`. Missing → clear ralph and report.
2. **Implement**: `/implement <TASK_ID>`. Implement moves the task into `doing` (pulling it back from `review` if it's returning with findings), does the work, and **leaves it in `doing`**. 
3. **Test**: `/test`. Failures → step 2.
4. **Checkpoint the green state**: invoke `/commit` to create a **local** commit of the green, tested working tree. This is the per-iteration rollback point and — critically — it is what makes the next review tight: with the work committed, the review scopes to *this iteration's commit*, not the whole accumulated uncommitted diff. **Commit only, NEVER push** (pushing is the user's separate step; per-task pushes would spam CI in batch mode). `/commit` stages all changes; "nothing to commit" is a no-op, not an error — but it means implement produced **no change this iteration** (no progress): record it and treat it under the step 7 guardrail rather than re-reviewing a stale diff.
5. **Review**: `/review <TASK_ID> HEAD~1..HEAD` — task-mode on `<TASK_ID>`, scoped to the checkpoint delta just committed (only this iteration's change, never the whole accumulated task diff). `/review` pulls the task `doing → review` and records findings on `<TASK_ID>`:
   - **clean** → task moves to `done`. Step 6.
   - **findings** → fresh dated `## Review Findings` checklist appended to the task, task stays in `review`. Step 2 — `/implement <TASK_ID>` pulls it back to `doing`, works the unchecked items, and flips them to `- [x]`.
6. **Verify done**: `op: "get task"`. Not in `done` → step 2. In `done` → the last checkpoint (step 4) already **is** the verified-good commit (green + clean review); no separate post-done commit is needed.
7. **Write the ledger entry, then check the guardrail.** Record this iteration on the task (see **The iteration ledger** below), then decide from the ledger — never from your memory of it: the same finding (file:line + message) in 3 ledger entries — or 3 consecutive `no-change` entries (step 4 "nothing to commit") — → stop, clear ralph, report what persists. Hitting the guardrail means the task is **stuck**: leave it in `review` and report it — **never force it to `done`**. A finding that survives 3 rounds is either a fix you haven't cracked yet or a contradictory/faulty rule; if it's the latter (per Scope), report it on the task and leave it **stuck** for a human to resolve — do not edit validators yourself and do not re-close. Closing a task with open findings is out of bounds.
8. **Clear ralph** and report with the card block (see **Report the card**).

### The iteration ledger (both modes)

{% include "_partials/step-record" %}

Every delegated step returns that block. Read the block — do not re-derive the outcome from the step's prose, and do not re-run the step to find out what it did.

At the end of every iteration, write one ledger comment on the task. This is the only durable copy of the loop state:

```json
{"op": "add comment", "task_id": "<TASK_ID>", "text": "### finish iteration 3 — findings\n- implement: changed — 3 files\n- test: green — cargo nextest, 1284 passed\n- commit: 42e32c3a3\n- review: findings — crates/kanban/src/tag.rs:88, crates/kanban/src/tag.rs:140"}
```

Rules for the ledger:

- The heading outcome is the outcome of the **last** step that ran in the iteration.
- The iteration number is the count of ledger comments already on the card, plus one. It is not the count of iterations in this session. A card picked up after a restart continues the count.
- Write the entry even when the iteration made no progress. `no-change` is the entry that arms the guardrail.

**Read the ledger before you decide:**

```json
{"op": "list comments", "task_id": "<TASK_ID>"}
```

The guardrail in step 7 compares the last three ledger entries. Reading them from the card — not from this conversation — is what makes the loop survive a context summary, a stopped session, or a different agent that picks the card up.

### Report the card

{% include "_partials/card-report" %}

Show the card block after every iteration, and once more when the loop stops.

In scoped-batch mode, show one block for each task as it leaves the loop. When the scope is clear, close with a list of three groups: **done**, **stuck**, and **skipped**. Name each task by its `short_id` and give the reason for every stuck and skipped task.

### Scoped-batch mode

**Strictly sequential — one task at a time.** Never use worktrees, never run concurrent `/implement` or `/review`. Pick one task, drive it fully to `done` using the exact single-task loop, then pick the next. (Parallel work on the shared working tree have repeatedly clobbered changes via stash/revert races — the slowness of sequential runs is far cheaper than lost work.)

1. **Pick one task in scope.** First check the `review` column, then the ready `todo` column — a task already in `review` is closer to done, so finish it first:
   - `op: "list tasks", column: "review"`, `filter`:
     - No scope → absent
     - Scope → `"<SCOPE_FILTER>"`
   - `op: "list tasks", column: "todo"`, `filter`:
     - No scope → `"#READY"`
     - Scope → `"#READY && (<SCOPE_FILTER>)"`

   Tasks in `doing` are already being worked — leave them. Take the **first** task from `review` if any, otherwise the first ready `todo` task. Pin its id as `<TASK_ID>`.

2. **Drive it to done.** Run the **single-task mode loop** (above) on `<TASK_ID>` in a sub agent. Reusing the loop means each iteration commits a local checkpoint via step 4, so by the time a task reaches `done` its verified-good state is already committed — before the next task is picked. Do not switch tasks mid-loop. A task that hits the guardrail is reported as stuck and skipped.

3. **Pick the next.** Return to step 1.

4. **Stop**: both the scoped `review` query and the scoped ready `todo` query return empty → `clear ralph` and report. **Tasks outside scope are deliberately ignored.**


### Sequential safety (both modes)
- **One task at a time.** Never spawn parallel `Agent` subagents, never run concurrent `/implement` or `/review`. Scoped-batch picks one task, drives it to `done`, then picks the next.
- **No worktrees.** `isolation: "worktree"` loses changes — agents write to isolated copies never merged back. All work happens in the current tree.
- Parallel agents on the shared tree have repeatedly clobbered work via stash/revert races. If asked to "speed up" finish, say no — slow and correct beats fast and lost.

### Scope
- **Review findings are in scope by definition.** A finding recorded by `/review` is work the task must address; acting on it is never "bonus refactoring." The no-bonus-refactoring rule restrains changes *you* invent — never the engine's findings.
- **The review gate is the only way to `done`.** A task reaches `done` when a fresh `/review` returns zero new findings, every prior item is checked, and `/review` itself moves it. Do **not** force a task to `done` with `complete task` / `move task` while findings are open. A task with open findings is **stuck**: leave it in `review` and report it.
- **Re-review is not noise.** New findings each round are the engine working. Keep looping.

{% include "_partials/findings-are-requirements" %}
