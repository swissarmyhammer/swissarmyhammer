Add a new item to a todo list for ephemeral task tracking during development sessions.

## Parameters

- `task` (required): Brief description of the task to be completed
- `context` (optional): Additional context, notes, or implementation details

## Examples

```json
{
  "task": "Implement file read functionality",
  "context": "Look at existing patterns in memoranda module"
}
```

## Returns

Returns confirmation that todo item was created with ULID. Todo lists stored in `.swissarmyhammer/todo.yaml`.
