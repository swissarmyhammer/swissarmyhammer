---
name: finish
profiles:
  - kanban
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

Drive kanban tasks all the way to `done` — orchestrating `/implement`, `/test`, and `/review` in a loop until each task lands in `done` or is reported stuck.

**Orchestrator only** — does not pick tasks, write code, or run tests. Delegates to `/implement`, `/review`, `/test`; uses `ralph` to stay alive between iterations.

Pipeline: `todo → doing → review → done`. `/implement` lands tasks in `review`; `/review` drives them to `done` (clean) or back to `review` with fresh findings.

## Invocation

| Invocation | Mode | Meaning |
|------------|------|---------|
| `/finish <task-id>` (26-char ULID) | **single-task** | Drive exactly that task. Never call `next task`. |
| `/finish` | **scoped-batch** (no scope) | All ready tasks. |
| `/finish #<tag>` | **scoped-batch** | Matching tag. |
| `/finish @<user>` | **scoped-batch** | Assigned to user. |
| `/finish $<project-slug>` | **scoped-batch** | In project. |
| `/finish <filter-expr>` | **scoped-batch** | Any filter DSL — applied to every `list tasks`. |

Detection:
1. ULID (26 chars, `[0-9A-Z]`) → single-task
2. No arg → scoped-batch, no filter
3. Otherwise → scoped-batch, arg passed verbatim as filter

Let `<SCOPE_FILTER>` be the DSL expression (or absent). Combine with `#READY` via `&&` on every scoped `list tasks`.

### Filter DSL recap

Atoms: `#<tag>`, `@<user>`, `$<project-slug>`, `^<task-id>`. Operators: `&&`, `||`, `!`, `()`. Virtual tags: `#READY`, `#BLOCKED`, `#BLOCKING`. All scoping (incl. project) flows through the filter.

## Process

### Set ralph (both modes)

**First action**:

```json
{"op": "set ralph", "instruction": "<mode-specific goal>"}
```

- single-task: `"Finish task <TASK_ID> — loop until it lands in done"`
- scoped-batch: `"Finish all ready kanban tasks in scope until the scope is clear"`

The Stop hook blocks stopping while ralph is active. Only `clear ralph` when the stop condition is met.

### Single-task mode

Pin `<TASK_ID>` for the entire loop — never `next task`, never switch tasks.

1. **Verify exists**: `op: "get task", id: "<TASK_ID>"`. Missing → clear ralph and report.
2. **Implement**: `/implement <TASK_ID>` (moves through `doing` into `review`).
3. **Test**: `/test`. Failures → step 2 (implement agent will pick the task up again from `review`, moving it back to `doing`).
4. **Review**: `/review <TASK_ID>`:
   - **clean** → task moves to `done`. Step 5.
   - **findings** → fresh dated `## Review Findings` checklist appended, task stays in `review`. Step 2 — `/implement <TASK_ID>` works the unchecked items, flips them to `- [x]`, moves back to `review`.
5. **Verify done**: `op: "get task"`. Not in `done` → step 2.
6. **Guardrail**: same finding (file:line + message) across 3 iterations → stop, clear ralph, report what persists.
7. **Clear ralph** and report: task id, iterations, final test status, persistent findings.

### Scoped-batch mode

1. **Ready todo in scope**: `op: "list tasks", column: "todo"`, `filter`:
   - No scope → `"#READY"`
   - Scope → `"#READY && (<SCOPE_FILTER>)"`

   Tasks in `doing` are already being worked; tasks in `review` are step 4.

2. **Implement the batch**: spawn parallel `Agent` subagents, one per task, each running `/implement <task-id>`. Send all Agent calls in a **single message** so they run concurrently. Each `/implement` will move its task into `review` — none to `done`.

3. **`/test`** after the batch.

4. **Review column (scoped)**: `op: "list tasks", column: "review"`, same `<SCOPE_FILTER>`. Spawn parallel `/review <task-id>` agents in a single message. Each either moves its task to `done` (clean + all prior items checked) or appends fresh dated findings and leaves it in `review`.

5. **Handle remaining**: any task still in scoped `review` has fresh unchecked `- [ ]` items. Dispatch parallel `/implement <task-id>` agents on each — they'll read the description, work the checklist, flip to `- [x]`, move back to `review`. Run `/test`, return to step 4.

6. **Loop** to step 1 until both queries (ready todo + review) return empty.

7. **Stop**: both empty → `clear ralph` and report. **Tasks outside scope are deliberately ignored.**

### Parallel Agent Prompt Template (scoped-batch only)

```
Run `/implement [TASK-ID]` on kanban task [TASK-ID]: [TASK-TITLE]

The explicit task id form pins `/implement` to this specific task — it will not call `next task`.
`/implement` will move the task through doing → review. Do NOT let it use `complete task`.

Task ID: [TASK-ID]
```

Each agent must target a specific task id. Never let parallel agents call `next task` — they'd race.

## Examples

**Single-task:** `/finish 01KN2X3Y4Z5A6B7C8D9E0F1G2H`.

1. ULID → single-task. Set ralph.
2. Verify task exists.
3. `/implement <id>` → moves through `doing` → `review`.
4. `/test` → pass.
5. `/review <id>` → 1 blocker (missing auth check on `/admin`). Fresh `## Review Findings` appended; task stays in `review`.
6. `/implement <id>` again → reads unchecked findings, addresses them, flips checkboxes, moves back to `review`.
7. `/test` → `/review` again — clean. Task → `done`.
8. Clear ralph. Report: 2 iterations, tests green.

**Scoped-batch:** `/finish #bug`.

1. Not a ULID → scoped-batch, `<SCOPE_FILTER> = #bug`. Set ralph.
2. `list tasks column: "todo" filter: "#READY && (#bug)"` → 3 ready bugs.
3. Spawn 3 parallel Agents in one message, each pinned to a task id.
4. `/test` → all green.
5. `list tasks column: "review" filter: "#bug"` → parallel `/review <id>` agents. Two → `done`; one gets findings, stays in `review`.
6. `/implement` on the remaining one to work the checklist, then `/test`, then re-review.
7. Loop until both queries empty.
8. Clear ralph, report. Tasks outside `#bug` untouched.

## Constraints

### Delegation
- `/implement` per task (sequential in single-task, parallel via Agent in scoped-batch) — owns implementation and the move to `review`.
- `/review` after each implement batch drives `review → done` or back with fresh findings.
- `/test` after each implement batch verifies green.
- Don't pick tasks, write code, run tests, or review yourself.
- Stuck agent → move on. In single-task mode, the step 6 guardrail handles it.

### Parallel safety (scoped-batch)
- **Max 4 concurrent agents.**
- **No worktrees.** `isolation: "worktree"` loses changes — agents write to isolated copies never merged back. All agents work in the current tree.
- Parallel failure → continue with the others, report at the end.

### Scope
- Do only what tasks say. No bonus refactoring.
- Kanban is the single source of truth — no TodoWrite/TaskCreate.

### When done
- single-task: task id, iterations, final test status, persistent findings.
- scoped-batch: summary of all finished tasks + test results; note parallel vs sequential; report failures/skipped.
