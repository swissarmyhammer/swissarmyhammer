# Todo Mark Complete Tool

Mark a todo item as completed in a todo list.

## Purpose
Update todo items to completed status when tasks are finished. Automatically manages the todo list lifecycle by removing the file when all items are complete.

## Parameters

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
  "id": "01K1KQM85501ECE8XJGNZKNJQW"
}
```
