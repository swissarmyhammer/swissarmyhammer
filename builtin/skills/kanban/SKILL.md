---
name: kanban
profiles:
  - kanban
description: Execute the next task from the kanban board. Use when the user says "kanban", "/kanban", "next task", "what's the next task", or "pick up work". Picks up the next ready task from the board and drives it through doing to review.
license: MIT OR Apache-2.0
compatibility: Requires the `kanban` MCP tool for all board, column, and task operations. 
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

{% include "_partials/review-column" %}

# Kanban

Execute the tasks from the board.

## Use Kanban for All Task Tracking

The kanban board is your todo list. **Never use TodoWrite, TaskCreate, or any other task tool** — only `kanban`. This is the single source of truth across Claude Code and llama-agent sessions.

**Subtasks are GFM checklists** (`- [ ]` / `- [x]`) inside the task's `description`. There is no separate subtask API — include them when creating the task, or `update task` to modify the description.

{% include "_partials/short-ids" %}

## Process

1. **Get next task**: `kanban` `op: "next task"` finds the next actionable task across all non-done columns.
   - Tag: `op: "next task", filter: "#bug"`
   - Assignee: `op: "next task", filter: "@alice"`
   - Combined: `op: "next task", filter: "#bug && @alice"`
2. **Move to doing**: `op: "move task", id: "<id>", column: "doing"`
3. **Read details**: `op: "get task", id: "<id>"`
4. **Work each subtask, check off immediately**:
   - Implement what it describes
   - `op: "update task", id: "<id>"`, change `- [ ]` → `- [x]` for the finished subtask
   - After EVERY subtask — never batch. The checklist is the progress indicator.
   - Preserve all other description content; only flip the one checkbox you finished.
5. **Move to review** when all subtasks are `- [x]`: first ensure the `review` column exists (idempotent — use the partial above), then `op: "move task", id: "<id>", column: "review"`. **Never use `complete task`** — that skips the review gate. After moving, stop and tell the user the task is ready for `/review`.

## Filtering Work

### Filter DSL

| Syntax | Meaning |
|--------|---------|
| `#tag` | Tasks with this tag (incl. virtual: READY, BLOCKED, BLOCKING) |
| `$project-slug` | Tasks assigned to this project |
| `@user` | Tasks assigned to this user |
| `^task-id` | Tasks referencing this id (via depends_on or own id) |
| `&&` / `and` | Both sides |
| `\|\|` / `or` | Either side |
| `!` / `not` | Negate |
| `()` | Grouping |
| Adjacent atoms | Implicit AND: `#bug @alice` ≡ `#bug && @alice` |

### Picking up work

Prefer `next task` with a filter — returns one ready task, excludes done:

```json
{"op": "next task", "filter": "#bug"}
{"op": "next task", "filter": "@alice"}
{"op": "next task", "filter": "#bug && @alice"}
{"op": "next task", "filter": "$auth-migration"}
{"op": "next task", "filter": "$auth-migration && @alice"}
```

### Listing

**Never call `list tasks` with no parameters** — always scope by `filter` or `column`:

```json
{"op": "list tasks", "column": "todo"}
{"op": "list tasks", "filter": "#bug"}
{"op": "list tasks", "filter": "#READY"}
{"op": "list tasks", "filter": "#bug && @alice"}
{"op": "list tasks", "filter": "#bug || #feature"}
{"op": "list tasks", "filter": "!#done && #READY"}
{"op": "list tasks", "filter": "$auth-migration"}
{"op": "list tasks", "filter": "$auth-migration && #bug"}
{"op": "list tasks", "filter": "$auth-migration || $frontend"}
```

`list tasks` excludes done unless you ask for `column: "done"`.

### Setting up tags

```json
{"op": "add tag", "id": "bug", "name": "Bug", "color": "ff0000", "description": "Bug fixes"}
{"op": "add tag", "id": "feature", "name": "Feature", "color": "00cc00"}
{"op": "add tag", "id": "chore", "name": "Chore", "color": "888888"}
```

Each tag needs `id`, `name`, `color` (6-char hex, no `#`). Description optional.

### Applying tags

```json
{"op": "add task", "title": "Fix login crash", "tags": ["bug"]}
{"op": "tag task", "id": "<id>", "tag": "feature"}
{"op": "untag task", "id": "<id>", "tag": "chore"}
```

### Managing tags

```json
{"op": "list tags"}
{"op": "update tag", "id": "bug", "name": "Bugfix", "color": "cc0000"}
{"op": "delete tag", "id": "chore"}
```

Deleting a tag removes it from all tasks.

## Projects

Group related tasks under a shared initiative.

### Create

```json
{"op": "add project", "id": "auth-migration", "name": "Auth Migration"}
{"op": "add project", "id": "frontend", "name": "Frontend", "description": "Frontend redesign", "color": "ff0000", "order": 5}
```

Required: `id` (slug), `name`. Optional: `description`, `color`, `order`. Omitting `order` auto-increments (first → 0). Duplicate `id` errors.

### Get / update / list / delete

```json
{"op": "get project", "id": "auth-migration"}
{"op": "update project", "id": "auth-migration", "name": "JWT Auth Migration"}
{"op": "update project", "id": "auth-migration", "description": "New desc", "color": "aabbcc", "order": 42}
{"op": "list projects"}
{"op": "delete project", "id": "auth-migration"}
```

`get project` returns `{id, name, description, color, order}` or `ProjectNotFound`. `update` only touches provided fields. `list projects` returns `{projects, count}` sorted by `order`. `delete project` **fails with `ProjectHasTasks`** if any task references it — reassign or complete first.

### Assigning / filtering

```json
{"op": "add task", "title": "Implement JWT refresh", "project": "auth-migration"}
{"op": "update task", "id": "<id>", "project": "frontend"}
```

Tasks without a project have `"project": ""`. Filter with `$slug`:

```json
{"op": "next task", "filter": "$auth-migration"}
{"op": "list tasks", "filter": "$auth-migration && #bug"}
{"op": "list tasks", "filter": "$auth-migration || $frontend"}
{"op": "list tasks", "filter": "!$auth-migration"}
```

### Workflow for a multi-task plan

1. Create a project for the initiative
2. Create tasks with `project` set
3. Use `$slug` in `list tasks` / `next task` to focus

## Guidelines

- Every subtask must be done — never skip or mark complete without doing the work
- Blocked or unclear → add a comment explaining
- Run tests after each subtask
- Only complete the task when all subtasks are done and tests pass
- New work discovered? Add a new kanban task — don't hold it in your head
