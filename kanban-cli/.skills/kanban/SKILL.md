---
name: kanban
description: Execute the next task from the kanban board. Use when the user wants to make progress on planned work by implementing the next available todo item.
metadata:
  author: swissarmyhammer
  version: 0.12.11
---

{% include "_partials/review-column" %}

# Do

Pick up and execute the next task from the kanban board.

## Important: Use Kanban for All Task Tracking

The kanban board is your todo list. Do NOT use any built-in task or todo tools (like TodoWrite or TaskCreate) — always use the `kanban` tool instead. Every task and work item belongs on the kanban board. This is how work is tracked across both Claude Code and llama-agent sessions, so it must be the single source of truth.

**Subtasks are GitHub Flavored Markdown checklists** inside the task's `description` field. There is no separate "add subtask" API — subtasks live in the description as `- [ ]` / `- [x]` items. To add subtasks, include them when creating the task or use `update task` to modify the description.

When the user asks you to track work, create a todo list, or remember tasks — use kanban tasks, not any other mechanism.

## Process

1. Get the next task: use `kanban` with `op: "next task"` to find the next actionable task. This searches all non-done columns for ready tasks.
   - To filter by tag: `op: "next task"`, `filter: "#bug"`
   - To filter by assignee: `op: "next task"`, `filter: "@alice"`
   - To combine: `op: "next task"`, `filter: "#bug && @alice"`
2. Move it to doing: use `kanban` with `op: "move task"`, `id: "<task-id>"`, `column: "doing"`
3. Read the task details: use `kanban` with `op: "get task"`, `id: "<task-id>"` to see description and subtasks
4. **Work through each subtask and check it off immediately**:
   - Implement what the subtask describes
   - **Mark it complete right away**: use `kanban` with `op: "update task"`, `id: "<task-id>"`, and update the `description` to change `- [ ]` to `- [x]` for the completed subtask
   - Do this after EVERY subtask — not in a batch at the end. The checklist is the progress indicator; leaving boxes unchecked while doing work defeats the purpose.
   - When updating the description, preserve all existing content (other checklist items, prose, etc.) — only flip the one checkbox you just finished.
5. **Move the task to review**: when ALL subtasks are done (every `- [ ]` is now `- [x]`), the task is ready for code review — not directly for `done`. First ensure the `review` column exists using the **Ensure the Review Column Exists** partial above (idempotent — run every time), then move the task there with `kanban` using `op: "move task"`, `id: "<task-id>"`, `column: "review"`. You MUST do this — never leave a task in "doing" when the work is finished. **Do NOT use `complete task`** — that skips the review gate by jumping to the terminal column. After moving to `review`, stop and tell the user the task is ready for `/review`.

## Filtering Work

### Filter DSL

All filtering uses a small expression language with these atoms and operators:

| Syntax | Meaning |
|--------|---------|
| `#tag` | Match tasks with this tag (includes virtual tags: READY, BLOCKED, BLOCKING) |
| `$project-slug` | Match tasks assigned to this project (by project slug/id) |
| `@user` | Match tasks assigned to this user |
| `^task-id` | Match tasks referencing this task (via depends_on or own id) |
| `&&` / `and` | Both sides must match |
| `\|\|` / `or` | Either side must match |
| `!` / `not` | Negate the following expression |
| `()` | Grouping |
| Adjacent atoms | Implicit AND: `#bug @alice` = `#bug && @alice` |

### Picking Up Work

Use `next task` with a `filter` to pick up specific kinds of work one task at a time:

```json
{"op": "next task", "filter": "#bug"}
{"op": "next task", "filter": "@alice"}
{"op": "next task", "filter": "#bug && @alice"}
{"op": "next task", "filter": "$auth-migration"}
{"op": "next task", "filter": "$auth-migration && @alice"}
```

This is the preferred way to work through tasks — it returns one ready task at a time and excludes done tasks automatically.

### Listing Tasks

**Never call `list tasks` with no parameters** — there is no good reason to dump every task. Always use a `filter` or `column`, or use `next task` to get one task at a time:

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

Note: `list tasks` automatically excludes done tasks unless you explicitly request `column: "done"`.

### Setting Up Tags

Create tags for the categories that matter to your project:

