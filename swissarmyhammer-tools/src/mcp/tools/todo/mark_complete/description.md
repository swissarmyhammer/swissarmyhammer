Mark a todo item as completed in a todo list.

## Parameters

- `id` (required): ULID of the todo item to mark as complete

## Examples

```json
{
  "id": "01K1KQM85501ECE8XJGNZKNJQW"
}
```

## Returns

Returns confirmation that item was marked complete. Automatically deletes the todo file when all tasks are complete.
