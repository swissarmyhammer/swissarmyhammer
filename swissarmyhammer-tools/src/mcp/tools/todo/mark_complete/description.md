# Todo Mark Complete Tool

Mark a todo item as completed in a todo list.

## Purpose
Update todo items to completed status when tasks are finished. Automatically manages the todo list lifecycle by removing the file when all items are complete.

## Parameters

- `todo_list` (required): Name of the todo list file (without extension)
- `id` (required): ULID of the todo item to mark as complete

## Behavior

- Marks the specified item as `done: true`
- Preserves all items in the file for status tracking and debugging
- Updates the todo list file with the new completion status
- If all tasks in the list are complete, automatically deletes the file
- Maintains clean workspace by removing completed todo lists

## Examples

Mark a specific todo item complete:
```json
{
  "todo_list": "feature_work",
  "id": "01K1KQM85501ECE8XJGNZKNJQW"
}
```

Complete an item from current session list:
```json
{
  "todo_list": "current_session",
  "id": "01K1KQM85501ECE8XJGNZKNJQX"
}
```

Mark refactoring task complete:
```json
{
  "todo_list": "refactoring",
  "id": "01K1KQM85501ECE8XJGNZKNJQY"
}
```

## File Lifecycle

1. **Item Marked Complete**: Item's `done` field updated to `true`
2. **Partial Completion**: File remains with mix of complete/incomplete items
3. **All Complete**: File automatically deleted to maintain clean workspace

## Returns

Success response confirming completion:
```json
{
  "message": "Marked todo item '01K1KQM85501ECE8XJGNZKNJQW' as complete in list 'feature_work'",
  "action": "marked_complete",
  "todo_list": "feature_work",
  "id": "01K1KQM85501ECE8XJGNZKNJQW"
}
```
