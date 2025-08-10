Create a new memo with the given title and content. Returns the created memo with its unique ID.

## Parameters

- `title` (required): Title of the memo
- `content` (required): Markdown content of the memo

## Examples

Create a memo with title and content:
```json
{
  "title": "Meeting Notes",
  "content": "# Team Meeting\n\nDiscussed project roadmap..."
}
```

## Returns

Returns the created memo with its unique ULID identifier and metadata.