```json
{"op": "add tag", "id": "bug", "name": "Bug", "color": "ff0000", "description": "Bug fixes"}
{"op": "add tag", "id": "feature", "name": "Feature", "color": "00cc00"}
{"op": "add tag", "id": "chore", "name": "Chore", "color": "888888"}
```

Each tag needs an `id`, `name`, and `color` (6-char hex, no `#`). Description is optional.

### Applying Tags to Tasks

Tag tasks when you create them or as you learn more about the work:

```json
{"op": "add task", "title": "Fix login crash", "tags": ["bug"]}
{"op": "tag task", "id": "<task-id>", "tag": "feature"}
{"op": "untag task", "id": "<task-id>", "tag": "chore"}
```

### Managing Tags

You can list, update, or delete tags as the project evolves:

```json
{"op": "list tags"}
{"op": "update tag", "id": "bug", "name": "Bugfix", "color": "cc0000"}
{"op": "delete tag", "id": "chore"}
```

Deleting a tag automatically removes it from all tasks.

## Using Projects to Group Tasks

Projects let you organize related tasks under a shared initiative. Create a project for each plan or workstream.

### Creating a Project

```json
{"op": "add project", "id": "auth-migration", "name": "Auth Migration"}
{"op": "add project", "id": "frontend", "name": "Frontend", "description": "Frontend redesign", "color": "ff0000", "order": 5}
```

Required fields: `id` (slug) and `name`. Optional fields: `description`, `color` (6-char hex, no `#`), `order` (position in project list).

**Auto-ordering**: When `order` is omitted, projects auto-increment — the first project gets order 0, the next gets 1, and so on. Specify `order` explicitly to control positioning.

**Duplicate detection**: Creating a project with an existing `id` returns an error. Choose unique slugs.

### Getting a Project

```json
{"op": "get project", "id": "auth-migration"}
```

Returns `{id, name, description, color, order}`. Returns a `ProjectNotFound` error if the ID doesn't exist.

### Updating a Project

```json
{"op": "update project", "id": "auth-migration", "name": "JWT Auth Migration"}
{"op": "update project", "id": "auth-migration", "description": "New desc", "color": "aabbcc", "order": 42}
```

All fields except `id` are optional — only provided fields are changed. Updating with no fields succeeds and returns the current values.

### Listing Projects

```json
{"op": "list projects"}
```

Returns `{projects: [...], count: N}` sorted by `order` ascending.

### Deleting a Project

```json
{"op": "delete project", "id": "auth-migration"}
```

Returns `{deleted: true, id: "..."}` on success. **Fails with `ProjectHasTasks` if any tasks reference the project** — reassign or complete those tasks first.

### Assigning Tasks to Projects

Set the `project` field when creating or updating a task:

```json
{"op": "add task", "title": "Implement JWT refresh", "project": "auth-migration"}
{"op": "update task", "id": "<task-id>", "project": "frontend"}
```

Tasks without a project have an empty `project` field. The task response always includes `"project": "<slug>"` (or `""` if unset).

### Filtering by Project

Once tasks are assigned to projects, use the `$project-slug` atom in any filter to scope work to a specific project. It composes with other atoms the same way `#tag` and `@user` do:

```json
{"op": "next task", "filter": "$auth-migration"}
{"op": "list tasks", "filter": "$auth-migration && #bug"}
{"op": "list tasks", "filter": "$auth-migration || $frontend"}
{"op": "list tasks", "filter": "!$auth-migration"}
```

The slug after `$` is the project `id` you used in `add project`. Negation (`!$slug`) excludes the project, and `$a || $b` matches tasks in either project.

### Workflow

When starting a plan with multiple related tasks:

1. Create a project for the initiative
2. Create tasks with the `project` field set to the project ID
3. Use the `$project-slug` filter atom in `list tasks` and `next task` to focus work on a project

## Guidelines

- Each kanban task can have subtasks — you need to do all of these subtasks to complete the task
- Do not skip subtasks or mark them complete without actually doing the work
- If a subtask is blocked or unclear, add a comment to the task explaining the issue
- Run tests after completing each subtask to catch problems early
- Only mark the task as complete when every subtask is done and tests pass
- If you discover new work while executing a task, add it as a new kanban task — don't hold it in your head
