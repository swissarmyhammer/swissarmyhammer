# Todo Create Tool

Add a new item to a todo list for ephemeral task tracking during development sessions.

## Purpose
Create new todo items in specified todo lists. Todo lists are stored in `.swissarmyhammer/todo.yaml` and are designed for temporary, session-based task management.

## Parameters

- `task` (required): Brief description of the task to be completed
- `context` (optional): Additional context, notes, or implementation details

## Behavior

- Auto-creates the todo list file if it doesn't exist
- Generates a sequential ULID for the new item
- Adds the item with `done: false` status
- Appends to the existing todo list
- Todo files should be added to `.gitignore` to prevent accidental commits

## Examples

Create a simple todo item:
```json
{
  "task": "Implement file read functionality"
}
```

Create a todo item with context:
```json
{
  "task": "Extract common validation logic",
  "context": "Look at existing patterns in memoranda and issues modules"
}
```

Create a todo for current session:
```json
{
  "task": "Fix test failures in todo module",
  "context": "Tests are failing because of missing test utilities dependency"
}
```
