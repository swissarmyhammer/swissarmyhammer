# Todo Create Tool

Add a new item to a todo list for ephemeral task tracking during development sessions.

## Purpose
Create new todo items in specified todo lists. Todo lists are stored as `.todo.yaml` files in `.swissarmyhammer/todo/` and are designed for temporary, session-based task management.

## Parameters

- `todo_list` (required): Name of the todo list file (without extension)
- `task` (required): Brief description of the task to be completed
- `context` (optional): Additional context, notes, or implementation details

## Behavior

- Auto-creates the todo list file if it doesn't exist
- Generates a sequential ULID for the new item
- Adds the item with `done: false` status
- Appends to the existing todo list
- Files are stored in `.swissarmyhammer/todo/` (repo root if in Git repo, otherwise current directory)
- Todo files should be added to `.gitignore` to prevent accidental commits

## Examples

Create a simple todo item:
```json
{
  "todo_list": "feature_work",
  "task": "Implement file read functionality"
}
```

Create a todo item with context:
```json
{
  "todo_list": "refactoring",
  "task": "Extract common validation logic",
  "context": "Look at existing patterns in memoranda and issues modules"
}
```

Create a todo for current session:
```json
{
  "todo_list": "current_session",
  "task": "Fix test failures in todo module",
  "context": "Tests are failing because of missing test utilities dependency"
}
```

## Returns

Success response with created todo item details:
```json
{
  "message": "Created todo item in list 'feature_work'",
  "todo_item": {
    "id": "01K1KQM85501ECE8XJGNZKNJQW",
    "task": "Implement file read functionality",
    "context": null,
    "done": false
  }
}
```