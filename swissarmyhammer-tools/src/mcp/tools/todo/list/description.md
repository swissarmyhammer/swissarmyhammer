List all available todo items with optional filtering by completion status.

## Examples

List all todos:
```json
{}
```

List only incomplete todos:
```json
{
  "completed": false
}
```

List only completed todos:
```json
{
  "completed": true
}
```

## Returns

Returns list of all todos with their metadata including id, task, context, completion status, along with summary counts of total, completed, and pending items.
