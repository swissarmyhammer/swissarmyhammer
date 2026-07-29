---
name: finish
profiles:
  - kanban
description: Drive kanban tasks from ready to done. The skill loops implement → test → commit → review until each task is clean. Use this skill when the user says "/finish", "drive tasks to done", "work the board", "finish the tasks", "finish the batch", or otherwise wants to orchestrate tasks through the full pipeline to done. The skill supports single-task mode (one task id) and scoped-batch mode (all ready tasks in a tag, project, or filter).
license: MIT OR Apache-2.0
compatibility: This skill requires the `kanban` and `ralph` MCP tools, plus a Stop-hook-capable harness.
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

Drive kanban tasks all the way to `done`. Orchestrate `/implement`, `/test`, `/commit`, and `/review` in a loop until each task lands in `done` or is reported stuck.

**Orchestrator only.** This skill does not pick tasks, write code, run tests, or commit. It delegates to `/implement`, `/review`, `/test`, and `/commit`. It uses `ralph` to stay alive between iterations.

**IMPORTANT:** run each skill-driven step in an appropriate subagent. This keeps context bloat low in this session.


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

Let `<SCOPE_FILTER>` be the DSL expression, or absent if there is none. Combine it with `#READY` using `&&` on every scoped `list tasks`.

### Filter DSL recap

Atoms: `#<tag>`, `@<user>`, `$<project-slug>`, `^<task-id>`. Operators: `&&`, `||`, `!`, `()`. Virtual tags: `#READY`, `#BLOCKED`, `#BLOCKING`. All scoping, including project scoping, flows through the filter.

The `^<task-id>` atom, and every id argument, accepts a full ULID, a 7-char short id, `^<short>`, or a unique ULID prefix. When you report on a task in prose, quote its `short_id` field (`^<short>`). Do not hand-abbreviate the ULID by prefix.

## Process

### Set ralph (both modes)

**First action**:

```json
{"op": "set ralph", "instruction": "<mode-specific goal>"}
```

- single-task: `"Finish task <TASK_ID> — loop until it lands in done"`
- scoped-batch: `"Finish all ready kanban tasks in scope until the scope is clear"`

The Stop hook blocks stopping while ralph is active. Call `clear ralph` only when the stop condition is met.

### Detect Projects

Run `/detected-projects` first. This tells you what you are working with.

### Record progress (both modes)

Log each iteration and state transition on the task being driven: implement landed green in `doing`, checkpoint committed, review verdict, or task stuck.

{% include "_partials/record-progress" %}

### Single-task mode

Pin `<TASK_ID>` for the entire loop. Never call `next task`. Never switch tasks.

1. **Verify the task exists**: call `op: "get task", id: "<TASK_ID>"`. If missing, clear ralph and report.
2. **Implement**: run `/implement <TASK_ID>`. Implement moves the task into `doing`, pulling it back from `review` if it is returning with findings. It does the work, and once really-done is green, **leaves the task in `doing`**. Implement no longer moves tasks into `review`.
3. **Test**: run `/test`. On failures, return to step 2.
4. **Checkpoint the green state**: invoke `/commit` to create a **local** commit of the green, tested working tree. This is the per-iteration rollback point. It also keeps the next review tight: with the work committed, the review scopes to *this iteration's commit*, not the whole accumulated uncommitted diff. **Commit only; NEVER push.** Pushing is the user's separate step; per-task pushes would spam CI in batch mode. `/commit` stages all changes. "Nothing to commit" is a no-op, not an error, but it means implement produced **no change this iteration** — no progress. Record it, and treat it under the step 7 guardrail rather than re-reviewing a stale diff.
5. **Review**: run `/review <TASK_ID> HEAD~1..HEAD` — task mode on `<TASK_ID>`, scoped to the checkpoint delta just committed. This is only this iteration's change, never the whole accumulated task diff. `/review` pulls the task from `doing` to `review` and records findings on `<TASK_ID>`:
   - **Clean** → the task moves to `done`. Go to step 6.
   - **Findings** → a fresh dated `## Review Findings` checklist is appended, and the task stays in `review`. Go to step 2 — `/implement <TASK_ID>` pulls it back to `doing`, works the unchecked items, and flips them to `- [x]`.
6. **Verify done**: call `op: "get task"`. If not in `done`, return to step 2. If in `done`, the last checkpoint from step 4 already **is** the verified-good commit — green and a clean review. No separate post-done commit is needed.
7. **Guardrail**: if the same finding (file:line plus message) persists across 3 iterations, or 3 consecutive iterations produce no change (step 4 "nothing to commit"), stop, clear ralph, and report what persists. Hitting the guardrail means the task is **stuck**. Leave it in `review` and report it; **never force it to `done`**. A finding that survives 3 rounds is either a fix you have not cracked yet, or a contradictory or faulty rule. If it is the latter (per Scope), report it on the task and leave it **stuck** for a human to resolve. Do not edit validators yourself, and do not re-close the task. Closing a task with open findings is out of bounds.
8. **Clear ralph** and report the task id, the number of iterations, the final test status, and any persistent findings.

### Scoped-batch mode

