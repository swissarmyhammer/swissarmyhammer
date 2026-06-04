# Planning Guide for Autonomous Agents

This guide describes how to create a high-quality implementation plan when operating as an autonomous coding agent without a host IDE's planning mode.

## Basis for the plan

The plan may be driven by a free-form description, a file, or both:

- **A basis file** — if the request names or references a file (a path, an `@`-mention, or "plan from <file>"), read it first with `Read` and treat its contents as the authoritative basis. Follow relevant file references inside it.
- **A description** — plan from it directly.
- **Both** — the description refines or scopes the file.

The basis tells you *what* to build; the research below tells you *what's affected*. Always do both.

## Goals

1. **Understand the work** — research the codebase deeply enough to know what needs to change and what will be affected.
2. **Produce a kanban board** — the plan artifact is kanban tasks with subtasks. Not a markdown document.
3. **Right-size the tasks** — each task is a single focused unit of work that can be independently implemented and verified.
4. **Collaborate with the user** — present tasks, discuss, iterate, and refine until the user is satisfied.
5. **Hand off cleanly** — when planning is complete, remind the user they can execute with `/finish` (autonomous) or `/implement` (one task at a time). Do NOT begin implementing.

## Constraints

### Research thoroughly before creating tasks

Use `code_context` as the primary research tool:

- **Check index health** — `op: "get status"`. If incomplete, trigger with `op: "rebuild index"`.
- **Detect projects** — `op: "detect projects"` for build commands and language guidelines.
- **Find symbols** — `op: "search symbol"` with domain keywords, `op: "get symbol"` for implementations, `op: "list symbols"` for file structure.
- **Map blast radius** — `op: "get blastradius"` on every file you expect to change. This is the most important research step — it reveals callers, downstream consumers, tests, and transitive dependencies. If blast radius is large, consider scoping the change more narrowly.
- **Trace call chains** — `op: "get callgraph"` with `direction: "inbound"` and `"outbound"` to understand execution flow.
- **Check recent history** — `git` with `op: "get changes"` on affected files.
- **Fall back to text search** — Glob, Grep, Read for string literals, config files, or patterns not in the index.

### Create tasks with the `kanban` tool — the board is the only artifact

The plan exists ONLY as kanban tasks created via the `kanban` tool. This is the deliverable, not a side effect.

**Never write a markdown plan file** (`PLAN.md`, `DRAFT_PLAN.md`, a scratch file under `.swissarmyhammer/tmp/`, or similar). A markdown document is not a plan — `/finish` and `/implement` read the kanban board, not prose. If the `kanban` tool is unavailable or its calls fail, STOP and tell the user; do NOT fall back to writing markdown and do NOT claim tasks were created.

Concretely:

1. **Ensure a board exists** — `kanban` `{"op": "init board", "name": "<repo/workspace name>"}`. (`add task` auto-creates one, but naming it explicitly is better.)
2. **Create one task per work item, as it crystallizes** — not batched at the end:
   `kanban` `{"op": "add task", "title": "Add User model and migration", "description": "## What\n…\n## Acceptance Criteria\n- [ ] …\n## Tests\n- [ ] …", "depends_on": ["<prior-task-id>"]}`
   The `description` MUST follow the Task Standards template below (What / Acceptance Criteria / Tests / Workflow).
3. **Capture each returned task id** to wire `depends_on` on later tasks.
4. **Verify before claiming done** — call `kanban` `{"op": "list tasks"}` and confirm the tasks actually exist. Never report a plan as complete without this read-back.

{% include "_partials/task-standards" %}

### Board naming
Name the board for the workspace/repository, not the specific feature.

### Ordering
Foundational changes first (data models, types, config), then core logic, then integration, then tests, then cleanup. Use `depends_on` for ordering constraints.

### Risks and open questions
Track unresolved questions as kanban tasks so they stay visible.

### No auto-implementation
When the plan is approved, do NOT begin implementing. Remind the user:
- `/finish` — drive tasks all the way to `done` (implement → test → review) autonomously
- `/implement` — implement one task at a time

### Anti-patterns to avoid

- **Skipping blast radius** — leads to missed downstream work and surprise breakage.
- **Skipping exploration** — jumping to tasks without reading code leads to wrong assumptions.
- **Unbounded searches** — scope to specific directories, not `**/*.rs`.
- **Vague tasks** — every task needs concrete, verifiable subtasks.
- **Mega-tasks** — more than 5 subtasks or 5 files means split it.
- **Missing dependencies** — tasks that assume prior work but don't declare it.
- **Missing tests and acceptance criteria** — every task needs both.
- **Writing a markdown plan file** — the kanban board is the only artifact; `/finish` and `/implement` consume kanban tasks, not prose. A `PLAN.md`/`DRAFT_PLAN.md` is a failure, not a plan.
- **Claiming tasks without creating them** — always `add task` via the `kanban` tool and read them back with `list tasks` before reporting the plan complete.
