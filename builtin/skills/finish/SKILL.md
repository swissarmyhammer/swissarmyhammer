---
name: finish
description: Drive kanban tasks from ready to done by looping implement → test → review until each task is clean. Use when the user says "/finish", "drive tasks to done", "work the board", "finish the tasks", "finish the batch", or otherwise wants to orchestrate tasks through the full pipeline to done. Supports single-task mode (one task id) and scoped-batch mode (all ready tasks in a tag, project, or filter). Uses ralph to prevent stopping between iterations.
license: MIT OR Apache-2.0
compatibility: Requires the `kanban` and `ralph` MCP tools plus a Stop-hook-capable harness (e.g. Claude Code) so the declared Stop hook can re-invoke the agent across iterations.
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

Drive kanban tasks all the way to `done` — orchestrating `/implement`, `/test`, and `/review` in a loop until each task either lands in `done` or is reported stuck.

This skill is an **orchestrator**. It does not pick tasks, write code, or run tests itself. It delegates to `/implement`, `/review`, and `/test`, and uses `ralph` to stay alive between iterations.

The loop drives the full pipeline: `todo → doing → review → done`. `/implement` lands tasks in the `review` column (not `done`); `/review` drives them from `review` to `done` (clean) or back to `review` with fresh findings for another implement pass.

**Sequential only.** This skill never spawns parallel agents. Tasks are processed one at a time, in order. Parallel execution has caused lost work (stashing, reverting, races on shared trees) and is forbidden here.

## Invocation

`/finish` has two modes selected by the argument:

| Invocation | Mode | Meaning |
|------------|------|---------|
| `/finish <task-id>` (26-char ULID) | **single-task** | Drive exactly that task to done. Do NOT call `next task`. |
| `/finish` | **scoped-batch** (no scope) | Every ready task on the board, one at a time. |
| `/finish #<tag>` (e.g. `/finish #bug`) | **scoped-batch** | Tasks matching that tag, one at a time. |
| `/finish @<user>` | **scoped-batch** | Tasks assigned to that user, one at a time. |
| `/finish $<project-slug>` (e.g. `/finish $auth-migration`) | **scoped-batch** | Tasks in that project, one at a time. |
| `/finish <filter-expression>` (e.g. `/finish "#bug && @alice"`) | **scoped-batch** | Any filter DSL expression — applied to every `list tasks` call. |

Argument detection:

1. Argument matches a ULID pattern (26 chars, `[0-9A-Z]`) → **single-task mode**.
2. No argument → **scoped-batch mode**, no filter.
3. Otherwise → **scoped-batch mode**, argument passed verbatim as the filter.

Let `<SCOPE_FILTER>` denote the scoped-batch DSL expression (or absent). In every `list tasks` call below, combine `<SCOPE_FILTER>` with `#READY` (and any other structural constraint) using `&&`.

### Filter DSL recap

The DSL atoms: `#<tag>`, `@<user>`, `$<project-slug>`, `^<task-id>`, plus `&&`, `||`, `!`, and parens. Virtual tags `#READY`, `#BLOCKED`, `#BLOCKING` are available. All scoping — including project — flows through the filter DSL directly.

## Process

### Set ralph (both modes)

**First action**, before anything else:

```json
{"op": "set ralph", "instruction": "<mode-specific goal — see below>"}
```

- single-task: `"Finish task <TASK_ID> — loop until it lands in done"`
- scoped-batch: `"Finish all ready kanban tasks in scope until the scope is clear"`

The Stop hook blocks you from stopping while ralph is active. This is intentional — do not work around it. Only call `ralph` with `op: "clear ralph"` when the mode's stop condition is met.

### Single-task mode

Pin `<TASK_ID>` (the argument) for the entire loop — never call `next task`, never switch tasks.

1. **Verify the task exists**: `kanban` `op: "get task"`, `id: "<TASK_ID>"`. If it doesn't exist, clear ralph and report.

2. **Implement**: invoke `/implement <TASK_ID>`. `/implement` will move the task through `doing` into `review`.

3. **Test**: invoke `/test`. If there are failures, return to step 2 — the implement agent will address them (it can see the same workspace and will pick up the task again from `review` if needed, moving it back to `doing`).

4. **Review**: invoke `/review <TASK_ID>`. Either:
   - **clean** → task moves to `done`. Go to step 5.
   - **findings** → a fresh dated `## Review Findings` checklist is appended to the task description and it stays in `review`. Return to step 2 — `/implement <TASK_ID>` will work through the unchecked findings, flip them to `- [x]`, and move the task back to `review`.

5. **Verify done**: `kanban` `op: "get task"`, `id: "<TASK_ID>"`. If the task is not in `done`, return to step 2.

6. **Guardrail**: if the same review findings (same file:line + message) recur across 3 iterations, stop the loop. The task is stuck and needs human input. Clear ralph and report what persists.

7. **Clear ralph** and report: task id, iterations taken, final test status, any persistent findings.

### Scoped-batch mode

Process tasks **one at a time, sequentially**. Never spawn more than one implement or review at a time. Never use parallel `Agent` calls in this skill.

1. **Pick the next ready todo task in scope**: `kanban` `op: "list tasks"`, `column: "todo"`, with a `filter` combining `#READY` and the scope:
   - No scope → `filter: "#READY"`
   - Scope present → `filter: "#READY && (<SCOPE_FILTER>)"`

   Take the first task from the result. If the result is empty, skip ahead to step 5.

