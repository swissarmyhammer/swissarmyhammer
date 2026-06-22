---
name: finish
profiles:
  - kanban
description: Drive kanban tasks from ready to done by looping implement → test → review until each task is clean. Use when the user says "/finish", "drive tasks to done", "work the board", "finish the tasks", "finish the batch", or otherwise wants to orchestrate tasks through the full pipeline to done. Supports single-task mode (one task id) and scoped-batch mode (all ready tasks in a tag, project, or filter).
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

Drive kanban tasks all the way to `done` — orchestrating `/implement`, `/test`, and `/review` in a loop until each task lands in `done` or is reported stuck.

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

Log each iteration / state transition — implement landed in `review`, tests run, review verdict, task stuck — on the task being driven.

{% include "_partials/record-progress" %}

### Single-task mode

Pin `<TASK_ID>` for the entire loop — never `next task`, never switch tasks.

1. **Verify exists**: `op: "get task", id: "<TASK_ID>"`. Missing → clear ralph and report.
2. **Implement**: `/implement <TASK_ID>` (moves through `doing` into `review`).
3. **Test**: `/test`. Failures → step 2 (implement agent will pick the task up again from `review`, moving it back to `doing`).
4. **Review**: `/review <TASK_ID>`:
   - **clean** → task moves to `done`. Step 5.
   - **findings** → fresh dated `## Review Findings` checklist appended, task stays in `review`. Step 2 — `/implement <TASK_ID>` works the unchecked items, flips them to `- [x]`, moves back to `review`.
5. **Verify done**: `op: "get task"`. Not in `done` → step 2.
6. **Commit the rollback point** (only once step 5 confirms `done`): invoke `/commit` to create a **local** commit of the verified-good state — green tests + clean review. This is a rollback point, not a publish: **commit only, NEVER push.** Pushing is the user's explicit, separate step; pushing per task would spam CI in batch mode. `/commit` reviews `git status` and stages all changes, so if the task produced no changes it is a no-op — "nothing to commit" is not an error, just skip ahead.
7. **Guardrail**: same finding (file:line + message) across 3 iterations → stop, clear ralph, report what persists.
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

2. **Drive it to done.** Run the **single-task mode loop** (steps 2–8 above) on `<TASK_ID>` in a sub agent. Reusing the loop means each finished task is committed locally via step 6 — one rollback-point commit per task, automatically — before the next is picked. Do not switch tasks mid-loop. A task that hits the guardrail is reported as stuck and skipped.

3. **Pick the next.** Return to step 1.

4. **Stop**: both the scoped `review` query and the scoped ready `todo` query return empty → `clear ralph` and report. **Tasks outside scope are deliberately ignored.**


## Constraints

### Delegation



- `/implement` per task — owns implementation and the move to `review`. **Always sequential**, in both modes.
- `/review` after each implement drives `review → done` or back with fresh findings.
- `/test` after each implement verifies green.
- `/commit` after a task is confirmed in `done` — creates the **local** rollback-point commit. **Commit only, NEVER push**; pushing is the user's separate step (avoids per-task CI runs in batch mode). "Nothing to commit" is a no-op, not an error.
- Don't pick tasks, write code, run tests, review, or run git yourself — delegate the commit to `/commit`.
- Stuck task → the step 7 guardrail handles it; in scoped-batch, report it stuck and move to the next task.

### Sequential safety (both modes)
- **One task at a time.** Never spawn parallel `Agent` subagents, never run concurrent `/implement` or `/review`. Scoped-batch picks one task, drives it to `done`, then picks the next.
- **No worktrees.** `isolation: "worktree"` loses changes — agents write to isolated copies never merged back. All work happens in the current tree.
- Parallel agents on the shared tree have repeatedly clobbered work via stash/revert races. If asked to "speed up" finish, say no — slow and correct beats fast and lost.

### Scope
- Do only what tasks say. No bonus refactoring.
- Kanban is the single source of truth — no TodoWrite/TaskCreate.

### When done
- single-task: task id, iterations, final test status, persistent findings.
- scoped-batch: summary of all finished tasks + test results; report any stuck/skipped tasks.
