Add a new item to a todo list for ephemeral task tracking during development sessions.

## Examples

```json
{
  "task": "Implement file read functionality",
  "context": "Look at existing patterns in memoranda module"
}
```

## Returns

Returns confirmation that todo item was created with:
- `todo_item`: The created todo item with id, task, context, done status, created_at, and updated_at timestamps
- `gc_count`: Number of completed todo items that were garbage collected during creation

Todo lists stored in `.swissarmyhammer/todo.yaml`.