2. **Drive that one task through implement → test → review**, exactly like single-task mode:
   - `/implement <task-id>` (pinned to that id — never `next task`)
   - `/test`; if failures, re-run `/implement <task-id>` to address them
   - `/review <task-id>`; if findings, re-run `/implement <task-id>` to work through the checklist, then `/test`, then `/review <task-id>` again
   - Apply the same 3-iteration guardrail as single-task mode: if the same finding recurs 3 times, mark the task stuck and move on
   - Stop only when the task is in `done` (or marked stuck)

3. **Verify**: `kanban` `op: "get task"`, `id: "<task-id>"`. Confirm `column: "done"` (or recorded as stuck).

4. **Loop**: return to step 1 to pick the next ready todo task.

5. **Drain the review column (scoped, sequentially)**: `kanban` `op: "list tasks"`, `column: "review"`, with the same `<SCOPE_FILTER>` (or no filter if none). For each task in the result, **one at a time**, run `/review <task-id>`. If clean, it moves to `done`. If findings, run `/implement <task-id>` to address the checklist, then `/test`, then `/review <task-id>` again. Repeat until that task lands in `done` (or is marked stuck), then move to the next.

6. **Stop condition**: when both scoped queries (ready todo in scope AND review tasks in scope) return empty, `clear ralph` and report. **Tasks outside the scope are deliberately ignored** — the loop does not touch them even if they are ready.

## Examples

### Example 1: single-task mode — drive one task all the way to done

User says: `/finish 01KN2X3Y4Z5A6B7C8D9E0F1G2H`

Actions:
1. Argument matches the 26-char ULID pattern → single-task mode. Set ralph: `{"op": "set ralph", "instruction": "Finish task 01KN2X3Y4Z5A6B7C8D9E0F1G2H — loop until it lands in done"}`.
2. Verify the task exists with `{"op": "get task", "id": "01KN2X3Y4Z5A6B7C8D9E0F1G2H"}`.
3. Invoke `/implement 01KN2X3Y4Z5A6B7C8D9E0F1G2H`. The implement agent moves it through `doing` into `review`.
4. Invoke `/test`. Tests pass.
5. Invoke `/review 01KN2X3Y4Z5A6B7C8D9E0F1G2H`. Review returns one blocker: "missing auth check on /admin". The review skill appends a fresh `## Review Findings` section and leaves the task in `review`.
6. Loop back to step 3 — `/implement 01KN2X3Y4Z5A6B7C8D9E0F1G2H` reads the unchecked findings, addresses them, flips the boxes to `- [x]`, and moves the task back to `review`.
7. `/test` → `/review` again — clean this time. Task advances to `done`.
8. Verify `{"op": "get task"}` shows `column: "done"`. Clear ralph: `{"op": "clear ralph"}`. Report: 2 iterations, tests green.

Result: Single task driven from whatever starting column to `done`. Ralph kept the loop alive between steps; the guardrail would have stopped it if the same finding had recurred 3 times.

### Example 2: scoped-batch mode — finish all ready bugs sequentially

User says: `/finish #bug`

Actions:
1. Argument is not a ULID → scoped-batch mode with `<SCOPE_FILTER> = #bug`. Set ralph: `{"op": "set ralph", "instruction": "Finish all ready kanban tasks in scope until the scope is clear"}`.
2. Query ready todo tasks in scope: `{"op": "list tasks", "column": "todo", "filter": "#READY && (#bug)"}` → returns 3 ready bug tasks. Take the first.
3. Drive that one task through `/implement <task-id>` → `/test` → `/review <task-id>`. If review returns findings, re-run `/implement <task-id>` to work the checklist, then `/test`, then `/review <task-id>` again. Loop on this single task until it reaches `done` (or hits the 3-iteration stuck guardrail).
4. Re-query the ready todo list in scope. Take the next task. Repeat step 3. Continue one task at a time until the ready todo list in scope is empty.
5. Drain the review column in scope: `{"op": "list tasks", "column": "review", "filter": "#bug"}`. For each task, sequentially, run `/review <task-id>`; if findings, run `/implement <task-id>` then `/test` then `/review <task-id>` again until that one task lands in `done`. Then move to the next review task.
6. When both scoped queries return empty, `{"op": "clear ralph"}` and report: 3 tasks driven to done, any stuck tasks. Tasks outside `#bug` are deliberately untouched.

Result: Every ready bug on the board reaches `done` through the full implement → test → review pipeline, processed strictly one at a time, with the scope filter respected on every query.

## Constraints

### Delegation

- Use `/implement` for each task. Each owns implementation and moving the task into `review`.
- Use `/review` after each implement to drive the task from `review` to `done` (or back for another pass with fresh findings).
- Use `/test` after each implement to verify all tests pass.
- Do not pick tasks, write code, run tests, or review code yourself.
- If an agent reports it is stuck on a task, engage the guardrail (3-iteration rule) and move on — do not try to fix it yourself.

### No parallel execution

- **Never** spawn parallel `Agent` calls from this skill.
- **Never** issue more than one `/implement`, `/review`, or `/test` invocation at a time.
- **Never** create worktrees, stash changes, revert changes, or otherwise manipulate the working tree to enable concurrency.
- One task at a time, in order, in the current working tree. Period.

### Scope

- Do only what the tasks say. No bonus refactoring, no unrelated changes.
- The kanban board is the single source of truth. Do not use TodoWrite, TaskCreate, or other task tracking.

### When done

- single-task: report the task id, iterations taken, final test status, and any persistent findings.
- scoped-batch: present a summary of all tasks finished and their test results, in the order they were processed. Report any tasks that were marked stuck.
