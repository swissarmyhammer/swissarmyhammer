---
name: kanban
description: Execute the next task from the kanban board. Use when the user wants to make progress on planned work by implementing the next available todo item.
metadata:
  author: swissarmyhammer
  version: "1.2"
---

# Do

Pick up and execute the next task from the kanban board.

## Important: Use Kanban for All Task Tracking

The kanban board is your todo list. Do NOT use any built-in task or todo tools (like TodoWrite or TaskCreate) — always use the `kanban` tool instead. Every task, subtask, and work item belongs on the kanban board as cards with subtasks. This is how work is tracked across both Claude Code and llama-agent sessions, so it must be the single source of truth.

When the user asks you to track work, create a todo list, or remember tasks — use kanban cards, not any other mechanism.

## Process

1. Get the next task: use `kanban` with `op: "next task"` to find the next actionable card. This searches all non-done columns for ready tasks.
   - To filter by tag: `op: "next task"`, `tag: "<tag-id>"`
   - To filter by assignee: `op: "next task"`, `assignee: "<actor-id>"`
2. Move it to doing: use `kanban` with `op: "move task"`, `id: "<task-id>"`, `column: "doing"`
3. Read the task details: use `kanban` with `op: "get task"`, `id: "<task-id>"` to see description and subtasks
4. Work through each subtask:
   - Implement what the subtask describes
   - Mark it complete: use `kanban` with `op: "complete subtask"`, `task_id: "<task-id>"`, `id: "<subtask-id>"`
5. **Complete the card**: when ALL subtasks are done, use `kanban` with `op: "complete task"`, `id: "<task-id>"`. You MUST do this — never leave a card in "doing" when the work is finished.

## Filtering Work

### By Tag

Use `next task` with a `tag` filter to pick up specific kinds of work one card at a time:

```json
{"op": "next task", "tag": "review-finding"}
```

This is the preferred way to work through tagged cards — it returns one ready card at a time and excludes done cards automatically.

**Avoid** using `list tasks` with only a `tag` filter — this returns every matching card across all columns (including done) and can produce huge results. If you must list, always combine `tag` with `column`:

```json
{"op": "list tasks", "tag": "bug", "column": "todo"}
```

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

## Guidelines

- Each kanban card can have subtasks — you need to do all of these subtasks to complete the card
- Do not skip subtasks or mark them complete without actually doing the work
- If a subtask is blocked or unclear, add a comment to the task explaining the issue
- Run tests after completing each subtask to catch problems early
- Only mark the card as complete when every subtask is done and tests pass
- If you discover new work while executing a task, add it as a new kanban card — don't hold it in your head
