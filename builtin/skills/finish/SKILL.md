---
name: finish
profiles:
  - kanban
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

**Orchestrator only** — does not pick tasks, write code, run tests, or commit. Delegates to `/implement`, `/review`, `/test`, `/commit`; uses `ralph` to stay alive between iterations.

**IMPORTANT** each of the skill driven steps should be run in an appropriate sub agent to minimize context bloat in this session.


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
- scoped-batch: `"Finish all ready kanban tasks in scope until the scope is clear"`

The Stop hook blocks stopping while ralph is active. Only `clear ralph` when the stop condition is met.

### Detect Projects

`/detected-projects` so we know what we are working with up front.

### Record progress (both modes)

Log each iteration / state transition — implement landed green in `doing`, checkpoint committed, review verdict, task stuck — on the task being driven.

{% include "_partials/record-progress" %}

### Single-task mode

Pin `<TASK_ID>` for the entire loop — never `next task`, never switch tasks.

1. **Verify exists**: `op: "get task", id: "<TASK_ID>"`. Missing → clear ralph and report.
2. **Implement**: `/implement <TASK_ID>`. Implement moves the task into `doing` (pulling it back from `review` if it's returning with findings), does the work, and — once really-done is green — **leaves it in `doing`**. Implement no longer moves tasks into `review`.
3. **Test**: `/test`. Failures → step 2.
4. **Checkpoint the green state**: invoke `/commit` to create a **local** commit of the green, tested working tree. This is the per-iteration rollback point and — critically — it is what makes the next review tight: with the work committed, the review scopes to *this iteration's commit*, not the whole accumulated uncommitted diff. **Commit only, NEVER push** (pushing is the user's separate step; per-task pushes would spam CI in batch mode). `/commit` stages all changes; "nothing to commit" is a no-op, not an error — but it means implement produced **no change this iteration** (no progress): record it and treat it under the step 7 guardrail rather than re-reviewing a stale diff.
5. **Review**: `/review <TASK_ID> HEAD~1..HEAD` — task-mode on `<TASK_ID>`, scoped to the checkpoint delta just committed (only this iteration's change, never the whole accumulated task diff). `/review` pulls the task `doing → review` and records findings on `<TASK_ID>`:
   - **clean** → task moves to `done`. Step 6.
   - **findings** → fresh dated `## Review Findings` checklist appended, task stays in `review`. Step 2 — `/implement <TASK_ID>` pulls it back to `doing`, works the unchecked items, and flips them to `- [x]`.
6. **Verify done**: `op: "get task"`. Not in `done` → step 2. In `done` → the last checkpoint (step 4) already **is** the verified-good commit (green + clean review); no separate post-done commit is needed.
7. **Guardrail**: same finding (file:line + message) across 3 iterations — or 3 consecutive no-change iterations (step 4 "nothing to commit") — → stop, clear ralph, report what persists. Hitting the guardrail means the task is **stuck**: leave it in `review` and report it — **never force it to `done`**. A finding that survives 3 rounds is either a fix you haven't cracked yet or a contradictory/faulty rule; if it's the latter (per Scope), report it on the task and leave it **stuck** for a human to resolve — do not edit validators yourself and do not re-close. Closing a task with open findings is out of bounds.
8. **Clear ralph** and report: task id, iterations, final test status, persistent findings.

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

2. **Drive it to done.** Run the **single-task mode loop** (steps 2–8 above) on `<TASK_ID>` in a sub agent. Reusing the loop means each iteration commits a local checkpoint via step 4, so by the time a task reaches `done` its verified-good state is already committed — before the next task is picked. Do not switch tasks mid-loop. A task that hits the guardrail is reported as stuck and skipped.

3. **Pick the next.** Return to step 1.

4. **Stop**: both the scoped `review` query and the scoped ready `todo` query return empty → `clear ralph` and report. **Tasks outside scope are deliberately ignored.**


## Constraints

### Delegation



- `/implement` per task — owns implementation; leaves the green task in `doing` (it does **not** move tasks into `review`). **Always sequential**, in both modes.
- `/test` after each implement verifies green.
- `/commit` after each green test — the per-iteration **checkpoint** commit. It both rolls back and scopes the next review (review targets the checkpoint delta). **Commit only, NEVER push**; pushing is the user's separate step (avoids per-task CI runs in batch mode). "Nothing to commit" is a no-op, not an error — and signals a no-change iteration.
- `/review <TASK_ID> HEAD~1..HEAD` after each checkpoint — pulls the task `doing → review` and drives `review → done` or back with fresh findings, scoped to the checkpoint delta.
- Don't pick tasks, write code, run tests, review, or run git yourself — delegate the commit to `/commit`.
- Stuck task → the step 7 guardrail handles it; in scoped-batch, report it stuck and move to the next task.

### Sequential safety (both modes)
- **One task at a time.** Never spawn parallel `Agent` subagents, never run concurrent `/implement` or `/review`. Scoped-batch picks one task, drives it to `done`, then picks the next.
- **No worktrees.** `isolation: "worktree"` loses changes — agents write to isolated copies never merged back. All work happens in the current tree.
- Parallel agents on the shared tree have repeatedly clobbered work via stash/revert races. If asked to "speed up" finish, say no — slow and correct beats fast and lost.

### Scope
- Do only what tasks say. No bonus refactoring — no **self-initiated** scope creep beyond the task and its review findings.
- **Review findings are in scope by definition.** A finding recorded by `/review` is work the task must address; acting on it is never "bonus refactoring." The no-bonus-refactoring rule restrains changes *you* invent — never the engine's findings.
- **Obey findings; never decline and never rewrite the rules.** A finding is an instruction. A task reaches `done` only through the review gate: a fresh `/review` returns zero new findings and every prior item is checked, and `/review` itself moves it. Do **not** force a task to `done` with `complete task` / `move task` while findings are open, do not "exercise orchestrator judgment" to dismiss them, and do **not** edit any validator to make a finding disappear — dismissing the order and rewriting the rulebook are both disobedience. Each finding is handled exactly one of two ways:
  1. **Fix the code at the root** — the default, and nearly always the answer. A finding names one instance of a cause; satisfy it by eliminating that cause across the whole file so a re-review of that file finds zero recurrences — not by patching only the cited line. Review is binary, like the test suite: any open finding means the task is **not done**, no matter how minor it looks — there is no severity tier that makes a finding optional. If findings feel like "churn" or "pedantry," that means the right fix hasn't been found yet, not that the finding is wrong. Re-review surfacing *new* findings each round is the engine working, not noise to wave off.
  2. **Report a contradiction — you cannot obey impossible orders.** Only when the findings genuinely cannot all be satisfied: two rules that can't both hold (a real contradiction), or a finding that demands code that won't compile / type-check, or that fights a deliberate documented contract (e.g. `snake_case` mirroring a backend payload, `null` required by a `T | null` type). Then **record the conflict on the task as a blocker, mark the task stuck, and stop.** You do not pick a winner, you do not touch `builtin/validators/…`, and you do not close. A human resolves the rule and re-runs.
- **"Data-driven" and "keep functions short" do not conflict.** A long function full of near-duplicate parallel branches is the *symptom* of not being data-driven; the fix — a spec table/map plus one generator or loop — is simultaneously shorter and DRY. If review flags both, satisfy both. Never decline one by citing the other; that is the agent failing to find the table-driven form, not a contradiction in the rules.
- Kanban is the single source of truth — no TodoWrite/TaskCreate.

### When done
- single-task: task id, iterations, final test status, persistent findings.
- scoped-batch: summary of all finished tasks + test results; report any stuck/skipped tasks.