**Strictly sequential — one task at a time.** Never use worktrees. Never run concurrent `/implement` or `/review`. Pick one task, drive it fully to `done` using the exact single-task loop, then pick the next. Parallel work on the shared working tree has repeatedly clobbered changes through stash and revert races. The slowness of sequential runs costs far less than lost work.

1. **Pick one task in scope.** First check the `review` column, then the ready `todo` column. A task already in `review` is closer to done, so finish it first:
   - `op: "list tasks", column: "review"`, `filter`:
     - No scope → absent
     - Scope → `"<SCOPE_FILTER>"`
   - `op: "list tasks", column: "todo"`, `filter`:
     - No scope → `"#READY"`
     - Scope → `"#READY && (<SCOPE_FILTER>)"`

   Leave tasks in `doing` alone; they are already being worked. Take the **first** task from `review`, if any. Otherwise take the first ready `todo` task. Pin its id as `<TASK_ID>`.

2. **Drive it to done.** Run the **single-task mode loop** (steps 2–8 above) on `<TASK_ID>` in a subagent. Reusing the loop means each iteration commits a local checkpoint through step 4, so by the time a task reaches `done` its verified-good state is already committed — before the next task is picked. Do not switch tasks mid-loop. Report a task that hits the guardrail as stuck, and skip it.

3. **Pick the next.** Return to step 1.

4. **Stop**: when both the scoped `review` query and the scoped ready `todo` query return empty, call `clear ralph` and report. **Tasks outside scope are deliberately ignored.**


## Constraints

### Delegation



- `/implement` runs per task. It owns implementation and leaves the green task in `doing`; it does **not** move tasks into `review`. **Always sequential**, in both modes.
- `/test` runs after each implement to verify the task is green.
- `/commit` runs after each green test, as the per-iteration **checkpoint** commit. It both provides a rollback point and scopes the next review, since review targets the checkpoint delta. **Commit only, NEVER push**; pushing is the user's separate step (avoids per-task CI runs in batch mode). "Nothing to commit" is a no-op, not an error — and signals a no-change iteration.
- `/review <TASK_ID> HEAD~1..HEAD` runs after each checkpoint. It pulls the task from `doing` to `review`, then drives it from `review` to `done`, or sends it back with fresh findings, scoped to the checkpoint delta.
- Do not pick tasks, write code, run tests, review, or run git yourself — delegate the commit to `/commit`.
- A stuck task is handled by the step 7 guardrail; in scoped-batch, report it stuck and move to the next task.

### Sequential safety (both modes)
- **One task at a time.** Never spawn parallel `Agent` subagents, never run concurrent `/implement` or `/review`. Scoped-batch picks one task, drives it to `done`, then picks the next.
- **No worktrees.** `isolation: "worktree"` loses changes, because agents write to isolated copies that are never merged back. Do all work in the current tree.
- Parallel agents on the shared tree have repeatedly clobbered work through stash and revert races. If asked to "speed up" finish, say no — slow and correct beats fast and lost.

### Scope
- Do only what the task says. No bonus refactoring — no **self-initiated** scope creep beyond the task and its review findings.
- **Review findings are in scope by definition.** A finding recorded by `/review` is work the task must address; acting on it is never "bonus refactoring." The no-bonus-refactoring rule restrains changes *you* invent — never the engine's findings.
- **Obey findings; never decline and never rewrite the rules.** A finding is an instruction. A task reaches `done` only through the review gate: a fresh `/review` returns zero new findings, every prior item is checked, and `/review` itself moves the task. Do **not** force a task to `done` with `complete task` or `move task` while findings are open, do not "exercise orchestrator judgment" to dismiss them, and do **not** edit any validator to make a finding disappear — dismissing the order and rewriting the rulebook are both disobedience. Handle each finding in exactly one of two ways:
  1. **Fix the code at the root** — the default, and nearly always the answer. A finding names one instance of a cause; satisfy it by eliminating that cause across the whole file so a re-review of that file finds zero recurrences — not by patching only the cited line. Review is binary, like the test suite: any open finding means the task is **not done**, no matter how minor it looks — there is no severity tier that makes a finding optional. If findings feel like "churn" or "pedantry," that means you have not found the right fix yet, not that the finding is wrong. When re-review surfaces *new* findings each round, that is the engine working, not noise to wave off.
  2. **Report a contradiction — you cannot obey impossible orders.** Use this only when the findings genuinely cannot all be satisfied: two rules that cannot both hold (a real contradiction), or a finding that demands code that will not compile or type-check, or that fights a deliberate documented contract (e.g. `snake_case` mirroring a backend payload, `null` required by a `T | null` type). Then **record the conflict on the task as a blocker, mark the task stuck, and stop.** Do not pick a winner, do not touch `builtin/validators/…`, and do not close the task. A human resolves the rule and re-runs.
- **"Data-driven" and "keep functions short" do not conflict.** A long function full of near-duplicate parallel branches is the *symptom* of not being data-driven; the fix — a spec table/map plus one generator or loop — is both shorter and DRY. If review flags both, satisfy both. Never decline one by citing the other; that means the agent failed to find the table-driven form, not that the rules contradict.
- Kanban is the single source of truth — do not use TodoWrite or TaskCreate.

### When done
- single-task: report the task id, the number of iterations, the final test status, and any persistent findings.
- scoped-batch: report a summary of all finished tasks and test results; report any stuck or skipped tasks.
