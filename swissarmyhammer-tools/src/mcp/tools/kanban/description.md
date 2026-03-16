# kanban

Kanban board operations for task management. File-backed — one `.kanban` directory per repo.

## Operations

The tool accepts `op` as a "verb noun" string (e.g., "add task", "move task"). Parameters are defined in the JSON schema.

**Board**: `init board`, `get board`, `update board`
**Columns**: `add column`, `get column`, `update column`, `delete column`, `list columns`
**Swimlanes**: `add swimlane`, `get swimlane`, `update swimlane`, `delete swimlane`, `list swimlanes`
**Actors**: `add actor` (use `ensure: true` for idempotent registration), `get actor`, `update actor`, `delete actor`, `list actors`
**Tasks**: `add task`, `get task`, `update task`, `move task`, `delete task`, `complete task`, `assign task`, `unassign task`, `next task`, `tag task`, `untag task`, `list tasks`
**Tags**: `add tag`, `get tag`, `update tag`, `delete tag`, `list tags`
**Comments**: `add comment`, `get comment`, `update comment`, `delete comment`, `list comments`
**Attachments**: `add attachment`, `get attachment`, `update attachment`, `delete attachment`, `list attachments`
**Activity**: `list activity`

## Important Behaviors

- `list tasks`: **Always provide at least one filter** (`column`, `tag`, `assignee`, `ready`). Done tasks excluded by default. Prefer `next task` for one actionable card at a time.
- `next task`: Returns oldest ready task (no incomplete dependencies) from any non-done column.
- `depends_on`: Tasks with incomplete dependencies are blocked and hidden from `ready` queries.
- Your actor is auto-created on MCP connect. Tasks you create are auto-assigned to you.

## Examples

```json
{"op": "init board", "name": "Sprint 1"}
{"op": "add tag", "name": "bug", "color": "ff0000"}
{"op": "add task", "title": "Fix login bug", "tags": ["bug"]}
{"op": "move task", "id": "<task-id>", "column": "doing"}
{"op": "next task"}
{"op": "complete task", "id": "<task-id>"}
{"op": "list tasks", "column": "todo"}
{"op": "add task", "title": "Blocked work", "depends_on": ["<blocker-id>"]}
{"op": "list activity", "limit": 10}
```
