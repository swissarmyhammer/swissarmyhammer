---
name: kanban
description: Execute the next task from the kanban board. Use when the user says "kanban", "/kanban", "next task", "what's the next task", or "pick up work". Picks up the next ready task from the board and drives it through doing to review.
license: MIT OR Apache-2.0
compatibility: Requires the `kanban` MCP tool for all board, column, and task operations. 
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Kanban

Execute the tasks from the board.

{% include "_partials/findings-are-requirements" %}

## Use Kanban for All Task Tracking

The kanban board is your todo list. **Never use TodoWrite, TaskCreate, or any other task tool** — only `kanban`. This is the single source of truth across Claude Code and llama-agent sessions.

**Subtasks are GFM checklists** (`- [ ]` / `- [x]`) inside the task's `description`. There is no separate subtask API — include them when creating the task, or `update task` to modify the description.

## Short IDs — reference tasks by short id, never hand-abbreviated prefixes

Every task's stored identity is its full 26-char ULID (e.g. `01KT6SA4911JQPK09YQRC9RB4G`). For humans, each task also has a **short id**: the **last 7 characters of the ULID, lowercased**, shown as `^<short>` (e.g. `^rc9rb4g`). The short id is never stored — it is always derived from the ULID — and it is the canonical short handle.

**Quote the short id from the tool's `short_id` field.** Every task in `get task` / `list tasks` / `next task` output carries a `short_id` field. When you refer to a task in prose, commits, or chat, copy that value (as `^<short>`). **Never hand-abbreviate the ULID by prefix** (`01KT6SA…`): same-session tasks share long leading runs and a prefix like `01KT6SA` collides across sibling cards. The trailing short id is collision-free.

**References resolve forgivingly** — anywhere a task id is accepted (`get`/`move`/`complete`/`update` task, `depends_on`, the `^` filter atom) you may pass any of:

| Input | Resolves by |
|-------|-------------|
| `01KT6SA4911JQPK09YQRC9RB4G` | full ULID — the stored identity |
| `rc9rb4g` | exact short id (the canonical suffix) |
| `^rc9rb4g` | short id with the `^` sigil |
| `01KT6SAM` | unique ULID prefix (git-style) |

Matching is case-insensitive, and the canonical forms win: a full ULID or exact short id always beats a colliding prefix interpretation. A prefix that matches more than one task **does not resolve** — the tool reports the reference as not found (it does not list the matches), so disambiguate by quoting the full 7-char short id. A prefix only works when it is long enough to be unique on the board; the short same-session prefixes (e.g. `01KT6SA`) that this feature exists to avoid are exactly the ambiguous ones. Display is always the short form.

**Example** — the same task, two ways to name it:

- Full ULID (stored identity): `01KT6SA4911JQPK09YQRC9RB4G`
- Short id (what you write): `^rc9rb4g`

Both resolve to that one task; write the short id.

## Process

1. **Get next task**: `kanban` `op: "next task"` finds the next actionable task across all non-done columns.
   - Tag: `op: "next task", filter: "#bug"`
   - Assignee: `op: "next task", filter: "@alice"`
   - Combined: `op: "next task", filter: "#bug && @alice"`
2. **Move to doing**: `op: "move task", id: "<id>", column: "doing"`
3. **Read details**: `op: "get task", id: "<id>"`. Then review prior context per **Record progress** below.
4. **Work each subtask, check off immediately**:
   - Implement what it describes
   - `op: "update task", id: "<id>"`, change `- [ ]` → `- [x]` for the finished subtask
   - After EVERY subtask — never batch. The checklist is the progress indicator.
   - Preserve all other description content; only flip the one checkbox you finished.
5. **Record progress**: log milestones, failures, and discoveries on the task — see **Record progress** below.
6. **Move to review** when all subtasks are `- [x]`: first ensure the `review` column exists (idempotent — use the partial above), then `op: "move task", id: "<id>", column: "review"`. **Never use `complete task`** — that skips the review gate. After moving, stop and tell the user the task is ready for `/review`.

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
- Blocked or unclear → record it on the task (see **Record progress** above)
- Run tests after each subtask
- Only complete the task when all subtasks are done and tests pass
- New work discovered? Add a new kanban task — don't hold it in your head
