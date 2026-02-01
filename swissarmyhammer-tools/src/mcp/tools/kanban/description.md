# kanban

Kanban board operations for task management. This is the best way to keep a TODO list for a project.

## Overview

The kanban tool provides file-backed task board management. A `.kanban` directory in a repository root **is** the project - one board per repo.

## Operations

The tool accepts `op` as a "verb noun" string (e.g., "add task", "move task").

### Board Operations

- `init board` - Initialize a new board (requires `name`)
- `get board` - Get board metadata
- `update board` - Update board metadata

### Task Operations

- `add task` - Add a new task (requires `title`)
- `get task` - Get task by ID (requires `id`)
- `update task` - Update a task (requires `id`)
- `move task` - Move task to column (requires `id` and `column`)
- `delete task` - Delete a task (requires `id`)
- `next task` - Get next actionable task
- `list tasks` - List tasks (optional filters: `column`, `ready`)

### Column Operations

- `add column` - Add a column (requires `id` and `name`)
- `get column` - Get column by ID
- `update column` - Update column
- `delete column` - Delete column (fails if has tasks)
- `list columns` - List all columns

## Examples

### Initialize a board

```json
{
  "op": "init board",
  "name": "My Project"
}
```

### Add a task

```json
{
  "op": "add task",
  "title": "Implement feature X",
  "description": "Details about the feature"
}
```

### Move a task

```json
{
  "op": "move task",
  "id": "01ABC123...",
  "column": "done"
}
```

### List ready tasks

```json
{
  "op": "list tasks",
  "ready": true
}
```

## Forgiving Input

The tool accepts multiple input formats:

```json
// Explicit op
{ "op": "add task", "title": "Fix bug" }

// Shorthand
{ "add": "task", "title": "Fix bug" }

// Inferred (has title but no id)
{ "title": "Fix bug" }
```
