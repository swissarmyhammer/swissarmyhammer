# Todo Show Tool

Retrieve a specific todo item or the next incomplete item from a todo list.

## Purpose
Display todo items for review and work planning. Supports both specific item lookup by ULID and sequential workflow using the "next" pattern.

## Parameters

- `todo_list` (required): Name of the todo list file (without extension)
- `item` (required): Either a specific ULID or "next" to get the first incomplete item

## Behavior

- If `item` is a ULID: Returns the specific todo item as structured data and YAML
- If `item` is "next": Returns the first incomplete todo item (FIFO order)
- Enforces single-item focus to avoid context pollution
- Returns complete item with all fields
- Returns both JSON structure and YAML representation for easy reading

## Examples

Get a specific todo item by ID:
```json
{
  "todo_list": "feature_work",
  "item": "01K1KQM85501ECE8XJGNZKNJQW"
}
```

Get the next incomplete item:
```json
{
  "todo_list": "feature_work",
  "item": "next"
}
```

Get next item for sequential processing:
```json
{
  "todo_list": "current_session",
  "item": "next"
}
```

## Returns

Success response with todo item (when found):
```json
{
  "todo_item": {
    "id": "01K1KQM85501ECE8XJGNZKNJQW",
    "task": "Implement file read functionality",
    "context": "Use existing patterns for inspiration",
    "done": false
  },
  "yaml": "id: 01K1KQM85501ECE8XJGNZKNJQW\ntask: \"Implement file read functionality\"\ncontext: \"Use existing patterns for inspiration\"\ndone: false"
}
```

Success response when no next item available:
```json
{
  "message": "No incomplete todo items found in list 'feature_work'",
  "todo_item": null
}
```
