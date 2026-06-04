---
name: plan
profiles:
  - kanban
description: Plan Mode workflow. Use this skill when the user says "/plan", "help me plan", "break this into tasks", "design the approach", or otherwise wants to plan work, and also whenever you are in Plan Mode. Drives all planning activity — research, task decomposition, and creating kanban tasks as the plan artifact.
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool for pre-plan research (symbol search, callgraph, blast-radius) and the `kanban` MCP tool for persisting the plan as kanban tasks. 
context: fork
agent: planner
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---



# Plan

Use whenever you enter Plan Mode or the user asks you to plan work.

$ARGUMENTS

## Goals

1. **Understand the work** — research deeply enough to know what changes and what's affected.
2. **Produce a kanban board** — the artifact is kanban tasks with subtasks. Not markdown. Not TodoWrite/TaskCreate/TaskUpdate.
3. **Right-size tasks** — each is one focused unit, independently implementable and verifiable.
4. **Collaborate** — present, discuss, iterate until the user is satisfied.
5. **Hand off cleanly** — when done, remind the user: `/finish` (autonomous) or `/implement` (one at a time).

## Example

**Feature request → decomposed board:** User says "I want to add authentication to the app".

1. Research with `code_context`: `search symbol "user"`, `search symbol "session"`, `get blastradius src/server.rs max_hops 3` to find boundaries and callers.
2. As design crystallizes in conversation, create tasks one at a time — not as an end-of-discussion batch:
   - `add task "Design auth architecture"` — design task, no code tests
   - `add task "Add User model and migration"` — model + migration
   - `add task "Implement POST /api/login"` with `depends_on: [<user-model-id>]`
3. Encode ordering with `depends_on` so foundational tasks precede integration.
4. Present the board, iterate.
5. User approves → remind: `/finish` (autonomous) or `/implement` (one at a time). Do NOT call `ExitPlanMode`, do NOT start implementing.

The board IS the plan — no markdown plan file.

## Constraints

{% include "_partials/architecture-awareness" %}

### Plans are kanban tasks — created as you go

Every planned item becomes a kanban task. The board IS the plan; no markdown files. **Create tasks as they crystallize during discussion, not at the end.** If a work item is defined enough to describe in conversation, it's defined enough to be a task. Don't wait to be asked.

### Research before tasks

`code_context` is primary. Always run `get blastradius` on files you expect to change — that's how you find downstream work you'd otherwise miss. Use symbol search, callgraphs, and text search (Glob/Grep/Read) to fill in the picture.

{% include "_partials/task-standards" %}

### Board naming

Name the board for the workspace/repository, not the feature being planned.

### User controls plan-mode exit

Do NOT call `ExitPlanMode`. The user decides when the plan is ready.

### No auto-implementation on exit

When the user exits plan mode or approves, do NOT begin implementing. Remind:
- `/finish` — drives tasks to `done` (implement → test → review) autonomously
- `/implement` — one task at a time

### Ordering

Foundational changes (data models, types, config) → core logic → integration → tests → cleanup. Use `depends_on` for ordering constraints.

## Autonomous Agent Mode

No Plan Mode UI or TUI? Follow `references/PLANNING_GUIDE.md`.

## Updating an Existing Plan

Update kanban directly — add tasks, `update task` to edit, `delete task` to remove, reorder dependencies. The board is a living document.
