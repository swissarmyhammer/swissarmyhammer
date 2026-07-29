---
name: kanban
profiles:
  - kanban
description: Execute the next task from the kanban board. Use this skill when the user says "kanban", "/kanban", "next task", "what's the next task", or "pick up work". It picks up the next ready task from the board and drives it through doing to review.
license: MIT OR Apache-2.0
compatibility: This skill needs the `kanban` MCP tool, for all board, column, and task operations.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Kanban

Execute the tasks from the board.

## Use Kanban for All Task Tracking

The kanban board is your to-do list. **Never use TodoWrite, TaskCreate, or any other task tool.** Use only `kanban`. It is the single source of truth across Claude Code and llama-agent sessions.

**Subtasks are GFM checklists** (`- [ ]` and `- [x]`) inside the `description` of the task. There is no separate subtask API. Include the subtasks when you create the task, or use `update task` to change the description.

{% include "_partials/short-ids" %}

## Process

1. **Get the next task**: `kanban` `op: "next task"` finds the next actionable task in every column except done.
   - Tag: `op: "next task", filter: "#bug"`
   - Assignee: `op: "next task", filter: "@alice"`
   - Combined: `op: "next task", filter: "#bug && @alice"`
2. **Move to doing**: `op: "move task", id: "<id>", column: "doing"`
3. **Read the details**: call `op: "get task", id: "<id>"`. Then review the earlier context, as shown in **Record progress** below.
4. **Work each subtask, check off immediately**:
   - Implement what it describes
   - Call `op: "update task", id: "<id>"`. Change `- [ ]` to `- [x]` for the finished subtask.
   - Do this after **every** subtask. Do not batch the updates. The checklist is the progress indicator.
   - Keep all other content in the description unchanged. Flip only the one checkbox you finished.
5. **Record progress**: log milestones, failures, and discoveries on the task. See **Record progress** below.
6. **Move the task to review** when every subtask is `- [x]`. First make sure the `review` column exists; this step is idempotent, so use the partial above. Then call `op: "move task", id: "<id>", column: "review"`. **Never use `complete task`.** It skips the review gate. After you move the task, stop, and tell the user that the task is ready for `/review`.

### Record progress

{% include "_partials/record-progress" %}

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
| Adjacent atoms | Implicit AND: `#bug @alice` means the same as `#bug && @alice` |

### Picking up work

Prefer `next task` with a filter. It returns one ready task, and excludes done tasks:

```json
{"op": "next task", "filter": "#bug"}
{"op": "next task", "filter": "@alice"}
{"op": "next task", "filter": "#bug && @alice"}
{"op": "next task", "filter": "$auth-migration"}
{"op": "next task", "filter": "$auth-migration && @alice"}
```

### Listing

**Never call `list tasks` with no parameters.** Always scope it with `filter` or `column`:

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

Each tag needs an `id`, a `name`, and a `color` (a 6-character hex code, with no `#`). The description is optional.

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

Required fields: `id` (a slug) and `name`. Optional fields: `description`, `color`, and `order`. If you omit `order`, the tool assigns the next number automatically, starting at 0. A duplicate `id` causes an error.

### Get, update, list, delete

```json
{"op": "get project", "id": "auth-migration"}
{"op": "update project", "id": "auth-migration", "name": "JWT Auth Migration"}
{"op": "update project", "id": "auth-migration", "description": "New desc", "color": "aabbcc", "order": 42}
{"op": "list projects"}
{"op": "delete project", "id": "auth-migration"}
```

`get project` returns `{id, name, description, color, order}`, or `ProjectNotFound`. `update` changes only the fields you provide. `list projects` returns `{projects, count}`, sorted by `order`. `delete project` **fails with `ProjectHasTasks`** if any task references the project. Reassign or complete those tasks first.

### Assigning and filtering

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
3. Use `$slug` in `list tasks` or `next task` to focus the view

## Guidelines

- You must finish every subtask. Do not skip one, and do not mark one complete without doing the work.
- If you are blocked, or something is unclear, record it on the task (see **Record progress** above).
- Run tests after each subtask
- Complete the task only when all subtasks are done and the tests pass
- Did you discover new work? Add a new kanban task. Do not hold it in your head.
