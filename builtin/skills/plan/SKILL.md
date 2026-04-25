---
name: plan
description: Plan Mode workflow. Use this skill when the user says "/plan", "help me plan", "break this into tasks", "design the approach", or otherwise wants to plan work, and also whenever you are in Plan Mode. Drives all planning activity — research, task decomposition, and creating kanban tasks as the plan artifact.
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool for pre-plan research (symbol search, callgraph, blast-radius) and the `kanban` MCP tool for persisting the plan as kanban tasks. 
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

{% include "_partials/coding-standards" %}

# Plan

Use this skill whenever you enter Plan Mode or the user asks you to plan work.

## Goals

1. **Understand the work** — research the codebase deeply enough to know what needs to change and what will be affected.
2. **Produce a kanban board** — the plan artifact is kanban tasks with subtasks. Not a markdown document, not built-in task tools (TodoWrite, TaskCreate, TaskUpdate).
3. **Right-size the tasks** — each task is a single focused unit of work that can be independently implemented and verified.
4. **Collaborate with the user** — present tasks, discuss, iterate, and refine until the user is satisfied with the plan.
5. **Hand off cleanly** — when planning is complete, remind the user they can execute with `/finish` (autonomous) or `/implement` (one task at a time).

## Examples

### Example 1: a feature request turns into a decomposed kanban board

User says: "I want to add authentication to the app"

Actions:
1. Research with `code_context`: `{"op": "search symbol", "query": "user"}`, `{"op": "search symbol", "query": "session"}`, and `{"op": "get blastradius", "file_path": "src/server.rs", "max_hops": 3}` to find existing boundaries and callers that will need to participate.
2. As the design crystallizes in conversation, create tasks on the kanban board one at a time — not as a batch at the end:
   - `{"op": "add task", "title": "Design auth architecture", "description": "What: Decide JWT vs session, storage strategy. Acceptance Criteria: strategy documented in task comments; token format and expiry policy decided. Tests: no code tests — design task."}`
   - `{"op": "add task", "title": "Add User model and migration", "description": "What: Add users table and User struct in src/models/user.rs..."}`
   - `{"op": "add task", "title": "Implement POST /api/login", "description": "What: ...", "depends_on": ["<user-model-task-id>"]}`
3. Encode ordering with `depends_on` so foundational tasks (model, migration) come before integration (login endpoint).
4. Present the resulting board to the user, iterate on titles/descriptions/ordering.
5. When the user approves the plan, remind them: use `/finish` to drive the batch autonomously, or `/implement` for one task at a time. Do NOT call `ExitPlanMode` or start implementing.

Result: A kanban board with foundational → integration → test tasks, each independently implementable. The board IS the plan — no markdown plan file was created.

## Constraints

### Plans are kanban tasks — created as you go
Every planned work item becomes a kanban task. The kanban board IS the plan. No markdown plan files. **Create tasks as they crystallize during discussion, not as a batch at the end.** If a work item is defined enough to describe in conversation, it is defined enough to be a task. Don't wait for the user to ask for tasks — the act of planning IS creating tasks.

### Research before tasks
Use `code_context` as the primary research tool. Always check blast radius (`op: "get blastradius"`) on files you expect to change — this is how you discover downstream work you'd otherwise miss. Use symbol search, call graphs, and text search (Glob/Grep/Read) to fill in the picture.

{% include "_partials/task-standards" %}

### Board naming
Name the board for the workspace/repository, not the specific feature being planned.

### User controls plan mode exit
Do NOT call ExitPlanMode yourself. The user decides when the plan is ready.

### No auto-implementation on exit
When the user exits plan mode or approves the plan, do NOT begin implementing. Instead, remind them:
- Use `/finish` to drive tasks all the way to `done` (implement → test → review) autonomously
- Use `/implement` to implement one task at a time

### Ordering
Foundational changes come first (data models, types, configuration), then core logic, then integration, then tests, then cleanup. Use `depends_on` to encode ordering constraints between tasks.

## Autonomous Agent Mode

When operating as an autonomous agent (no Plan Mode UI), follow the `references/PLANNING_GUIDE.md` resource file bundled with this skill.

## Updating an Existing Plan

Update kanban tasks directly — add new tasks, update existing ones with `op: "update task"`, remove obsolete ones with `op: "delete task"`, and reorder dependencies as needed. The board is a living document.
