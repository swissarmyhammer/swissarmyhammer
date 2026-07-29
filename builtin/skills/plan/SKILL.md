---
name: plan
profiles:
  - kanban
description: A Plan Mode workflow. Use this skill when the user says "/plan", "help me plan", "break this into tasks", "design the approach", or otherwise wants to plan work, and also whenever you are in Plan Mode. It drives all planning activity — research, breaking work into tasks, and creating kanban tasks as the plan artifact.
license: MIT OR Apache-2.0
compatibility: This skill needs the `code_context` MCP tool, for research before planning — symbol search, callgraph, and blast-radius. It also needs the `kanban` MCP tool, to save the plan as kanban tasks.
agent: planner
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---



# Plan

Use whenever you enter Plan Mode or the user asks you to plan work.

$ARGUMENTS


## Interpreting the arguments

The arguments above may be a plain description of the work, a path to a file that is the basis for the plan, or both.

- **If the arguments name or point to a file** — for example a path like `docs/spec.md`, an `@`-mention, or "plan from <file>" — read that file first with `Read`. Treat its content as the main basis for the plan. Follow any further file references inside it that matter.
- **If the arguments are a description**, plan from the description directly.
- **If both are given**, the description narrows or refines what is in the file.

Either way, do the `code_context` research below before you create tasks. The basis file tells you *what* to build. The research tells you *what is affected*.

## Goals

1. **Understand the work.** Research deeply enough to know what changes, and what is affected.
2. **Produce a kanban board.** The plan artifact is kanban tasks with subtasks. Do not use markdown. Do not use TodoWrite, TaskCreate, or TaskUpdate.
3. **Right-size the tasks.** Each task is one focused unit. Each task must be possible to implement and verify on its own.
4. **Collaborate with the user.** Present the plan, discuss it, and revise it until the user is satisfied.
5. **Hand off cleanly.** When the plan is done, remind the user of two options: `/finish` to work autonomously, or `/implement` to work one task at a time.

## Example

**Feature request becomes a decomposed board:** The user says "I want to add authentication to the app".

1. Research with `code_context`: run `search symbol "user"` and `search symbol "session"`. Run `get callgraph` (inbound) on the symbols you expect to change, to find their callers. Run `get blastradius src/server.rs max_hops 3` when a change touches the signature of a shared symbol.
2. Make sure a board exists: `kanban` `{"op": "init board", "name": "<repo name>"}`. Note that `add task` creates one automatically, but you should still name it.
3. As the design takes shape in conversation, create tasks one at a time with the `kanban` tool. Do not create them as one batch at the end of the discussion. Each `description` must follow the Task Standards template: What, Acceptance Criteria, and Tests:
   - `{"op": "add task", "title": "Design auth architecture", "description": "## What\n…\n## Acceptance Criteria\n- [ ] …\n## Tests\n- [ ] …"}`
   - `{"op": "add task", "title": "Add User model and migration", "description": "## What\n…"}`
   - `{"op": "add task", "title": "Implement POST /api/login", "description": "…", "depends_on": ["<user-model-task-id>"]}`
4. Encode the order with `depends_on`, so foundational tasks come before integration tasks.
5. Verify with `{"op": "list tasks"}`. Present the board to the user, and revise it.
6. Before handoff, **double-check the board** (see below). Launch the `double-check` agent to critique it. Apply its REVISE findings once.
7. When the user approves, remind them of `/finish`, for autonomous work, or `/implement`, for one task at a time. Do not call `ExitPlanMode`. Do not start implementing.

The board is the plan. **Never write a markdown plan file**, for example `PLAN.md`, `DRAFT_PLAN.md`, or a scratch file. `/finish` and `/implement` read kanban, not prose. If the `kanban` tool is unavailable, or its calls fail, stop and tell the user. Do not use markdown instead. Do not claim that tasks exist without a `list tasks` read-back.

## Constraints

{% include "_partials/architecture-awareness" %}

### No Phases

Phases are a project management tool, not a planning tool. They encourage batch work and waterfall handoffs. Do not use them. The workflow is continuous: research, then task creation, then implementation, then testing, then review, then done, with feedback loops between each step.

The dependency graph of the tasks encodes the order you need. For example, a "Design auth architecture" task can be a dependency of "Implement POST /api/login", which can in turn be a dependency of "Write login tests". This allows natural parallel work and iteration, without strict phase boundaries.

### Plans are kanban tasks — created as you go

Every planned item becomes a kanban task. The board is the plan; do not use markdown files. **Create tasks as they take shape during discussion, not at the end.** If a work item is clear enough to describe in conversation, it is clear enough to be a task. Do not wait to be asked.

### Research before tasks

Use `code_context` as your main tool. Use symbol search, callgraphs (inbound, to find callers), and text search (Glob, Grep, Read) to build the picture. When a planned change touches the signature of a shared symbol, run `get blastradius` on the file. This surfaces downstream work that you would otherwise miss. It is built from LSP call edges, so treat an empty `edges: []` result as "LSP is not ready", not as "no impact" — and use inbound `get callgraph` instead.

{% include "_partials/task-standards" %}

### Board naming

Name the board for the workspace or repository, not for the feature you are planning.

### Double-check the board

After you build the board, and before you remind the user of `/finish` or `/implement`, double-check the board critically. Launch the `double-check` subagent against the tasks you just created. Ask it to try to prove that the plan is wrong or incomplete:

- Are the tasks **right-sized**? Is each one focused, and possible to implement and verify on its own?
- Are the **acceptance criteria verifiable**? Are they concrete and machine-checkable, not vague?
- Are the **dependencies and order** sound? Does foundational work come before integration? Are there no cycles?
- Is **anything from the stated intent missing**? Is there work that the request implies, but no task covers?

The agent returns a PASS or REVISE verdict. **Apply REVISE findings once.** Adjust, add, or reorder tasks with the `kanban` tool to address them, then move on to the handoff reminder. Do not loop: run one double-check pass, apply it, then hand off. This double-check works on the kanban board itself, because the board is the plan. It never produces or reads a markdown plan file.

### User controls plan-mode exit

Do not call `ExitPlanMode`. The user decides when the plan is ready.

### No auto-implementation on exit

When the user exits plan mode, or approves the plan, do not begin implementing. Remind the user:
- `/finish` — drives tasks to `done` autonomously, through implement, test, and review
- `/implement` — one task at a time

### Ordering

The order runs: foundational changes, such as data models, types, and config; then core logic; then integration; then tests; then cleanup. Use `depends_on` to encode order.

## Autonomous Agent Mode

Is there no Plan Mode UI or TUI, for example a headless `-p` run? The procedure above still applies: research, then create kanban tasks one at a time with the `kanban` tool, and verify with `list tasks`. Do not wait for a UI, and do not write a markdown plan file. When bundled, `references/PLANNING_GUIDE.md` has the long form of this guide, but this file has everything you need.

## Updating an Existing Plan

Update kanban directly. Add tasks with `add task`. Edit a task with `update task`. Remove a task with `delete task`. Reorder dependencies as needed. The board is a living document.
